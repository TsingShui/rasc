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
    if !matches!(&case(storage,last)?.transfer,Transfer::Jump(edge) if edge.id==region.latch&&edge.target==region.header)
    {
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
    trace(storage, &region.body, output, budget)?;
    output.push("continue;\n}\n", budget)?;
    trace(storage, &region.tail, output, budget)
}
