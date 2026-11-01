use crate::p8_java::dispatcher::{Case, Dispatcher, Transfer};
use crate::p8_java::structured::StructuredLoop;
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
        output.push("break;\n", budget)
    } else if edge.target == exit {
        output.push("break;\n", budget)
    } else if edge.target == header {
        output.push("continue;\n", budget)
    } else if edge.target == next {
        Ok(())
    } else {
        Err(RequestError::InternalInvariant(
            "guarded exit changed lexical scope",
        ))
    }
}
pub(super) fn write(
    body: &StructuredLoop,
    output: &mut TextBuffer,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<(), RequestError> {
    let storage = &body.storage;
    let region = &body.region;
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
    super::dispatcher::locals(storage, output, budget)?;
    trace(storage, &region.prelude, output, budget)?;
    output.push("while (true) {\n", budget)?;
    super::dispatcher::statements(header, output, budget)?;
    super::dispatcher::open_if(&header.transfer, output, budget)?;
    super::java::assignments(&yes.copies, output, budget)?;
    if !region.body_yes {
        output.push("break;\n", budget)?;
    }
    output.push("} else {\n", budget)?;
    super::java::assignments(&no.copies, output, budget)?;
    if region.body_yes {
        output.push("break;\n", budget)?;
    }
    output.push("}\n", budget)?;
    let mut guards = region.guards.iter();
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
                    output.push("continue;\n", budget)?;
                }
            }
            Transfer::Branch { yes, no, .. } => {
                let Some(guard) = guards.next().filter(|guard| guard.block == id) else {
                    return Err(RequestError::InternalInvariant(
                        "natural branch lost exit guard",
                    ));
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
    if guards.next().is_some() {
        return Err(RequestError::InternalInvariant(
            "natural loop has extra exit guards",
        ));
    }
    output.push("}\n", budget)?;
    trace(storage, &region.tail, output, budget)
}
