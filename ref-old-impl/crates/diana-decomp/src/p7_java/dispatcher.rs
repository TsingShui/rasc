use super::branch::{Assignment, Local, LocalId, copies};
use crate::input::MethodSource;
use crate::p2_flow::{BlockId, EdgeId, Terminal};
use crate::p4_types::Kind;
use crate::p5_variables::StorageBody;
use crate::substrate::owned::{TextBuffer, copy_units, reserve};
use crate::{JavaMethod, RequestError, RunContext};

pub(crate) struct Edge {
    pub source: BlockId,
    pub id: EdgeId,
    pub copies: Vec<Assignment>,
    pub target: BlockId,
}
pub(crate) enum Transfer {
    ReturnVoid,
    InitializerExit,
    Throw(super::branch::ValueUse),
    Rethrow {
        value: super::branch::ValueUse,
        handler: BlockId,
    },
    Switch {
        form: crate::p6_regions::SwitchForm,
        value: LocalId,
        cases: Vec<(i32, Edge)>,
        default: Edge,
    },
    Return(super::branch::ValueUse),
    Jump(Edge),
    Branch {
        condition: super::conditions::Condition,
        yes: Edge,
        no: Edge,
    },
}
pub(crate) struct Case {
    pub key: BlockId,
    pub statements: Vec<super::calls::Statement>,
    pub transfer: Transfer,
}
pub(crate) struct Dispatcher {
    pub prologue: Vec<super::constructor_entry::Prologue>,
    pub locals: Vec<Local>,
    pub entry: Vec<Assignment>,
    pub cases: Vec<Case>,
}
fn ix(id: u32) -> Result<usize, RequestError> {
    usize::try_from(id).map_err(|_| RequestError::PlatformCapacity)
}
fn push<T>(values: &mut Vec<T>, value: T, budget: &mut RunContext<'_>) -> Result<(), RequestError> {
    reserve(values, 1, budget)?;
    values.push(value);
    Ok(())
}
fn edge(
    storage: &StorageBody<'_>,
    id: EdgeId,
    source: BlockId,
    target: BlockId,
    budget: &mut RunContext<'_>,
) -> Result<Edge, RequestError> {
    let ssa = &storage.typed;
    let original = ssa
        .ssa
        .edges
        .get(ix(id.0)?)
        .ok_or(RequestError::InternalInvariant(
            "dispatcher edge is missing",
        ))?;
    if original.source != source
        || original.target != target
        || ix(target.0)? >= ssa.ssa.blocks.len()
    {
        return Err(RequestError::InternalInvariant(
            "dispatcher edge differs from the original CFG",
        ));
    }
    let mut output = Vec::new();
    copies(storage, Some(id), target, &mut output, budget)?;
    Ok(Edge {
        source,
        id,
        copies: output,
        target,
    })
}

#[derive(Default)]
pub(super) struct LoweringChecks {
    pub refused: Option<&'static str>,
    pub uncertainties: Vec<crate::OutputReason>,
}

pub(super) fn lower(
    input: &MethodSource<'_, '_>,
    catalog: &crate::context::Context,
    plan: crate::p6_regions::PlannedBody<'_>,
    type_names: &super::types::TypeNames,
    calls: &mut super::calls::SourceCalls,
    budget: &mut RunContext<'_>,
    checks: &mut LoweringChecks,
) -> Result<Option<JavaMethod>, RequestError> {
    let plan = plan.into_data();
    let storage = &plan.storage;
    let placements = super::exceptional::obligations(storage, budget)?;
    if !placements.is_empty() && !matches!(plan.shape, crate::p6_regions::Shape::Protected(Some(_)))
    {
        checks.refused = Some("placements_without_protected");
        return Ok(None);
    }
    let ssa = &storage.typed;
    let Some(raw) = input.decoded() else {
        return Err(RequestError::InternalInvariant(
            "dispatcher has no original method",
        ));
    };
    if !raw
        .layout()
        .original()
        .try_byte_range()
        .as_range()
        .is_empty()
        && !matches!(plan.shape, crate::p6_regions::Shape::Protected(Some(_)))
    {
        checks.refused = Some("abrupt_initializer");
        return Ok(None);
    }
    if !std::ptr::eq(
        raw.layout().original().instructions(),
        ssa.ssa.raw.instructions(),
    ) {
        return Err(RequestError::InternalInvariant(
            "dispatcher belongs to another method",
        ));
    }

    let flags = input.access_flags();
    let initializer = super::initializers::is_initializer(input, budget)?;
    if initializer && !super::initializers::header_admitted(input, budget)? {
        checks.refused = Some("initializer_header");
        return Ok(None);
    }
    let expected_monitor = if flags & 0x20 == 0 {
        None
    } else if flags & 8 != 0 {
        Some(crate::p2_flow::MethodMonitor::DeclaringClass)
    } else {
        Some(crate::p2_flow::MethodMonitor::This)
    };
    if ssa.ssa.method_monitor != expected_monitor {
        return Err(RequestError::InternalInvariant(
            "method monitor changed original flags",
        ));
    }
    let constructor = matches!(ssa.ssa.frame, crate::p2_flow::MethodFrame::Constructor(_));
    if (!constructor
        && (flags & 8 != 0) != matches!(ssa.ssa.frame, crate::p2_flow::MethodFrame::Static))
        || if constructor {
            flags
                & !(crate::substrate::flags::PUBLIC
                    | crate::substrate::flags::PRIVATE
                    | crate::substrate::flags::PROTECTED
                    | crate::substrate::flags::STATIC
                    | crate::substrate::flags::FINAL
                    | crate::substrate::flags::SYNTHETIC
                    | crate::substrate::flags::CONSTRUCTOR)
                != 0
                || flags & crate::substrate::flags::CONSTRUCTOR == 0
        } else if initializer {
            false
        } else {
            !crate::substrate::flags::method_valid(flags) || flags & 0x1000 != 0 && flags & 8 == 0
        }
        || !crate::substrate::flags::visibility_valid(flags)
    {
        checks.refused = Some("method_flags");
        return Ok(None);
    }
    // Ownership metadata is only needed for nested classes.
    let owner = copy_units(input.owner(), budget)?;
    let nested_scope = if owner.contains(&u16::from(b'$')) {
        crate::p4_types::declarations::nested_scope::collect(
            input.program(),
            input.class_id(),
            catalog,
            budget,
        )?
    } else {
        None
    };
    let name = if constructor && nested_scope.is_some() {
        let Some(scope) = &nested_scope else {
            checks.refused = Some("nested_scope_name");
            return Ok(None);
        };
        let mut name = Vec::new();
        for &unit in scope.name() {
            crate::substrate::owned::push(&mut name, unit, budget)?;
        }
        name
    } else if constructor {
        let owner = copy_units(input.owner(), budget)?;
        if owner.len() < 3 {
            return Err(RequestError::InternalInvariant(
                "constructor lost owner descriptor",
            ));
        }
        let start = owner
            .iter()
            .rposition(|&unit| unit == 47)
            .map_or(1, |index| index + 1);
        let mut name = Vec::new();
        for &unit in &owner[start..owner.len() - 1] {
            budget.checkpoint()?;
            crate::substrate::owned::push(&mut name, unit, budget)?;
        }
        name
    } else {
        crate::substrate::owned::copy_slice_units(input.declaration().name_units(), budget)?
    };
    if !initializer && !super::declarations::identifier(&name, budget)? {
        checks.refused = Some("identifier_name");
        return Ok(None);
    }
    let mut catch_names = Vec::new();
    if let crate::p6_regions::Shape::Protected(Some(region)) = &plan.shape {
        let Some(names) = super::protected::names(
            input,
            catalog,
            storage,
            region,
            &mut checks.uncertainties,
            budget,
        )?
        else {
            checks.refused = Some("catch_names");
            return Ok(None);
        };
        catch_names = names;
    }
    let mut prologue = Vec::new();
    if let Some(entry) = super::constructor_entry::lower(storage, calls, budget)? {
        push(&mut prologue, entry, budget)?;
    }
    let locals = super::local_declarations::lower(storage, type_names, budget)?;
    let mut entry = Vec::new();
    copies(storage, None, BlockId(0), &mut entry, budget)?;
    let mut cases = Vec::new();
    for (index, block) in ssa.ssa.blocks.iter().enumerate() {
        budget.checkpoint()?;
        let key = BlockId(u32::try_from(index).map_err(|_| RequestError::PlatformCapacity)?);
        let mut outgoing = Vec::new();
        for (index, original) in ssa.ssa.edges.iter().enumerate() {
            budget.checkpoint()?;
            if original.source == key {
                push(
                    &mut outgoing,
                    EdgeId(u32::try_from(index).map_err(|_| RequestError::PlatformCapacity)?),
                    budget,
                )?;
            }
        }
        let Some(statements) =
            super::calls::operations(storage, key, calls, type_names, &placements, budget)?
        else {
            checks.refused = Some("call_operations");
            return Ok(None);
        };
        let bound = &ssa.ssa.bound[index];
        let transfer =
            match block.terminal {
                Terminal::Throw(_) => {
                    if !outgoing.is_empty() {
                        return Err(RequestError::InternalInvariant(
                            "throw acquired a normal edge",
                        ));
                    }
                    let value = bound
                        .terminal
                        .ok_or(RequestError::InternalInvariant("throw lost operand"))?;
                    let conversion = ssa
                        .thrown_uses
                        .get(index)
                        .and_then(|usage| *usage)
                        .ok_or(RequestError::InternalInvariant("throw lost typed use"))?;
                    let usage = super::values::lower(LocalId(value.0), conversion);
                    match super::exceptional::exception_root(ssa, value, budget)? {
                        Some(handler) => Transfer::Rethrow {
                            value: usage,
                            handler,
                        },
                        None => Transfer::Throw(usage),
                    }
                }
                Terminal::ReturnVoid => {
                    if !outgoing.is_empty()
                        || !matches!(ssa.ssa.result, crate::p2_flow::ResultType::Void)
                    {
                        return Err(RequestError::InternalInvariant(
                            "void exit changed its original contract",
                        ));
                    }
                    if initializer {
                        Transfer::InitializerExit
                    } else {
                        Transfer::ReturnVoid
                    }
                }
                Terminal::Switch { table, .. } => {
                    let form = *plan
                        .switches
                        .get(ix(table.0)?)
                        .ok_or(RequestError::InternalInvariant("switch lost P7 policy"))?;
                    let table = ssa.ssa.switches.get(ix(table.0)?).ok_or(
                        RequestError::InternalInvariant("switch has a foreign payload table"),
                    )?;
                    if outgoing.len()
                        != table
                            .cases
                            .len()
                            .checked_add(1)
                            .ok_or(RequestError::PlatformCapacity)?
                    {
                        return Err(RequestError::InternalInvariant(
                            "switch edge coverage is not exact",
                        ));
                    }
                    let value = LocalId(
                        bound
                            .terminal
                            .ok_or(RequestError::InternalInvariant("switch lost its selector"))?
                            .0,
                    );
                    if locals[ix(value.0)?].kind != Kind::Int {
                        checks.refused = Some("dispatcher_value_kind");
                        return Ok(None);
                    }
                    let mut cases = Vec::new();
                    for (index, &(case_key, target)) in table.cases.iter().enumerate() {
                        budget.checkpoint()?;
                        push(
                            &mut cases,
                            (
                                case_key,
                                edge(storage, outgoing[index], key, target, budget)?,
                            ),
                            budget,
                        )?;
                    }
                    let default = edge(
                        storage,
                        outgoing[table.cases.len()],
                        key,
                        table.default,
                        budget,
                    )?;
                    Transfer::Switch {
                        form,
                        value,
                        cases,
                        default,
                    }
                }
                Terminal::Return(_) => {
                    if !outgoing.is_empty() {
                        return Err(RequestError::InternalInvariant(
                            "return has an outgoing dispatcher edge",
                        ));
                    }
                    let value = LocalId(
                        bound
                            .terminal
                            .ok_or(RequestError::InternalInvariant(
                                "dispatcher return lost its value",
                            ))?
                            .0,
                    );
                    let Some(value) = super::branch::returned(ssa, value)? else {
                        checks.refused = Some("returned_use");
                        return Ok(None);
                    };
                    Transfer::Return(value)
                }
                Terminal::Jump(target) => {
                    if outgoing.len() != 1 {
                        return Err(RequestError::InternalInvariant(
                            "jump edge coverage is not exact",
                        ));
                    }
                    Transfer::Jump(edge(storage, outgoing[0], key, target, budget)?)
                }
                Terminal::Test { yes, no, .. } => {
                    if outgoing.len() != 2 {
                        return Err(RequestError::InternalInvariant(
                            "branch edge coverage is not exact",
                        ));
                    }
                    let Some(condition) =
                        super::conditions::lower(ssa, bound, block.terminal, budget)?
                    else {
                        checks.refused = Some("condition_lowering");
                        return Ok(None);
                    };
                    Transfer::Branch {
                        condition,
                        yes: edge(storage, outgoing[0], key, yes, budget)?,
                        no: edge(storage, outgoing[1], key, no, budget)?,
                    }
                }
            };
        push(
            &mut cases,
            Case {
                key,
                statements,
                transfer,
            },
            budget,
        )?;
    }
    // Entry is key zero and every stored target was checked against this exact
    // case domain. The synthetic default is therefore unreachable.
    if cases.is_empty() || cases[0].key != BlockId(0) {
        return Err(RequestError::InternalInvariant(
            "dispatcher has no entry case",
        ));
    }
    let mut header = TextBuffer::new();
    if initializer {
        header.push("static", budget)?;
    } else {
        for (bit, text) in [
            (1, "public "),
            (2, "private "),
            (4, "protected "),
            (8, "static "),
            (0x10, "final "),
            (0x20, "synchronized "),
            (0x800, "strictfp "),
        ] {
            if flags & bit != 0 {
                header.push(text, budget)?;
            }
        }
        if !constructor {
            header.push(type_names.result(ssa.ssa.result, budget)?, budget)?;
            header.push(" ", budget)?;
        }
        for scalar in char::decode_utf16(name.iter().copied()) {
            header.character(
                scalar.map_err(|_| {
                    RequestError::InternalInvariant("checked dispatcher identifier is not scalar")
                })?,
                budget,
            )?;
        }
        header.push("(", budget)?;
        for (index, kind) in ssa
            .ssa
            .parameters
            .iter()
            .skip(ssa.ssa.frame.implicit_words() + usize::from(storage.typed.capture.is_some()))
            .enumerate()
        {
            budget.checkpoint()?;
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
        for (index, descriptor) in calls.throws.descriptors.iter().enumerate() {
            budget.checkpoint()?;
            let Some(name) = super::calls::type_name(descriptor, budget)? else {
                checks.refused = Some("throws_type_name");
                return Ok(None);
            };
            header.push(if index == 0 { " throws " } else { ", " }, budget)?;
            header.push(&name, budget)?;
        }
    }
    let body = Dispatcher {
        prologue,
        locals,
        entry,
        cases,
    };
    super::dispatcher_verify::verify(input, storage, &body, type_names, budget)?;
    let syntax = if initializer {
        let mut normal_exit = false;
        for case in &body.cases {
            budget.checkpoint()?;
            normal_exit |= matches!(case.transfer, Transfer::InitializerExit);
        }
        super::initializers::MemberSyntax::Initializer {
            always_abrupt: !normal_exit,
        }
    } else {
        super::initializers::MemberSyntax::Method
    };
    if let crate::p6_regions::Shape::Protected(Some(region)) = plan.shape {
        let source = super::protected_control::lower(&body, &region, &catch_names, budget)?;
        return Ok(Some(JavaMethod::protected(
            header.finish(),
            super::protected::ProtectedBody {
                storage: body,
                source,
            },
            syntax,
        )));
    }
    if let crate::p6_regions::Shape::Structured(region) = plan.shape {
        let source = super::natural_control::lower(&body, &region, budget)?;
        let method = JavaMethod::structured(
            header.finish(),
            super::structured::StructuredBody {
                storage: body,
                source,
            },
            syntax,
        );
        return Ok(Some(method));
    }
    Ok(Some(JavaMethod::dispatcher(header.finish(), body, syntax)))
}
