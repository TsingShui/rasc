use super::debug::DebugIssue;
use crate::input::{InputError, MethodError};
use crate::{ByteOffset, ClassDefId, ControlError, FailureKind, MethodId};
use std::collections::TryReserveError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DebugError {
    #[error(transparent)]
    Input(#[from] InputError),
    #[error(transparent)]
    Method(#[from] MethodError),
    #[error(transparent)]
    Control(#[from] ControlError),
    #[error("debug output capacity is not representable on this platform")]
    PlatformCapacity,
    #[error("debug output allocation failed: {0}")]
    Allocation(#[source] TryReserveError),
    #[error("method {method:?} is not defined in class {class:?}")]
    MethodNotDefined { class: ClassDefId, method: MethodId },
    #[error("malformed original facts {issue:?}; debug output also failed: {resource}")]
    AfterMalformed {
        issue: DebugIssue,
        #[source]
        resource: DebugResourceError,
    },
    #[error("BUG: {0}")]
    InternalInvariant(&'static str),
}

#[derive(Debug, Error)]
pub enum DebugResourceError {
    #[error(transparent)]
    Control(ControlError),
    #[error("debug allocation failed: {0}")]
    Allocation(#[source] TryReserveError),
    #[error("debug capacity is not representable on this platform; field {offset:?}")]
    PlatformCapacity { offset: Option<ByteOffset> },
}

impl DebugResourceError {
    fn from_input(error: InputError) -> Result<Self, InputError> {
        match error {
            InputError::Control(error) => Ok(Self::Control(error)),
            InputError::PlatformCapacity { offset_byte } => Ok(Self::PlatformCapacity {
                offset: Some(offset_byte),
            }),
            other => Err(other),
        }
    }

    fn from_method(error: MethodError) -> Result<Self, MethodError> {
        match error {
            MethodError::Control(error) => Ok(Self::Control(error)),
            MethodError::Allocation(error) => Ok(Self::Allocation(error)),
            MethodError::PlatformCapacity => Ok(Self::PlatformCapacity { offset: None }),
            MethodError::Input(error) => Self::from_input(error).map_err(MethodError::Input),
            other => Err(other),
        }
    }
}

impl DebugError {
    pub(super) fn preserve_malformed(self, issue: Option<DebugIssue>) -> Self {
        let Some(issue) = issue else {
            return self;
        };
        if self.kind() != FailureKind::ResourceExhausted {
            return self;
        }
        let resource = match self {
            Self::Control(error) => Ok(DebugResourceError::Control(error)),
            Self::Allocation(error) => Ok(DebugResourceError::Allocation(error)),
            Self::PlatformCapacity => Ok(DebugResourceError::PlatformCapacity { offset: None }),
            Self::Input(error) => DebugResourceError::from_input(error).map_err(Self::Input),
            Self::Method(error) => DebugResourceError::from_method(error).map_err(Self::Method),
            other => Err(other),
        };
        match resource {
            Ok(resource) => Self::AfterMalformed { issue, resource },
            Err(original) => original,
        }
    }

    pub(super) fn into_method(self) -> MethodError {
        match self {
            Self::Input(error) => MethodError::Input(error),
            Self::Method(error) => error,
            Self::Control(error) => MethodError::Control(error),
            Self::PlatformCapacity => MethodError::PlatformCapacity,
            Self::Allocation(error) => MethodError::Allocation(error),
            Self::InternalInvariant(message) => MethodError::InternalInvariant(message),
            Self::MethodNotDefined { .. } => {
                MethodError::InternalInvariant("selection failed inside a selected instruction")
            }
            Self::AfterMalformed { .. } => MethodError::InternalInvariant(
                "finalized debug failure reentered an instruction writer",
            ),
        }
    }

    pub fn kind(&self) -> FailureKind {
        match self {
            Self::Input(error) => error.kind(),
            Self::Method(error) => error.kind(),
            Self::Control(error) => error.kind(),
            Self::PlatformCapacity | Self::Allocation(_) => FailureKind::ResourceExhausted,
            Self::MethodNotDefined { .. } | Self::AfterMalformed { .. } => FailureKind::Malformed,
            Self::InternalInvariant(_) => FailureKind::InternalInvariant,
        }
    }
}

pub(super) fn kind_name(kind: FailureKind) -> &'static str {
    match kind {
        FailureKind::Malformed => "malformed",
        FailureKind::Unsupported => "unsupported",
        FailureKind::ResourceExhausted => "resource_exhausted",
        FailureKind::InternalInvariant => "internal_invariant",
        FailureKind::Cancelled => "cancelled",
    }
}
