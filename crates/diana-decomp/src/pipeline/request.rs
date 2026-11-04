use crate::FailureKind;
use crate::input::{InputError, MethodError};
use std::collections::TryReserveError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RequestError {
    #[error(transparent)]
    Input(#[from] InputError),
    #[error(transparent)]
    Method(#[from] MethodError),
    #[error(transparent)]
    Flow(#[from] crate::p2::FlowError),
    #[error("request allocation failed: {0}")]
    Allocation(#[source] TryReserveError),
    #[error("request capacity is not representable on this platform")]
    PlatformCapacity,
    #[error("undefined register {register:?} at {pc:?}")]
    UndefinedRegister {
        register: crate::RegisterId,
        pc: crate::CodeUnitOffset,
    },
    /// Conservative exception ingress prevents proving an otherwise grounded value.
    /// This preserves uncertainty rather than declaring a definite invalid read.
    #[error("definition availability unproved for register {register:?} at {pc:?}")]
    DefinitionAvailabilityUnproved {
        register: crate::RegisterId,
        pc: crate::CodeUnitOffset,
    },
    #[error("monitor scope unproved at {pc:?}")]
    MonitorScopeUnproved { pc: crate::CodeUnitOffset },
    #[error("partial or invalid wide register {register:?} read at {pc:?}")]
    InvalidWideRead {
        register: crate::RegisterId,
        pc: crate::CodeUnitOffset,
    },
    #[error("non-empty fill has no admitted FreshArray shape at {pc:?}")]
    ArrayFillShape { pc: crate::CodeUnitOffset },
    #[error("allocation/initialization cannot be represented at {pc:?}")]
    InitializationUnavailable { pc: crate::CodeUnitOffset },
    #[error("binding evidence missing for method {reference:?} at {pc:?}")]
    BindingEvidenceMissing {
        reference: crate::MethodId,
        pc: crate::CodeUnitOffset,
    },
    #[error("binding evidence missing for field {reference:?} at {pc:?}")]
    FieldBindingEvidenceMissing {
        reference: crate::FieldId,
        pc: crate::CodeUnitOffset,
    },
    #[error("nested source-name binding is unproved in class {class:?}")]
    SourceNameUnproved { class: crate::ClassDefId },
    #[error("requested DEX identity does not exist")]
    UnknownDex,
    #[error("BUG: {0}")]
    InternalInvariant(&'static str),
}

impl RequestError {
    pub fn kind(&self) -> FailureKind {
        match self {
            Self::Input(error) => error.kind(),
            Self::Method(error) => error.kind(),
            Self::Flow(error) => error.kind(),

            Self::Allocation(_) | Self::PlatformCapacity => FailureKind::ResourceExhausted,
            Self::UnknownDex | Self::UndefinedRegister { .. } | Self::InvalidWideRead { .. } => {
                FailureKind::Malformed
            }
            Self::DefinitionAvailabilityUnproved { .. }
            | Self::MonitorScopeUnproved { .. }
            | Self::ArrayFillShape { .. }
            | Self::InitializationUnavailable { .. }
            | Self::BindingEvidenceMissing { .. }
            | Self::FieldBindingEvidenceMissing { .. }
            | Self::SourceNameUnproved { .. } => FailureKind::Unsupported,
            Self::InternalInvariant(_) => FailureKind::InternalInvariant,
        }
    }
}
