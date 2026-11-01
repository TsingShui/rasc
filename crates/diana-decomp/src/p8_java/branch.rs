use crate::input::{MethodInput, RawPrintable};
use crate::p6_variables::normal::StorageBody;
use crate::pipeline::branch::{BlockId, EdgeId, Kind, Op, Terminal, TypedBody};
use crate::substrate::owned::{TextBuffer, copy_units, reserve};
use crate::{EnvironmentId, MethodBudget, RequestError, VerifiedJavaMethod};

pub(crate) use crate::p6_variables::LocalId;
pub(crate) use crate::p6_variables::normal::Local;
pub(crate) enum Right {
    Local(ValueUse),
    Literal(i32),
}
pub(crate) enum Expression {
    Unary(crate::pipeline::scalar::Unary, ValueUse),
    ReverseSubtract(i32, ValueUse),
    Compare(ValueUse, crate::pipeline::scalar::Compare, ValueUse),
    Binary(ValueUse, crate::pipeline::branch::Arithmetic, Right),
    Local(ValueUse),
    Constant(i32),
}
pub(crate) struct Assignment {
    pub target: LocalId,
    pub expression: Expression,
}
pub(crate) use super::values::ValueUse;
pub(super) fn returned(
    ssa: &TypedBody<'_>,
    value: LocalId,
) -> Result<Option<ValueUse>, RequestError> {
    let conversion = ssa
        .returned_uses
        .get(ix(value.0)?)
        .ok_or(RequestError::InternalInvariant(
            "return use has foreign value",
        ))?;
    Ok(conversion.map(|conversion| super::values::lower(value, conversion)))
}

pub(crate) struct Diamond {
    pub locals: Vec<Local>,
    pub entry: Vec<Assignment>,
    pub yes: Vec<Assignment>,
    pub no: Vec<Assignment>,
    pub condition: LocalId,
    pub condition_kind: Kind,
    pub opcode: u8,
    pub returned: ValueUse,
}
fn ix(id: u32) -> Result<usize, RequestError> {
    usize::try_from(id).map_err(|_| RequestError::PlatformCapacity)
}
fn append<T>(
    items: &mut Vec<T>,
    item: T,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<(), RequestError> {
    reserve(items, 1, budget)?;
    items.push(item);
    Ok(())
}
fn edge(
    ssa: &TypedBody<'_>,
    source: BlockId,
    target: BlockId,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<EdgeId, RequestError> {
    let mut found = None;
    for (index, edge) in ssa.ssa.edges.iter().enumerate() {
        budget.charge_work(1)?;
        if edge.source == source && edge.target == target {
            if found.is_some() {
                return Err(RequestError::InternalInvariant("diamond edge is ambiguous"));
            }
            found = Some(EdgeId(
                u32::try_from(index).map_err(|_| RequestError::PlatformCapacity)?,
            ));
        }
    }
    found.ok_or(RequestError::InternalInvariant("diamond edge is missing"))
}
pub(super) fn local_declarations(
    storage: &StorageBody<'_>,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<Vec<Local>, RequestError> {
    let mut locals = Vec::new();
    for &local in &storage.locals {
        budget.charge_work(1)?;
        append(&mut locals, local, budget)?;
    }
    Ok(locals)
}
pub(super) fn copies(
    storage: &StorageBody<'_>,
    incoming: Option<EdgeId>,
    target: BlockId,
    output: &mut Vec<Assignment>,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<(), RequestError> {
    for copy in &storage.copies(incoming, target)?.steps {
        budget.charge_work(1)?;
        append(
            output,
            Assignment {
                target: copy.target,
                expression: Expression::Local(ValueUse::Direct(copy.source)),
            },
            budget,
        )?;
    }
    Ok(())
}
pub(super) fn operation(
    typed: &TypedBody<'_>,
    op: &crate::pipeline::branch::BoundOp,
) -> Result<Option<Assignment>, RequestError> {
    use crate::p2_flow::normal::Operand;
    let numeric = |index: usize| -> Result<ValueUse, RequestError> {
        let usage = typed
            .numeric_uses
            .get(ix(op.id.0)?)
            .and_then(Option::as_ref)
            .and_then(|uses| uses.uses.get(index))
            .ok_or(RequestError::InternalInvariant(
                "numeric operation lost typed use",
            ))?;
        if usage.value != op.input(index)? {
            return Err(RequestError::InternalInvariant(
                "numeric use changed original input",
            ));
        }
        Ok(super::values::lower(
            LocalId(usage.value.0),
            usage.conversion,
        ))
    };
    let expression = match op.op {
        Op::Unary(_, unary, _) => Expression::Unary(unary, numeric(0)?),
        Op::ReverseSubtract(_, literal, _) => Expression::ReverseSubtract(literal, numeric(0)?),
        Op::Compare(_, _, compare, _) => Expression::Compare(numeric(0)?, compare, numeric(1)?),
        Op::FloatBinary(_, _, kind, _) => {
            Expression::Binary(numeric(0)?, kind, Right::Local(numeric(1)?))
        }
        Op::Binary(_, _, kind, right) => Expression::Binary(
            numeric(0)?,
            kind,
            match right {
                Operand::Register(_) => Right::Local(numeric(1)?),
                Operand::Literal(value) => Right::Literal(value),
            },
        ),
        Op::Nop => return Ok(None),
        Op::NewInstance(..)
        | Op::Initialize(..)
        | Op::NewArray(..)
        | Op::FreshFill(..)
        | Op::EmptyFill(..)
        | Op::Cast(..)
        | Op::String(..)
        | Op::Call(_, _)
        | Op::ArrayLength(..)
        | Op::ArrayLoad(..)
        | Op::ArrayStore(..)
        | Op::FieldLoad(..)
        | Op::FieldStore(..) => {
            return Err(RequestError::InternalInvariant(
                "call entered pure assignment lowering",
            ));
        }
        Op::Constant(_, value) => Expression::Constant(value),
        Op::Move(_, _) => Expression::Local(if typed.numeric_uses[ix(op.id.0)?].is_some() {
            numeric(0)?
        } else {
            ValueUse::Direct(LocalId(op.input(0)?.0))
        }),
    };
    Ok(Some(Assignment {
        target: LocalId(
            op.result
                .ok_or(RequestError::InternalInvariant(
                    "verified operation lost its local",
                ))?
                .0,
        ),
        expression,
    }))
}
pub(super) fn operations(
    ssa: &TypedBody<'_>,
    block: BlockId,
    output: &mut Vec<Assignment>,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<(), RequestError> {
    for &id in &ssa.ssa.bound[ix(block.0)?].ops {
        budget.charge_work(1)?;
        let op = ssa.ssa.operation(id)?;
        if let Some(statement) = operation(ssa, op)? {
            append(output, statement, budget)?;
        }
    }
    Ok(())
}

pub(crate) fn verify(
    input: &MethodInput<'_, '_>,
    catalog: &crate::DeclarationCatalog,
    storage: StorageBody<'_>,
    environment: EnvironmentId,
    representation: crate::RepresentationPolicy,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<Option<VerifiedJavaMethod>, RequestError> {
    super::fresh_fill::validate(input, &storage.typed, budget)?;
    if !super::cast_ops::validate(input, catalog, &storage.typed, budget)? {
        return Ok(None);
    }
    let Some(mut calls) = super::calls::validate_bindings(input, catalog, &storage.typed, budget)?
    else {
        return Ok(None);
    };
    let Some(type_names) = super::types::resolve(input, catalog, &storage, budget)? else {
        return Ok(None);
    };
    if matches!(
        representation,
        crate::RepresentationPolicy::StructuredThenDispatch
    ) && let Some(mut method) =
        verify_diamond(input, &storage, &type_names, environment, budget)?
    {
        method.type_names = type_names;
        return Ok(Some(method));
    }
    let method = super::dispatcher::verify(
        input,
        &storage,
        &type_names,
        &mut calls,
        environment,
        representation,
        budget,
    )?;
    Ok(method.map(|mut method| {
        method.type_names = type_names;
        method
    }))
}

fn verify_diamond(
    input: &MethodInput<'_, '_>,
    storage: &StorageBody<'_>,
    type_names: &super::types::TypeNames,
    environment: EnvironmentId,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<Option<VerifiedJavaMethod>, RequestError> {
    let ssa = &storage.typed;
    if ssa.ssa.blocks.len() != 4 {
        return Ok(None);
    }
    for block in &ssa.ssa.blocks {
        budget.charge_work(1)?;
        for op in &block.ops {
            budget.charge_work(1)?;
            if op.kind.effects().observable() {
                return Ok(None);
            }
        }
    }
    let Terminal::Test {
        register: _,
        other,
        opcode,
        yes,
        no,
    } = ssa.ssa.blocks[0].terminal
    else {
        return Ok(None);
    };
    if other.is_some() {
        return Ok(None);
    }
    if yes == no || yes == BlockId(0) || no == BlockId(0) {
        return Ok(None);
    }
    let (Terminal::Jump(join), Terminal::Jump(other)) = (
        ssa.ssa.blocks[ix(yes.0)?].terminal,
        ssa.ssa.blocks[ix(no.0)?].terminal,
    ) else {
        return Ok(None);
    };
    if join != other
        || join == yes
        || join == no
        || join == BlockId(0)
        || !matches!(ssa.ssa.blocks[ix(join.0)?].terminal, Terminal::Return(_))
    {
        return Ok(None);
    }
    if ssa.ssa.edges.len() != 4 {
        return Ok(None);
    }
    let RawPrintable::Method(raw) = input.raw() else {
        return Err(RequestError::InternalInvariant(
            "diamond lost its original method",
        ));
    };
    if !std::ptr::eq(
        raw.layout().original().instructions(),
        ssa.ssa.raw.instructions(),
    ) {
        return Err(RequestError::InternalInvariant(
            "diamond belongs to another raw scope",
        ));
    }
    if raw.annotations().method_range().is_some() || raw.annotations().parameter_range().is_some() {
        return Ok(None);
    }
    super::declarations::check_target(input, budget)?;
    let flags = input.access_flags();
    if flags & 8 == 0
        || flags & !0x81f != 0
        || (flags & 7).count_ones() > 1
        || input.owner_access_flags() & 0x200 != 0
    {
        return Ok(None);
    }
    let name = copy_units(input.declaration().name, budget)?;
    if !super::declarations::identifier(&name, budget)? {
        return Ok(None);
    }
    let locals = local_declarations(storage, budget)?;
    let condition = LocalId(
        ssa.ssa.bound[0]
            .terminal
            .ok_or(RequestError::InternalInvariant(
                "diamond has no bound condition",
            ))?
            .0,
    );
    let condition_kind = locals[ix(condition.0)?].kind;
    if condition_kind == Kind::Float {
        return Ok(None);
    }
    if matches!(
        condition_kind,
        Kind::Boolean
            | Kind::Object
            | Kind::String
            | Kind::PrimitiveArray(_)
            | Kind::ObjectArray
            | Kind::Nominal(_)
    ) && !matches!(opcode, 0x38 | 0x39)
    {
        return Ok(None);
    }
    let returned = LocalId(
        ssa.ssa.bound[ix(join.0)?]
            .terminal
            .ok_or(RequestError::InternalInvariant(
                "diamond has no bound return",
            ))?
            .0,
    );
    let Some(returned) = self::returned(ssa, returned)? else {
        return Ok(None);
    };
    let mut entry = Vec::new();
    copies(storage, None, BlockId(0), &mut entry, budget)?;
    operations(ssa, BlockId(0), &mut entry, budget)?;
    let mut yes_statements = Vec::new();
    copies(
        storage,
        Some(edge(ssa, BlockId(0), yes, budget)?),
        yes,
        &mut yes_statements,
        budget,
    )?;
    operations(ssa, yes, &mut yes_statements, budget)?;
    copies(
        storage,
        Some(edge(ssa, yes, join, budget)?),
        join,
        &mut yes_statements,
        budget,
    )?;
    let mut no_statements = Vec::new();
    copies(
        storage,
        Some(edge(ssa, BlockId(0), no, budget)?),
        no,
        &mut no_statements,
        budget,
    )?;
    operations(ssa, no, &mut no_statements, budget)?;
    copies(
        storage,
        Some(edge(ssa, no, join, budget)?),
        join,
        &mut no_statements,
        budget,
    )?;
    // Join operations are a sequence after both incoming copy plans. The first
    // profile accepts a return-only join, not a skipped operation list.
    for &id in &ssa.ssa.bound[ix(join.0)?].ops {
        budget.charge_work(1)?;
        if !matches!(ssa.ssa.operation(id)?.op, Op::Nop) {
            return Ok(None);
        }
    }
    let mut header = TextBuffer::new();
    for (bit, text) in [
        (1, "public "),
        (2, "private "),
        (4, "protected "),
        (8, "static "),
        (0x10, "final "),
        (0x800, "strictfp "),
    ] {
        if flags & bit != 0 {
            header.push(text, budget)?;
        }
    }
    header.push(type_names.result(ssa.ssa.result, budget)?, budget)?;
    header.push(" ", budget)?;
    for unit in name {
        header.character(
            char::from_u32(u32::from(unit)).ok_or(RequestError::InternalInvariant(
                "checked diamond identifier is not scalar",
            ))?,
            budget,
        )?;
    }
    header.push("(", budget)?;
    for (index, kind) in ssa.ssa.parameters.iter().enumerate() {
        budget.charge_work(1)?;
        if index != 0 {
            header.push(", ", budget)?;
        }
        header.push(type_names.declaration(*kind, budget)?, budget)?;
        header.push(" p", budget)?;
        header.unsigned(
            u64::try_from(index).map_err(|_| RequestError::PlatformCapacity)?,
            budget,
        )?;
    }
    header.push(")", budget)?;
    Ok(Some(VerifiedJavaMethod::diamond(
        header.finish(),
        Diamond {
            locals,
            entry,
            yes: yes_statements,
            no: no_statements,
            condition,
            condition_kind,
            opcode,
            returned,
        },
        environment,
    )))
}
