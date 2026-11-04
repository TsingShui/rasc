use super::source_control::{Builder, Label, Plan, Step};
use crate::p2_flow::BlockId;
use crate::p7_java::dispatcher::{Case, Dispatcher, Transfer};
use crate::{RequestError, RunContext};
fn case(storage: &Dispatcher, id: BlockId) -> Result<&Case, RequestError> {
    storage
        .cases
        .get(usize::try_from(id.0).map_err(|_| RequestError::PlatformCapacity)?)
        .filter(|case| case.key == id)
        .ok_or(RequestError::InternalInvariant(
            "structured region lost original block",
        ))
}
fn trace(
    storage: &Dispatcher,
    blocks: &[BlockId],
    output: &mut Builder,
    budget: &mut RunContext<'_>,
) -> Result<(), RequestError> {
    for &id in blocks {
        budget.checkpoint()?;
        let block = case(storage, id)?;
        output.statements(block, budget)?;
        match &block.transfer {
            Transfer::Jump(edge) => output.copies(edge, budget)?,
            Transfer::Return(value) => output.returned(*value, budget)?,
            Transfer::ReturnVoid => output.marker(Step::ReturnVoid, budget)?,
            Transfer::InitializerExit => output.marker(Step::InitializerExit, budget)?,
            Transfer::Throw(_) | Transfer::Rethrow { .. } => {
                output.thrown(&block.transfer, budget)?
            }
            _ => {
                return Err(RequestError::InternalInvariant(
                    "linear region changed transfer kind",
                ));
            }
        }
    }
    Ok(())
}
fn guard_edge(
    edge: &crate::p7_java::dispatcher::Edge,
    header: BlockId,
    exit: BlockId,
    next: BlockId,
    stub: Option<&Case>,
    output: &mut Builder,
    budget: &mut RunContext<'_>,
) -> Result<(), RequestError> {
    output.copies(edge, budget)?;
    if let Some(stub) = stub {
        if edge.target != stub.key {
            return Err(RequestError::InternalInvariant(
                "exit stub changed source edge",
            ));
        }
        output.statements(stub, budget)?;
        let Transfer::Jump(tail) = &stub.transfer else {
            return Err(RequestError::InternalInvariant("exit stub lost transfer"));
        };
        if tail.target != exit {
            return Err(RequestError::InternalInvariant(
                "exit stub changed lexical scope",
            ));
        }
        output.copies(tail, budget)?;
        output.marker(Step::Break(Label::Loop0), budget)
    } else if edge.target == exit {
        output.marker(Step::Break(Label::Loop0), budget)
    } else if edge.target == header {
        output.marker(Step::Continue(Label::Loop0), budget)
    } else if edge.target == next {
        Ok(())
    } else {
        Err(RequestError::InternalInvariant(
            "guarded exit changed lexical scope",
        ))
    }
}
fn diamond_edge(
    storage: &Dispatcher,
    edge: &crate::p7_java::dispatcher::Edge,
    arm: Option<&crate::p6_regions::Arm>,
    join: BlockId,
    output: &mut Builder,
    budget: &mut RunContext<'_>,
) -> Result<(), RequestError> {
    budget.checkpoint()?;
    output.copies(edge, budget)?;
    if let Some(arm) = arm {
        if edge.target != arm.block {
            return Err(RequestError::InternalInvariant("diamond changed arm entry"));
        }
        let block = case(storage, arm.block)?;
        output.statements(block, budget)?;
        let Transfer::Jump(exit) = &block.transfer else {
            return Err(RequestError::InternalInvariant("diamond arm lost jump"));
        };
        if exit.id != arm.exit || exit.target != join {
            return Err(RequestError::InternalInvariant("diamond changed arm exit"));
        }
        output.copies(exit, budget)?;
    } else if edge.target != join {
        return Err(RequestError::InternalInvariant(
            "empty diamond arm changed join",
        ));
    }
    Ok(())
}
fn lower_into(
    storage: &Dispatcher,
    region: &crate::p6_regions::Region,
    output: &mut Builder,
    budget: &mut RunContext<'_>,
) -> Result<(), RequestError> {
    match region {
        crate::p6_regions::Region::Sequence(block) => trace(storage, &[*block], output, budget),
        crate::p6_regions::Region::Loop(region) => write_loop(storage, region, output, budget),
        crate::p6_regions::Region::Diamond(region) => {
            let header = case(storage, region.block)?;
            output.statements(header, budget)?;
            let Transfer::Branch { yes, no, .. } = &header.transfer else {
                return Err(RequestError::InternalInvariant(
                    "diamond header lost condition",
                ));
            };
            output.condition(header, budget)?;
            diamond_edge(
                storage,
                yes,
                region.yes.as_ref(),
                region.join,
                output,
                budget,
            )?;
            output.marker(Step::Else, budget)?;
            diamond_edge(storage, no, region.no.as_ref(), region.join, output, budget)?;
            output.marker(Step::End, budget)?;
            trace(storage, &[region.join], output, budget)
        }
    }
}
fn write_loop(
    storage: &Dispatcher,
    region: &crate::p6_regions::NaturalLoop,
    output: &mut Builder,
    budget: &mut RunContext<'_>,
) -> Result<(), RequestError> {
    let header = case(storage, region.header)?;
    let Transfer::Branch { yes, no, .. } = &header.transfer else {
        return Err(RequestError::InternalInvariant(
            "loop header lost condition",
        ));
    };
    let (enter, leave) = if region.body_yes {
        (yes, no)
    } else {
        (no, yes)
    };
    if enter.id != region.enter || leave.id != region.leave {
        return Err(RequestError::InternalInvariant(
            "loop changed header edge identities",
        ));
    }
    let Some(&last) = region.body.last() else {
        return Err(RequestError::InternalInvariant("natural loop lost latch"));
    };
    let latch = match &case(storage, last)?.transfer {
        Transfer::Jump(edge) => Some(edge),
        Transfer::Branch { yes, no, .. } if yes.target == region.header => Some(yes),
        Transfer::Branch { yes, no, .. } if no.target == region.header => Some(no),
        _ => None,
    };
    if !matches!(latch,Some(edge) if edge.id==region.latch&&edge.target==region.header) {
        return Err(RequestError::InternalInvariant(
            "natural loop changed latch",
        ));
    }
    trace(storage, &region.prelude, output, budget)?;
    output.marker(Step::Loop(Label::Loop0), budget)?;
    output.statements(header, budget)?;
    output.condition(header, budget)?;
    output.copies(yes, budget)?;
    if !region.body_yes {
        output.marker(Step::Break(Label::Loop0), budget)?;
    }
    output.marker(Step::Else, budget)?;
    output.copies(no, budget)?;
    if region.body_yes {
        output.marker(Step::Break(Label::Loop0), budget)?;
    }
    output.marker(Step::End, budget)?;
    let mut branches = region.branches.iter();
    for (at, &id) in region.body.iter().enumerate() {
        budget.checkpoint()?;
        let block = case(storage, id)?;
        let next = region.body.get(at + 1).copied().unwrap_or(region.header);
        output.statements(block, budget)?;
        match &block.transfer {
            Transfer::Jump(edge) => {
                if edge.target != next {
                    return Err(RequestError::InternalInvariant(
                        "natural body changed sequence target",
                    ));
                }
                output.copies(edge, budget)?;
                if next == region.header {
                    output.marker(Step::Continue(Label::Loop0), budget)?;
                }
            }
            Transfer::Branch { yes, no, .. } => {
                let Some(branch) = branches.next().filter(|branch| branch.block() == id) else {
                    return Err(RequestError::InternalInvariant(
                        "natural branch lost exit guard",
                    ));
                };
                let guard = match branch {
                    crate::p6_regions::BodyBranch::Exit(guard) => guard,
                    crate::p6_regions::BodyBranch::Diamond(diamond) => {
                        if diamond.join != next
                            || diamond.yes_entry != yes.id
                            || diamond.no_entry != no.id
                        {
                            return Err(RequestError::InternalInvariant(
                                "diamond changed join or entry edges",
                            ));
                        }
                        output.condition(block, budget)?;
                        diamond_edge(storage, yes, diamond.yes.as_ref(), next, output, budget)?;
                        output.marker(Step::Else, budget)?;
                        diamond_edge(storage, no, diamond.no.as_ref(), next, output, budget)?;
                        output.marker(Step::End, budget)?;
                        continue;
                    }
                };
                let stub = match guard.stub {
                    Some(id) => Some(case(storage, id)?),
                    None => None,
                };
                output.condition(block, budget)?;
                guard_edge(
                    yes,
                    region.header,
                    leave.target,
                    next,
                    if guard.exit == yes.id { stub } else { None },
                    output,
                    budget,
                )?;
                output.marker(Step::Else, budget)?;
                guard_edge(
                    no,
                    region.header,
                    leave.target,
                    next,
                    if guard.exit == no.id { stub } else { None },
                    output,
                    budget,
                )?;
                output.marker(Step::End, budget)?;
            }
            _ => {
                return Err(RequestError::InternalInvariant(
                    "natural body lost transfer",
                ));
            }
        }
    }
    if branches.next().is_some() {
        return Err(RequestError::InternalInvariant(
            "natural loop has extra exit guards",
        ));
    }
    output.marker(Step::End, budget)?;
    trace(storage, &region.tail, output, budget)
}

pub(super) fn lower(
    storage: &Dispatcher,
    region: &crate::p6_regions::Region,
    budget: &mut RunContext<'_>,
) -> Result<Plan, RequestError> {
    let mut plan = Builder::new();
    lower_into(storage, region, &mut plan, budget)?;
    plan.finish(storage, None, &[], budget)
}
