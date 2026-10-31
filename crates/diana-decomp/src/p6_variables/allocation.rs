use crate::p4_calls::allocation::{AdjacentAllocation, AllocationId};
use crate::pipeline::call_chain::ValueId;
use crate::{CodeUnitOffset, RequestError};

pub(crate) struct DeferredAllocation {
    _sealed: (),
    pub allocation: AllocationId,
    pub init: CodeUnitOffset,
    pub value: ValueId,
    pub receiver: crate::RegisterId,
}
pub(crate) fn bind(
    pair: &AdjacentAllocation,
    value: ValueId,
) -> Result<DeferredAllocation, RequestError> {
    if pair.id.0.0 >= pair.init.0 {
        return Err(RequestError::InternalInvariant(
            "deferred allocation has no normal init completion",
        ));
    }
    Ok(DeferredAllocation {
        _sealed: (),
        allocation: pair.id,
        init: pair.init,
        value,
        receiver: pair.receiver,
    })
}
