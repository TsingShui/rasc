use super::source_control::Step;
use crate::p6_regions::protection::{NodeId, Protection};
use crate::p7_java::dispatcher::Dispatcher;
use crate::substrate::owned::push;
use crate::{RequestError, RunContext};

pub(super) fn membership(
    storage: &Dispatcher,
    protection: &Protection,
    root: NodeId,
    steps: &[Step],
    budget: &mut RunContext<'_>,
) -> Result<(), RequestError> {
    let broken =
        || RequestError::InternalInvariant("final source protection changed enclosed blocks");
    let blocks = crate::p6_regions::protection::blocks(&protection.nodes, &[root], budget)?;
    let mut membership = Vec::new();
    for _ in &storage.cases {
        budget.checkpoint()?;
        push(&mut membership, false, budget)?;
    }
    for block in blocks {
        budget.checkpoint()?;
        let at = usize::try_from(block.0).map_err(|_| RequestError::PlatformCapacity)?;
        *membership.get_mut(at).ok_or_else(broken)? = true;
    }
    for step in steps {
        budget.checkpoint()?;
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
        budget.checkpoint()?;
        if present {
            return Err(broken());
        }
    }
    Ok(())
}
