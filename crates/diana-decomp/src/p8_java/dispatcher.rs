use super::branch::{Assignment, Local, LocalId, copies};
use crate::input::{MethodInput, RawPrintable};
use crate::pipeline::branch::{BlockId, Definition, EdgeId, Kind, Terminal, VerifiedSsa};
use crate::substrate::owned::{TextBuffer, copy_units, reserve};
use crate::{EnvironmentId, MethodBudget, RequestError, VerifiedJavaMethod};

pub(crate) struct Edge {
    pub id: EdgeId,
    pub copies: Vec<Assignment>,
    pub target: BlockId,
}
pub(crate) enum Transfer {
    Switch {
        value: LocalId,
        cases: Vec<(i32, Edge)>,
        default: Edge,
    },
    Return(super::branch::ValueUse),
    Jump(Edge),
    Branch {
        left: LocalId,
        right: Option<LocalId>,
        kind: Kind,
        opcode: u8,
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
    pub locals: Vec<Local>,
    pub entry: Vec<Assignment>,
    pub cases: Vec<Case>,
}
fn ix(id: u32) -> Result<usize, RequestError> {
    usize::try_from(id).map_err(|_| RequestError::PlatformCapacity)
}
fn push<T>(
    values: &mut Vec<T>,
    value: T,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<(), RequestError> {
    reserve(values, 1, budget)?;
    values.push(value);
    Ok(())
}
fn edge(
    ssa: &VerifiedSsa<'_>,
    id: EdgeId,
    source: BlockId,
    target: BlockId,
    locals: &mut Vec<Local>,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<Edge, RequestError> {
    let original = ssa
        .edges
        .get(ix(id.0)?)
        .ok_or(RequestError::InternalInvariant(
            "dispatcher edge is missing",
        ))?;
    if original.source != source || original.target != target || ix(target.0)? >= ssa.blocks.len() {
        return Err(RequestError::InternalInvariant(
            "dispatcher edge differs from the original CFG",
        ));
    }
    let mut output = Vec::new();
    copies(ssa, Some(id), target, &mut output, locals, budget)?;
    Ok(Edge {
        id,
        copies: output,
        target,
    })
}

pub(crate) fn verify(
    input: &MethodInput<'_, '_>,
    ssa: &VerifiedSsa<'_>,
    environment: EnvironmentId,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<Option<VerifiedJavaMethod>, RequestError> {
    let RawPrintable::Method(raw) = input.raw() else {
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
    {
        return Ok(None);
    }
    if !std::ptr::eq(
        raw.layout().original().instructions(),
        ssa.raw.instructions(),
    ) {
        return Err(RequestError::InternalInvariant(
            "dispatcher belongs to another method",
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
        push(
            &mut locals,
            Local {
                id: LocalId(u32::try_from(index).map_err(|_| RequestError::PlatformCapacity)?),
                kind,
                parameter: match definition {
                    Definition::Parameter(index) => Some(*index),
                    _ => None,
                },
            },
            budget,
        )?;
    }
    let mut entry = Vec::new();
    copies(ssa, None, BlockId(0), &mut entry, &mut locals, budget)?;
    let mut cases = Vec::new();
    for (index, block) in ssa.blocks.iter().enumerate() {
        budget.charge_work(1)?;
        let key = BlockId(u32::try_from(index).map_err(|_| RequestError::PlatformCapacity)?);
        let mut outgoing = Vec::new();
        for (index, original) in ssa.edges.iter().enumerate() {
            budget.charge_work(1)?;
            if original.source == key {
                push(
                    &mut outgoing,
                    EdgeId(u32::try_from(index).map_err(|_| RequestError::PlatformCapacity)?),
                    budget,
                )?;
            }
        }
        let Some(statements) = super::calls::operations(ssa, key, budget)? else {
            return Ok(None);
        };
        let bound = &ssa.bound[index];
        let transfer = match block.terminal {
            Terminal::Switch { table, .. } => {
                let table =
                    ssa.switches
                        .get(ix(table.0)?)
                        .ok_or(RequestError::InternalInvariant(
                            "switch has a foreign payload table",
                        ))?;
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
                    return Ok(None);
                }
                let mut cases = Vec::new();
                for (index, &(case_key, target)) in table.cases.iter().enumerate() {
                    budget.charge_work(1)?;
                    push(
                        &mut cases,
                        (
                            case_key,
                            edge(ssa, outgoing[index], key, target, &mut locals, budget)?,
                        ),
                        budget,
                    )?;
                }
                let default = edge(
                    ssa,
                    outgoing[table.cases.len()],
                    key,
                    table.default,
                    &mut locals,
                    budget,
                )?;
                Transfer::Switch {
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
                Transfer::Jump(edge(ssa, outgoing[0], key, target, &mut locals, budget)?)
            }
            Terminal::Test {
                opcode, yes, no, ..
            } => {
                if outgoing.len() != 2 {
                    return Err(RequestError::InternalInvariant(
                        "branch edge coverage is not exact",
                    ));
                }
                let left = LocalId(
                    bound
                        .terminal
                        .ok_or(RequestError::InternalInvariant(
                            "dispatcher condition lost its value",
                        ))?
                        .0,
                );
                let right = bound.other.map(|id| LocalId(id.0));
                let kind = locals[ix(left.0)?].kind;
                if kind == Kind::Float {
                    return Ok(None);
                }
                if let Some(right) = right
                    && locals[ix(right.0)?].kind != kind
                {
                    return Ok(None);
                }
                if matches!(kind, Kind::Boolean | Kind::Object)
                    && !matches!(opcode, 0x32 | 0x33 | 0x38 | 0x39)
                {
                    return Ok(None);
                }
                Transfer::Branch {
                    left,
                    right,
                    kind,
                    opcode,
                    yes: edge(ssa, outgoing[0], key, yes, &mut locals, budget)?,
                    no: edge(ssa, outgoing[1], key, no, &mut locals, budget)?,
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
                "checked dispatcher identifier is not scalar",
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
    let body = Dispatcher {
        locals,
        entry,
        cases,
    };
    super::dispatcher_verify::verify(ssa, &body, budget)?;
    Ok(Some(VerifiedJavaMethod::dispatcher(
        header.finish(),
        body,
        environment,
    )))
}
