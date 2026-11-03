use crate::context::Context;
use crate::input::MethodInput;
use crate::substrate::owned::{copy_units, reserve};
use crate::{MethodBudget, RequestError};

pub(super) fn retain(
    declared: Option<&[Vec<u16>]>,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<Vec<Vec<u16>>, RequestError> {
    let mut result = Vec::new();
    if let Some(declared) = declared {
        for descriptor in declared {
            let mut copy = Vec::new();
            reserve(&mut copy, descriptor.len(), budget)?;
            for &unit in descriptor {
                budget.charge_work(1)?;
                copy.push(unit);
            }
            reserve(&mut result, 1, budget)?;
            result.push(copy);
        }
    }
    Ok(result)
}

/// Original Throws annotation descriptors, not an inferred escaping-exception set.
pub(crate) struct SourceThrows {
    _sealed: (),
    pub descriptors: Vec<Vec<u16>>,
}
pub(crate) fn source_throws(
    input: &MethodInput<'_, '_>,
    _catalog: &Context,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<Option<SourceThrows>, RequestError> {
    let original = input.throws_metadata(budget)?;
    let mut descriptors = Vec::new();
    for exception in original.exceptions {
        budget.charge_work(1)?;
        reserve(&mut descriptors, 1, budget)?;
        descriptors.push(copy_units(exception, budget)?);
    }
    Ok(Some(SourceThrows {
        _sealed: (),
        descriptors,
    }))
}

pub(crate) struct Invocation {
    pub reference: crate::MethodId,
    pub pc: crate::CodeUnitOffset,
}

impl SourceThrows {
    pub(crate) fn retained(&self, budget: &mut MethodBudget<'_, '_>) -> Result<Self, RequestError> {
        Ok(Self {
            _sealed: (),
            descriptors: retain(Some(&self.descriptors), budget)?,
        })
    }
}
