use crate::input::{InputError, MethodError};
use crate::{ControlError, DebugError, FailureKind};
use std::collections::TryReserveError;
use thiserror::Error;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JavaTargetItem {
    ParameterWords,
    MethodName,
    MethodDescriptor,
    ConstantUtf8Bytes,
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
    #[error("final assignment unproved for {field:?}")]
    FinalAssignmentUnproved { field: crate::FieldKey },
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
    #[error("Java source may inline constant field {reference:?} read at {pc:?}")]
    JavaConstantFieldRead {
        reference: crate::FieldId,
        pc: crate::CodeUnitOffset,
    },
    #[error("nested source-name binding is unproved in class {class:?}")]
    SourceNameUnproved { class: crate::ClassDefId },
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
            Self::UnknownDex | Self::UndefinedRegister { .. } | Self::InvalidWideRead { .. } => {
                FailureKind::Malformed
            }
            Self::DefinitionAvailabilityUnproved { .. }
            | Self::MonitorScopeUnproved { .. }
            | Self::FinalAssignmentUnproved { .. }
            | Self::ArrayFillShape { .. }
            | Self::JavaTargetLimit { .. }
            | Self::InitializationUnavailable { .. }
            | Self::BindingEvidenceMissing { .. }
            | Self::FieldBindingEvidenceMissing { .. }
            | Self::SourceNameUnproved { .. }
            | Self::JavaConstantFieldRead { .. } => FailureKind::Unsupported,
            Self::InternalInvariant(_) => FailureKind::InternalInvariant,
        }
    }
}
