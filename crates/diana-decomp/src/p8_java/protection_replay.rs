use super::source_control::Step;
use crate::p7_regions::protection::{NodeId, Protection};
use crate::p8_java::dispatcher::Dispatcher;
use crate::substrate::owned::push;
use crate::{MethodBudget, RequestError};

pub(super) fn membership(
    storage: &Dispatcher,
    protection: &Protection,
    root: NodeId,
    steps: &[Step],
    budget: &mut MethodBudget<'_, '_>,
) -> Result<(), RequestError> {
    let broken =
        || RequestError::InternalInvariant("final source protection changed enclosed blocks");
    let blocks = crate::p7_regions::protection::blocks(&protection.nodes, &[root], budget)?;
    let mut membership = Vec::new();
    for _ in &storage.cases {
        budget.charge_work(1)?;
        push(&mut membership, false, budget)?;
    }
    for block in blocks {
        budget.charge_work(1)?;
        let at = usize::try_from(block.0).map_err(|_| RequestError::PlatformCapacity)?;
        *membership.get_mut(at).ok_or_else(broken)? = true;
    }
    for step in steps {
        budget.charge_work(1)?;
        if let Step::Statements(block) = step {
            let at = usize::try_from(block.0).map_err(|_| RequestError::PlatformCapacity)?;
            let present = membership.get_mut(at).ok_or_else(broken)?;
            if !*present {
                return Err(broken());
            }
            *present = false;
        }
    }
    for present in membership {
        budget.charge_work(1)?;
        if present {
            return Err(broken());
        }
    }
    Ok(())
}
