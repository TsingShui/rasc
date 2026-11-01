use crate::p8_java::dispatcher::{Case, Dispatcher, Transfer};
use crate::p8_java::structured::StructuredBody;
use crate::pipeline::branch::BlockId;
use crate::substrate::owned::TextBuffer;
use crate::{MethodBudget, RequestError};
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
    output: &mut TextBuffer,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<(), RequestError> {
    for &id in blocks {
        budget.charge_work(1)?;
        let block = case(storage, id)?;
        super::dispatcher::statements(block, output, budget)?;
        match &block.transfer {
            Transfer::Jump(edge) => super::java::assignments(&edge.copies, output, budget)?,
            Transfer::Return(value) => super::java::returned(*value, output, budget)?,
            Transfer::ReturnVoid => output.push("return;\n", budget)?,
            Transfer::Throw(_) | Transfer::Rethrow { .. } => {
                super::java::thrown(&block.transfer, output, budget)?
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
    edge: &crate::p8_java::dispatcher::Edge,
    header: BlockId,
    exit: BlockId,
    next: BlockId,
    stub: Option<&Case>,
    output: &mut TextBuffer,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<(), RequestError> {
    super::java::assignments(&edge.copies, output, budget)?;
    if let Some(stub) = stub {
        if edge.target != stub.key {
            return Err(RequestError::InternalInvariant(
                "exit stub changed source edge",
            ));
        }
        super::dispatcher::statements(stub, output, budget)?;
        let Transfer::Jump(tail) = &stub.transfer else {
            return Err(RequestError::InternalInvariant("exit stub lost transfer"));
        };
        if tail.target != exit {
            return Err(RequestError::InternalInvariant(
                "exit stub changed lexical scope",
            ));
        }
        super::java::assignments(&tail.copies, output, budget)?;
        output.push("break loop0;\n", budget)
    } else if edge.target == exit {
        output.push("break loop0;\n", budget)
    } else if edge.target == header {
        output.push("continue loop0;\n", budget)
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
    edge: &crate::p8_java::dispatcher::Edge,
    arm: Option<&crate::p7_regions::Arm>,
    join: BlockId,
    output: &mut TextBuffer,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<(), RequestError> {
    budget.charge_work(1)?;
    super::java::assignments(&edge.copies, output, budget)?;
    if let Some(arm) = arm {
        if edge.target != arm.block {
            return Err(RequestError::InternalInvariant("diamond changed arm entry"));
        }
        let block = case(storage, arm.block)?;
        super::dispatcher::statements(block, output, budget)?;
        let Transfer::Jump(exit) = &block.transfer else {
            return Err(RequestError::InternalInvariant("diamond arm lost jump"));
        };
        if exit.id != arm.exit || exit.target != join {
            return Err(RequestError::InternalInvariant("diamond changed arm exit"));
        }
        super::java::assignments(&exit.copies, output, budget)?;
    } else if edge.target != join {
        return Err(RequestError::InternalInvariant(
            "empty diamond arm changed join",
        ));
    }
    Ok(())
}
pub(super) fn write(
    body: &StructuredBody,
    type_names: &crate::p8_java::types::TypeNames,
    output: &mut TextBuffer,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<(), RequestError> {
    let storage = &body.storage;
    match &body.region {
        crate::p7_regions::Region::Loop(region) => {
            write_loop(storage, region, type_names, output, budget)
        }
        crate::p7_regions::Region::Diamond(region) => {
            super::dispatcher::locals(storage, type_names, output, budget)?;
            let header = case(storage, region.block)?;
            super::dispatcher::statements(header, output, budget)?;
            let Transfer::Branch { yes, no, .. } = &header.transfer else {
                return Err(RequestError::InternalInvariant(
                    "diamond header lost condition",
                ));
            };
            super::dispatcher::open_if(&header.transfer, output, budget)?;
            diamond_edge(
                storage,
                yes,
                region.yes.as_ref(),
                region.join,
                output,
                budget,
            )?;
            output.push("} else {\n", budget)?;
            diamond_edge(storage, no, region.no.as_ref(), region.join, output, budget)?;
            output.push("}\n", budget)?;
            trace(storage, &[region.join], output, budget)
        }
    }
}
fn write_loop(
    storage: &Dispatcher,
    region: &crate::p7_regions::NaturalLoop,
    type_names: &crate::p8_java::types::TypeNames,
    output: &mut TextBuffer,
    budget: &mut MethodBudget<'_, '_>,
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
    super::dispatcher::locals(storage, type_names, output, budget)?;
    trace(storage, &region.prelude, output, budget)?;
    output.push("loop0: while (true) {\n", budget)?;
    super::dispatcher::statements(header, output, budget)?;
    super::dispatcher::open_if(&header.transfer, output, budget)?;
    super::java::assignments(&yes.copies, output, budget)?;
    if !region.body_yes {
        output.push("break loop0;\n", budget)?;
    }
    output.push("} else {\n", budget)?;
    super::java::assignments(&no.copies, output, budget)?;
    if region.body_yes {
        output.push("break loop0;\n", budget)?;
    }
    output.push("}\n", budget)?;
    let mut branches = region.branches.iter();
    for (at, &id) in region.body.iter().enumerate() {
        budget.charge_work(1)?;
        let block = case(storage, id)?;
        let next = region.body.get(at + 1).copied().unwrap_or(region.header);
        super::dispatcher::statements(block, output, budget)?;
        match &block.transfer {
            Transfer::Jump(edge) => {
                if edge.target != next {
                    return Err(RequestError::InternalInvariant(
                        "natural body changed sequence target",
                    ));
                }
                super::java::assignments(&edge.copies, output, budget)?;
                if next == region.header {
                    output.push("continue loop0;\n", budget)?;
                }
            }
            Transfer::Branch { yes, no, .. } => {
                let Some(branch) = branches.next().filter(|branch| branch.block() == id) else {
                    return Err(RequestError::InternalInvariant(
                        "natural branch lost exit guard",
                    ));
                };
                let guard = match branch {
                    crate::p7_regions::BodyBranch::Exit(guard) => guard,
                    crate::p7_regions::BodyBranch::Diamond(diamond) => {
                        if diamond.join != next
                            || diamond.yes_entry != yes.id
                            || diamond.no_entry != no.id
                        {
                            return Err(RequestError::InternalInvariant(
                                "diamond changed join or entry edges",
                            ));
                        }
                        super::dispatcher::open_if(&block.transfer, output, budget)?;
                        diamond_edge(storage, yes, diamond.yes.as_ref(), next, output, budget)?;
                        output.push("} else {\n", budget)?;
                        diamond_edge(storage, no, diamond.no.as_ref(), next, output, budget)?;
                        output.push("}\n", budget)?;
                        continue;
                    }
                };
                let stub = match guard.stub {
                    Some(id) => Some(case(storage, id)?),
                    None => None,
                };
                super::dispatcher::open_if(&block.transfer, output, budget)?;
                guard_edge(
                    yes,
                    region.header,
                    leave.target,
                    next,
                    if guard.exit == yes.id { stub } else { None },
                    output,
                    budget,
                )?;
                output.push("} else {\n", budget)?;
                guard_edge(
                    no,
                    region.header,
                    leave.target,
                    next,
                    if guard.exit == no.id { stub } else { None },
                    output,
                    budget,
                )?;
                output.push("}\n", budget)?;
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
    output.push("}\n", budget)?;
    trace(storage, &region.tail, output, budget)
}
