/// Classification is independent of the human-readable error text.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FailureKind {
    Malformed,
    Unsupported,
    ResourceExhausted,
    InternalInvariant,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiagnosticCode {
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
