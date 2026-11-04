use super::parallel_control::SharedCancellation;
use std::sync::Arc;
use thiserror::Error;

/// Cancellation and control invariants are independent of input validity.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum ControlError {
    #[error("request cancelled")]
    Cancelled,
    #[error("BUG: {0}")]
    InternalInvariant(&'static str),
}

/// Request cancellation, shared by method workers. Once observed, cancellation
/// remains terminal even if the caller's callback later returns false.
pub struct RunContext<'c> {
    is_cancelled: &'c dyn Fn() -> bool,
    pub(super) cancelled: bool,
    pub(super) parallel_callback: Option<&'c (dyn Fn() -> bool + Sync)>,
    pub(super) shared: Option<Arc<SharedCancellation>>,
}

impl<'c> RunContext<'c> {
    pub fn new(is_cancelled: &'c dyn Fn() -> bool) -> Self {
        Self {
            is_cancelled,
            cancelled: false,
            parallel_callback: None,
            shared: None,
        }
    }

    /// Enables scoped parallel workers using the same cancellation source.
    pub fn new_parallel(callback: &'c (dyn Fn() -> bool + Sync)) -> Self {
        let mut control = Self::new(callback);
        control.parallel_callback = Some(callback);
        control
    }

    pub fn checkpoint(&mut self) -> Result<(), ControlError> {
        if let Some(shared) = &self.shared {
            let callback = self
                .parallel_callback
                .ok_or(ControlError::InternalInvariant(
                    "parallel cancellation callback disappeared",
                ))?;
            return shared.checkpoint(callback);
        }
        if !self.cancelled && (self.is_cancelled)() {
            self.cancelled = true;
        }
        if self.cancelled {
            Err(ControlError::Cancelled)
        } else {
            Ok(())
        }
    }
}

/// Shared readers accept request cancellation without acquiring method-analysis
/// construction authority or owning the caller's cancellation state.
pub(crate) trait InputControl {
    fn checkpoint(&mut self) -> Result<(), ControlError>;
}

impl InputControl for RunContext<'_> {
    fn checkpoint(&mut self) -> Result<(), ControlError> {
        RunContext::checkpoint(self)
    }
}
