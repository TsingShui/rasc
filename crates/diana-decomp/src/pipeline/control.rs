use thiserror::Error;

/// Cancellation and control invariants are independent of input validity.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum ControlError {
    #[error("request cancelled")]
    Cancelled,
    #[error("BUG: {0}")]
    InternalInvariant(&'static str),
}

/// Request cancellation. Once observed, cancellation remains terminal even if
/// the caller's callback later returns false.
pub struct RunContext<'c> {
    is_cancelled: &'c dyn Fn() -> bool,
    cancelled: bool,
}

impl<'c> RunContext<'c> {
    pub fn new(is_cancelled: &'c dyn Fn() -> bool) -> Self {
        Self {
            is_cancelled,
            cancelled: false,
        }
    }

    pub fn checkpoint(&mut self) -> Result<(), ControlError> {
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

/// Shared readers accept request cancellation without acquiring analysis
/// construction authority.
pub(crate) trait InputControl {
    fn checkpoint(&mut self) -> Result<(), ControlError>;
}

impl InputControl for RunContext<'_> {
    fn checkpoint(&mut self) -> Result<(), ControlError> {
        RunContext::checkpoint(self)
    }
}
