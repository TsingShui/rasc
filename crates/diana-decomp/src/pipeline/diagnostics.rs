use super::control::ControlError;

/// Classification is independent of the human-readable error text.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FailureKind {
    Malformed,
    Unsupported,
    ResourceExhausted,
    InternalInvariant,
    Cancelled,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiagnosticCode {
    Cancelled,
    InputMalformed(crate::input::InputProblem),
    InputUnsupported(crate::input::UnsupportedInput),
    MethodMalformed(crate::input::MethodProblem),
    ReferenceOutOfBounds,
    ForeignInput,
    UnknownInstructionWidth,
    UnsupportedInstructionVersion,
    MalformedInstruction(crate::raw::LayoutProblem),
    AllocationFailed,
    PlatformCapacity,
    MethodNotDefined,
    MetadataUnavailable,
    BindingEvidenceMissing,
    ArrayFillShape,
    InitializationUnavailable,
    BranchJavaUnsupported,
    JavaAnalysisUnavailable,
    InvokeCustomUnsupported,
    InvokePolymorphicUnsupported,
    JavaDeclarationUnsupported,
    ClassAssemblyUnavailable,
    UnknownDex,
    UndefinedRegister,
    DefinitionAvailabilityUnproved,
    InvalidWideRead,
    MonitorScopeUnproved,
    InternalInvariant,
}

impl ControlError {
    pub fn kind(&self) -> FailureKind {
        match self {
            Self::Cancelled => FailureKind::Cancelled,
            Self::InternalInvariant(_) => FailureKind::InternalInvariant,
        }
    }

    pub fn code(&self) -> DiagnosticCode {
        match self {
            Self::Cancelled => DiagnosticCode::Cancelled,
            Self::InternalInvariant(_) => DiagnosticCode::InternalInvariant,
        }
    }
}
