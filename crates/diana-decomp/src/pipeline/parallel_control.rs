use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use super::control::{ControlError, RunContext};

pub(super) struct SharedCancellation {
    cancelled: AtomicBool,
}
impl SharedCancellation {
    pub(super) fn checkpoint(
        &self,
        callback: &(dyn Fn() -> bool + Sync),
    ) -> Result<(), ControlError> {
        if self.cancelled.load(Ordering::Relaxed) || callback() {
            self.cancelled.store(true, Ordering::Relaxed);
            return Err(ControlError::Cancelled);
        }
        Ok(())
    }
}

/// Scoped workers share a latched cancellation flag and thread-safe callback.
#[derive(Clone)]
pub(crate) struct ParallelControl<'c> {
    callback: &'c (dyn Fn() -> bool + Sync),
    shared: Arc<SharedCancellation>,
}
impl<'c> ParallelControl<'c> {
    pub(crate) fn worker(&self) -> RunContext<'c> {
        let mut control = RunContext::new_parallel(self.callback);
        control.shared = Some(Arc::clone(&self.shared));
        control
    }
}
impl<'c> RunContext<'c> {
    pub(crate) fn supports_parallel(&self) -> bool {
        self.parallel_callback.is_some()
    }

    pub(crate) fn parallel_control(&mut self) -> Result<Option<ParallelControl<'c>>, ControlError> {
        self.checkpoint()?;
        let Some(callback) = self.parallel_callback else {
            return Ok(None);
        };
        let shared = self.shared.clone().unwrap_or_else(|| {
            Arc::new(SharedCancellation {
                cancelled: AtomicBool::new(false),
            })
        });
        Ok(Some(ParallelControl { callback, shared }))
    }

    /// Joining finishes before request cancellation is checked for publication.
    pub(crate) fn finish_parallel(
        &mut self,
        parallel: &ParallelControl<'_>,
    ) -> Result<(), ControlError> {
        if parallel.shared.cancelled.load(Ordering::Relaxed) {
            self.cancelled = true;
        }
        self.checkpoint()
    }
}
