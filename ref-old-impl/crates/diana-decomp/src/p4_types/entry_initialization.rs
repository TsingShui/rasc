use crate::p2_flow::Op;
use crate::p2_flow::entry::ParameterFamily;
use crate::p3_ssa::entry::{EntrySsa, Graph};
use crate::p3_ssa::{Definition, ValueId};
use crate::{CodeUnitOffset, MethodId, RequestError, RunContext};

const WORD: u8 = 1;
const REFERENCE: u8 = 2;
const WIDE: u8 = 4;
const ZERO: u8 = 8;
const THIS: u8 = 16;
#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) enum Origin {
    Parameter(u32),
    Word(i32),
    Wide(i64),
}
fn family(graph: &Graph<'_, '_>, origin: Origin) -> Result<u8, RequestError> {
    Ok(match origin {
        Origin::Parameter(0) => THIS,
        Origin::Parameter(index) => match graph.parameter(index)? {
            ParameterFamily::Word => WORD,
            ParameterFamily::Wide => WIDE,
            ParameterFamily::Reference => REFERENCE,
        },
        Origin::Word(0) => ZERO,
        Origin::Word(_) => WORD,
        Origin::Wide(_) => WIDE,
    })
}
pub(super) fn origin(
    graph: &Graph<'_, '_>,
    mut value: ValueId,
    budget: &mut RunContext<'_>,
) -> Result<Option<Origin>, RequestError> {
    // Raw zero can flow through either word moves or object moves until its use is fixed.
    let mut allowed = WORD | REFERENCE | WIDE | ZERO | THIS;
    for _ in 0..graph.len() {
        budget.checkpoint()?;
        let found = match graph.definition(value)? {
            Definition::Parameter(index) => Some(Origin::Parameter(index)),
            Definition::OpResult(id) => {
                let op = graph.operation(id)?;
                match op.op {
                    Op::Constant(_, bits) => Some(Origin::Word(bits)),
                    Op::ConstantWide(_, bits) => Some(Origin::Wide(bits)),
                    Op::Move(..) | Op::MoveWide(..) => {
                        allowed &= match graph.opcode(op)? {
                            1..=3 => WORD | ZERO,
                            4..=6 => WIDE,
                            7..=9 => REFERENCE | ZERO | THIS,
                            _ => {
                                return Err(RequestError::InternalInvariant(
                                    "entry move lost original family",
                                ));
                            }
                        };
                        value = op.input(0)?;
                        continue;
                    }
                    _ => return Ok(None),
                }
            }
            Definition::Phi => {
                let Some(seed) = graph.seed(value)? else {
                    return Ok(None);
                };
                value = seed;
                continue;
            }
            Definition::ExceptionAt(_) => return Ok(None),
        };
        if let Some(found) = found {
            return Ok((allowed & family(graph, found)? != 0).then_some(found));
        }
    }
    Err(RequestError::InternalInvariant(
        "entry definition origin is cyclic",
    ))
}
pub(crate) struct Link {
    _sealed: (),
    method: MethodId,
    target: MethodId,
    pc: CodeUnitOffset,
    kind: super::initialization::ChainKind,
}
impl Link {
    pub(crate) fn method(&self) -> MethodId {
        self.method
    }

    pub(crate) fn target(&self) -> MethodId {
        self.target
    }
    pub(crate) fn pc(&self) -> CodeUnitOffset {
        self.pc
    }
    pub(crate) fn kind(&self) -> super::initialization::ChainKind {
        self.kind
    }
}
pub(crate) fn prove(
    _schedule: &crate::engine::SchedulePermit,
    entry: &EntrySsa<'_>,
    formals: &[ParameterFamily],
    scope: Option<crate::p4_types::declarations::allocation::ConstructorUse>,
    budget: &mut RunContext<'_>,
) -> Result<Option<Link>, RequestError> {
    prove_impl(entry, formals, scope, budget)
}

fn prove_impl(
    entry: &EntrySsa<'_>,
    formals: &[ParameterFamily],
    scope: Option<crate::p4_types::declarations::allocation::ConstructorUse>,
    budget: &mut RunContext<'_>,
) -> Result<Option<Link>, RequestError> {
    let graph = Graph::Entry(entry);
    for op in crate::p3_ssa::entry::operations(entry) {
        budget.checkpoint()?;
        match op.op {
            Op::Nop | Op::Constant(..) | Op::ConstantWide(..) => {}
            Op::Move(..) | Op::MoveWide(..) => {
                if origin(
                    &graph,
                    op.result.ok_or(RequestError::InternalInvariant(
                        "entry move lost definition",
                    ))?,
                    budget,
                )?
                .is_none()
                {
                    return Ok(None);
                }
            }
            Op::Initialize(call) => {
                let site = entry
                    .flow
                    .calls
                    .get(usize::try_from(call.0).map_err(|_| RequestError::PlatformCapacity)?)
                    .ok_or(RequestError::InternalInvariant("entry init lost call"))?;
                if origin(&graph, op.input(0)?, budget)? != Some(Origin::Parameter(0))
                    || op.inputs.len()
                        != formals
                            .len()
                            .checked_add(1)
                            .ok_or(RequestError::PlatformCapacity)?
                {
                    return Ok(None);
                }
                for (&value, formal) in op.inputs.iter().skip(1).zip(formals) {
                    budget.checkpoint()?;
                    let Some(source) = origin(&graph, value, budget)? else {
                        return Ok(None);
                    };
                    let actual = family(&graph, source)?;
                    let accepted = match formal {
                        ParameterFamily::Word => WORD | ZERO,
                        ParameterFamily::Reference => REFERENCE | ZERO,
                        ParameterFamily::Wide => WIDE,
                    };
                    if actual & accepted == 0 {
                        return Ok(None);
                    }
                }
                let relation = if site.owner == entry.flow.owner {
                    crate::p4_types::declarations::allocation::ConstructorUse::This
                } else {
                    crate::p4_types::declarations::allocation::ConstructorUse::Super
                };
                if scope != Some(relation) {
                    return Ok(None);
                }
                return Ok(Some(Link {
                    _sealed: (),
                    method: entry.flow.method,
                    target: site.reference,
                    pc: site.pc,
                    kind: if site.owner == entry.flow.owner {
                        super::initialization::ChainKind::This
                    } else {
                        super::initialization::ChainKind::Super
                    },
                }));
            }
            _ => return Ok(None),
        }
    }
    Ok(None)
}
