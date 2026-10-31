use crate::input::{MethodInput, RawPrintable};
use crate::pipeline::branch::{BlockId, Definition, EdgeId, Kind, Op, Sym, Terminal, VerifiedSsa};
use crate::substrate::owned::{TextBuffer, copy_units, reserve};
use crate::{EnvironmentId, MethodBudget, RequestError, VerifiedJavaMethod};

pub(crate) use crate::p6_variables::LocalId;
pub(crate) struct Local {
    pub id: LocalId,
    pub kind: Kind,
    pub parameter: Option<u32>,
}
pub(crate) enum Right {
    Local(LocalId),
    Literal(i32),
}
pub(crate) enum Expression {
    Compare(LocalId, crate::pipeline::scalar::Compare, LocalId),
    Binary(LocalId, crate::pipeline::branch::Arithmetic, Right),
    Local(LocalId),
    Constant(i32),
}
pub(crate) struct Assignment {
    pub target: LocalId,
    pub expression: Expression,
}
#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum Return {
    Direct(LocalId),
    Boolean01(LocalId),
    BooleanWord(LocalId),
    Narrow {
        value: LocalId,
        kind: crate::p5_types::Narrow,
    },
}
impl Return {
    pub(crate) fn value(self) -> LocalId {
        match self {
            Self::Direct(value)
            | Self::Boolean01(value)
            | Self::BooleanWord(value)
            | Self::Narrow { value, .. } => value,
        }
    }
}
pub(super) fn returned(
    ssa: &VerifiedSsa<'_>,
    value: LocalId,
) -> Result<Option<Return>, RequestError> {
    let index = ix(value.0)?;
    use crate::pipeline::branch::DeclarationType;
    if let DeclarationType::Narrow(kind) = ssa.result {
        let mut range = ssa.ranges[index];
        if ssa.integral_bits[index] != 0 && ssa.integral_bits[index] & 4 == 0 {
            range = range.and_then(crate::p5_types::IntRange::known01);
        }
        return Ok(
            if ssa.kinds[index] == Some(Kind::Int)
                && range.is_some_and(|range| range.within(kind.range()))
            {
                Some(Return::Narrow { value, kind })
            } else {
                None
            },
        );
    }
    Ok(match (ssa.kinds[index], ssa.result.kind()) {
        (Some(Kind::Int), Kind::Int) | (Some(Kind::Boolean), Kind::Boolean) => {
            Some(Return::Direct(value))
        }
        (Some(Kind::Boolean), Kind::Int) => Some(Return::BooleanWord(value)),
        (Some(Kind::Int), Kind::Boolean)
            if ssa.integral_bits[index] != 0 && ssa.integral_bits[index] & 4 == 0 =>
        {
            Some(Return::Boolean01(value))
        }
        _ => None,
    })
}
pub(crate) struct Diamond {
    pub locals: Vec<Local>,
    pub entry: Vec<Assignment>,
    pub yes: Vec<Assignment>,
    pub no: Vec<Assignment>,
    pub condition: LocalId,
    pub condition_kind: Kind,
    pub opcode: u8,
    pub returned: Return,
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
    ssa: &VerifiedSsa<'_>,
    source: BlockId,
    target: BlockId,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<EdgeId, RequestError> {
    let mut found = None;
    for (index, edge) in ssa.edges.iter().enumerate() {
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
pub(super) fn copies(
    ssa: &VerifiedSsa<'_>,
    incoming: Option<EdgeId>,
    target: BlockId,
    output: &mut Vec<Assignment>,
    locals: &mut Vec<Local>,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<(), RequestError> {
    use crate::p6_variables::{Copy, LocalType, LocalTypeId, Width};
    let mut parallel = Vec::new();
    for phi in &ssa.phis {
        budget.charge_work(1)?;
        if phi.block == target {
            let mut sym = None;
            for &(edge, value) in &phi.inputs {
                budget.charge_work(1)?;
                if edge == incoming {
                    if sym.is_some() {
                        return Err(RequestError::InternalInvariant(
                            "verified Phi has a duplicate edge",
                        ));
                    }
                    sym = Some(value);
                }
            }
            let sym = sym.ok_or(RequestError::InternalInvariant(
                "verified Phi lost its edge",
            ))?;
            let Sym::Value(source) = sym else {
                return Err(RequestError::InternalInvariant(
                    "verified diamond contains a bad Phi input",
                ));
            };
            append(
                &mut parallel,
                Copy {
                    target: LocalId(phi.result.0),
                    source: LocalId(source.0),
                },
                budget,
            )?;
        }
    }
    let mut types = Vec::new();
    for local in locals.iter() {
        budget.charge_work(1)?;
        append(
            &mut types,
            LocalType::new(
                LocalTypeId(match local.kind {
                    Kind::Int => 0,
                    Kind::Boolean => 1,
                    Kind::Object => 2,
                    Kind::Float => 3,
                }),
                1,
            )?,
            budget,
        )?;
    }
    let plan = crate::p6_variables::plan(&parallel, &types, budget)?;
    for temporary in plan.temporaries {
        budget.charge_work(1)?;
        if ix(temporary.local.0)? != locals.len() {
            return Err(RequestError::InternalInvariant(
                "copy temporary does not extend local storage",
            ));
        }
        let kind = match (temporary.kind.identity.0, temporary.kind.width) {
            (0, Width::One) => Kind::Int,
            (1, Width::One) => Kind::Boolean,
            (2, Width::One) => Kind::Object,
            (3, Width::One) => Kind::Float,
            _ => {
                return Err(RequestError::InternalInvariant(
                    "diamond copy temporary has a foreign type",
                ));
            }
        };
        append(
            locals,
            Local {
                id: temporary.local,
                kind,
                parameter: None,
            },
            budget,
        )?;
    }
    for copy in plan.steps {
        budget.charge_work(1)?;
        append(
            output,
            Assignment {
                target: copy.target,
                expression: Expression::Local(copy.source),
            },
            budget,
        )?;
    }
    Ok(())
}
pub(super) fn operations(
    ssa: &VerifiedSsa<'_>,
    block: BlockId,
    output: &mut Vec<Assignment>,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<(), RequestError> {
    for op in &ssa.bound[ix(block.0)?].ops {
        budget.charge_work(1)?;
        let expression = match op.op {
            Op::Compare(_, _, compare, _) => {
                let Some(crate::pipeline::branch::BoundOperand::Value(right)) = op.other else {
                    return Err(RequestError::InternalInvariant(
                        "owned cmp lost right operand",
                    ));
                };
                Expression::Compare(
                    LocalId(
                        op.source
                            .ok_or(RequestError::InternalInvariant(
                                "owned cmp lost left operand",
                            ))?
                            .0,
                    ),
                    compare,
                    LocalId(right.0),
                )
            }
            Op::Binary(_, _, kind, _) => Expression::Binary(
                LocalId(
                    op.source
                        .ok_or(RequestError::InternalInvariant(
                            "verified arithmetic left local absent",
                        ))?
                        .0,
                ),
                kind,
                match op.other.ok_or(RequestError::InternalInvariant(
                    "verified arithmetic right local absent",
                ))? {
                    crate::pipeline::branch::BoundOperand::Value(id) => Right::Local(LocalId(id.0)),
                    crate::pipeline::branch::BoundOperand::Literal(value) => Right::Literal(value),
                },
            ),
            Op::Nop => continue,
            Op::Constant(_, value) => Expression::Constant(value),
            Op::Move(_, _) => Expression::Local(LocalId(
                op.source
                    .ok_or(RequestError::InternalInvariant(
                        "verified move lost its source",
                    ))?
                    .0,
            )),
        };
        append(
            output,
            Assignment {
                target: LocalId(
                    op.result
                        .ok_or(RequestError::InternalInvariant(
                            "verified operation lost its local",
                        ))?
                        .0,
                ),
                expression,
            },
            budget,
        )?;
    }
    Ok(())
}

pub(crate) fn verify(
    input: &MethodInput<'_, '_>,
    ssa: VerifiedSsa<'_>,
    environment: EnvironmentId,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<Option<VerifiedJavaMethod>, RequestError> {
    if let Some(method) = verify_diamond(input, &ssa, environment, budget)? {
        return Ok(Some(method));
    }
    super::dispatcher::verify(input, &ssa, environment, budget)
}

fn verify_diamond(
    input: &MethodInput<'_, '_>,
    ssa: &VerifiedSsa<'_>,
    environment: EnvironmentId,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<Option<VerifiedJavaMethod>, RequestError> {
    if ssa.blocks.len() != 4 {
        return Ok(None);
    }
    let Terminal::Test {
        register: _,
        other,
        opcode,
        yes,
        no,
    } = ssa.blocks[0].terminal
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
        ssa.blocks[ix(yes.0)?].terminal,
        ssa.blocks[ix(no.0)?].terminal,
    ) else {
        return Ok(None);
    };
    if join != other
        || join == yes
        || join == no
        || join == BlockId(0)
        || !matches!(ssa.blocks[ix(join.0)?].terminal, Terminal::Return(_))
    {
        return Ok(None);
    }
    if ssa.edges.len() != 4 {
        return Ok(None);
    }
    let RawPrintable::Method(raw) = input.raw() else {
        return Err(RequestError::InternalInvariant(
            "diamond lost its original method",
        ));
    };
    if !std::ptr::eq(
        raw.layout().original().instructions(),
        ssa.raw.instructions(),
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
    let mut locals = Vec::new();
    for (index, definition) in ssa.values.iter().enumerate() {
        budget.charge_work(1)?;
        let Some(kind) = ssa.kinds[index] else {
            return Ok(None);
        };
        let parameter = match definition {
            Definition::Parameter(index) => Some(*index),
            _ => None,
        };
        append(
            &mut locals,
            Local {
                id: LocalId(u32::try_from(index).map_err(|_| RequestError::PlatformCapacity)?),
                kind,
                parameter,
            },
            budget,
        )?;
    }
    let condition = LocalId(
        ssa.bound[0]
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
    if matches!(condition_kind, Kind::Boolean | Kind::Object) && !matches!(opcode, 0x38 | 0x39) {
        return Ok(None);
    }
    let returned = LocalId(
        ssa.bound[ix(join.0)?]
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
    copies(ssa, None, BlockId(0), &mut entry, &mut locals, budget)?;
    operations(ssa, BlockId(0), &mut entry, budget)?;
    let mut yes_statements = Vec::new();
    copies(
        ssa,
        Some(edge(ssa, BlockId(0), yes, budget)?),
        yes,
        &mut yes_statements,
        &mut locals,
        budget,
    )?;
    operations(ssa, yes, &mut yes_statements, budget)?;
    copies(
        ssa,
        Some(edge(ssa, yes, join, budget)?),
        join,
        &mut yes_statements,
        &mut locals,
        budget,
    )?;
    let mut no_statements = Vec::new();
    copies(
        ssa,
        Some(edge(ssa, BlockId(0), no, budget)?),
        no,
        &mut no_statements,
        &mut locals,
        budget,
    )?;
    operations(ssa, no, &mut no_statements, budget)?;
    copies(
        ssa,
        Some(edge(ssa, no, join, budget)?),
        join,
        &mut no_statements,
        &mut locals,
        budget,
    )?;
    // Join operations are a sequence after both incoming copy plans. The first
    // profile accepts a return-only join, not a skipped operation list.
    for op in &ssa.bound[ix(join.0)?].ops {
        budget.charge_work(1)?;
        if !matches!(op.op, Op::Nop) {
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
    header.push(ssa.result.spelling(), budget)?;
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
    for (index, kind) in ssa.parameters.iter().enumerate() {
        budget.charge_work(1)?;
        if index != 0 {
            header.push(", ", budget)?;
        }
        header.push(kind.spelling(), budget)?;
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
