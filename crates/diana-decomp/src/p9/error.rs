use thiserror::Error;
#[derive(Debug, Error)]
pub enum RenderError {
    #[error("output allocation failed: {0}")]
    Allocation(#[source] std::collections::TryReserveError),
    #[error("output size is not representable on this platform")]
    Capacity,
    #[error("method rendering scratch allocation failed: {0}")]
    MethodScratch(#[source] std::collections::TryReserveError),
    #[error("BUG in output buffer: {0}")]
    OutputInvariant(&'static str),
    #[error("cannot retain method {method:?} rendering diagnostic ({cause}): {allocation}")]
    DiagnosticRetention {
        method: crate::MethodId,
        cause: LocalRenderError,
        #[source]
        allocation: std::collections::TryReserveError,
    },
    #[error("BUG: {0}")]
    Invariant(&'static str),
}
impl RenderError {
    pub fn kind(&self) -> crate::FailureKind {
        match self {
            Self::Allocation(_)
            | Self::Capacity
            | Self::MethodScratch(_)
            | Self::DiagnosticRetention { .. } => crate::FailureKind::ResourceExhausted,
            Self::Invariant(_) | Self::OutputInvariant(_) => crate::FailureKind::InternalInvariant,
        }
    }
}
#[derive(Debug, Error)]
pub enum LocalRenderError {
    #[error("BUG: {0}")]
    Invariant(&'static str),
    #[error("method rendering scratch allocation failed: {0}")]
    Scratch(#[source] std::collections::TryReserveError),
}
impl LocalRenderError {
    pub fn kind(&self) -> crate::FailureKind {
        match self {
            Self::Invariant(_) => crate::FailureKind::InternalInvariant,
            Self::Scratch(_) => crate::FailureKind::ResourceExhausted,
        }
    }
}
#[derive(Debug, Error)]
pub enum OutputError {
    #[error("text output failed: {0}")]
    Io(#[source] std::io::Error),
    #[error("JSON output failed: {0}")]
    Json(#[source] serde_json::Error),
}
