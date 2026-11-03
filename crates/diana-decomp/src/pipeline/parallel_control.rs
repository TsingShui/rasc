use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use super::control::{ControlError, Limits, Quota, RequestLimit, RunContext};

pub(super) struct RequestState {
    pub(super) work: Quota,
    pub(super) allocation: Quota,
    pub(super) metadata: Quota,
    terminal: Option<ControlError>,
}
pub(super) struct SharedRequest {
    pub(super) state: Mutex<RequestState>,
    cancelled: AtomicBool,
    bounded: bool,
}
impl SharedRequest {
    pub(super) fn new(control: &RunContext<'_>) -> Self {
        Self {
            state: Mutex::new(RequestState {
                work: control.work,
                allocation: control.allocation,
                metadata: control.metadata,
                terminal: None,
            }),
            cancelled: AtomicBool::new(false),
            bounded: control.limits.max_request_work_steps.is_some()
                || control.limits.max_request_allocation_bytes.is_some()
                || control.limits.max_metadata_items.is_some(),
        }
    }
    pub(super) fn bounded(&self) -> bool {
        self.bounded
    }
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
    pub(super) fn latch_cancelled(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }
    pub(super) fn update<T>(
        &self,
        operation: impl FnOnce(&mut RequestState) -> Result<T, ControlError>,
    ) -> Result<T, ControlError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| ControlError::InvalidReservation {
                resource: RequestLimit::MaxRequestWorkSteps,
            })?;
        if let Some(error) = state.terminal {
            return Err(error);
        }
        match operation(&mut state) {
            Ok(value) => Ok(value),
            Err(error) => {
                state.terminal = Some(error);
                Err(error)
            }
        }
    }
}

/// Worker construction carries the shared request ledger and thread-safe
/// cancellation source, never an independent request limit.
#[derive(Clone)]
pub(crate) struct ParallelControl<'c> {
    limits: Limits,
    callback: &'c (dyn Fn() -> bool + Sync),
    shared: Arc<SharedRequest>,
}
impl<'c> ParallelControl<'c> {
    pub(crate) fn worker(&self) -> RunContext<'c> {
        let mut control = RunContext::new_parallel(self.limits, self.callback);
        control.work = Quota::new(None);
        control.allocation = Quota::new(None);
        control.metadata = Quota::new(None);
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
        let shared = Arc::new(SharedRequest::new(self));
        Ok(Some(ParallelControl {
            limits: self.limits,
            callback,
            shared,
        }))
    }

    /// Workers have finished before their usage is committed. Bounded workers
    /// already charged the shared quotas; unbounded workers retain local fast
    /// ledgers, whose exact totals are committed here without numeric caps.
    pub(crate) fn finish_parallel(
        &mut self,
        parallel: &ParallelControl<'_>,
        usage: &[super::control::ResourceUsage],
    ) -> Result<(), ControlError> {
        let shared = &parallel.shared;
        if shared.bounded() {
            let state = shared
                .state
                .lock()
                .map_err(|_| ControlError::InvalidReservation {
                    resource: RequestLimit::MaxRequestWorkSteps,
                })?;
            self.work = state.work;
            self.allocation = state.allocation;
            self.metadata = state.metadata;
            if let Some(error) = state.terminal {
                return Err(error);
            }
        } else {
            for usage in usage {
                self.work = self
                    .work
                    .spend(usage.work_steps, RequestLimit::MaxRequestWorkSteps)?;
                self.allocation = self.allocation.spend(
                    usage.allocation_bytes,
                    RequestLimit::MaxRequestAllocationBytes,
                )?;
                self.metadata = self
                    .metadata
                    .spend(usage.metadata_items, RequestLimit::MaxMetadataItems)?;
            }
        }
        if shared.cancelled.load(Ordering::Relaxed) {
            self.cancelled = true;
        }
        self.checkpoint()
    }
}
