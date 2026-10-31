use crate::input::{InputError, MethodError};
use crate::{ControlError, DebugError, FailureKind};
use std::collections::TryReserveError;
use thiserror::Error;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JavaTargetItem {
    ParameterWords,
    MethodName,
    MethodDescriptor,
}

#[derive(Debug, Error)]
pub enum RequestError {
    #[error(transparent)]
    Input(#[from] InputError),
    #[error(transparent)]
    Method(#[from] MethodError),
    #[error(transparent)]
    Debug(#[from] DebugError),
    #[error(transparent)]
    Control(#[from] ControlError),
    #[error("request allocation failed: {0}")]
    Allocation(#[source] TryReserveError),
    #[error("request capacity is not representable on this platform")]
    PlatformCapacity,
    #[error("requested DEX identity does not exist")]
    UnknownDex,
    #[error("JavaTargetLimit: {item:?} uses {actual}, maximum {maximum}")]
    JavaTargetLimit {
        item: JavaTargetItem,
        actual: u64,
        maximum: u64,
    },
    #[error("BUG: {0}")]
    InternalInvariant(&'static str),
}

impl RequestError {
    pub fn kind(&self) -> FailureKind {
        match self {
            Self::Input(error) => error.kind(),
            Self::Method(error) => error.kind(),
            Self::Debug(error) => error.kind(),
            Self::Control(error) => error.kind(),
            Self::Allocation(_) | Self::PlatformCapacity => FailureKind::BudgetExceeded,
            Self::UnknownDex => FailureKind::Malformed,
            Self::JavaTargetLimit { .. } => FailureKind::Unsupported,
            Self::InternalInvariant(_) => FailureKind::InternalInvariant,
        }
    }
}
