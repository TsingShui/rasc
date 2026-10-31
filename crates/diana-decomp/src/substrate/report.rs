use crate::input::{InputError, MethodError};
use crate::p8_java::JavaMethodDecl;
use crate::{
    ByteOffset, ClassKey, DebugError, DebugSource, DexId, DiagnosticId, EnvironmentId, FailureKind,
    InputId, MethodKey, ResourceUsage, SourceByteOffset,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ImageOrigin {
    pub input: InputId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ImageFact {
    pub dex: DexId,
    pub origin: ImageOrigin,
    pub header_offset: ByteOffset,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OutputReason {
    BranchJavaUnsupported,
    BindingEvidenceMissing {
        reference: crate::MethodId,
        pc: crate::CodeUnitOffset,
    },
    JavaAnalysisUnavailable,
    JavaDeclarationUnsupported,
    ClassAssemblyUnavailable,
}

#[derive(Debug)]
pub enum DiagnosticCause {
    Input(InputError),
    Method(MethodError),
    Debug(DebugError),
    Request(crate::RequestError),
    Original(crate::DebugIssue),
    Unsupported(OutputReason),
}
impl DiagnosticCause {
    pub fn kind(&self) -> FailureKind {
        match self {
            Self::Input(e) => e.kind(),
            Self::Method(e) => e.kind(),
            Self::Debug(e) => e.kind(),
            Self::Request(e) => e.kind(),
            Self::Original(e) => e.kind(),
            Self::Unsupported(_) => FailureKind::Unsupported,
        }
    }
}

#[derive(Debug)]
pub struct Diagnostic {
    pub method: Option<MethodKey>,
    pub cause: DiagnosticCause,
}

impl Diagnostic {
    pub fn code(&self) -> crate::DiagnosticCode {
        use crate::DiagnosticCode as Code;
        match &self.cause {
            DiagnosticCause::Input(error) => input_code(*error),
            DiagnosticCause::Method(error) => method_code(error),
            DiagnosticCause::Debug(error) => debug_code(error),
            DiagnosticCause::Request(error) => request_code(error),
            DiagnosticCause::Original(issue) => issue_code(*issue),
            DiagnosticCause::Unsupported(OutputReason::BindingEvidenceMissing { .. }) => {
                Code::BindingEvidenceMissing
            }
            DiagnosticCause::Unsupported(OutputReason::BranchJavaUnsupported) => {
                Code::BranchJavaUnsupported
            }
            DiagnosticCause::Unsupported(OutputReason::JavaAnalysisUnavailable) => {
                Code::JavaAnalysisUnavailable
            }
            DiagnosticCause::Unsupported(OutputReason::JavaDeclarationUnsupported) => {
                Code::JavaDeclarationUnsupported
            }
            DiagnosticCause::Unsupported(OutputReason::ClassAssemblyUnavailable) => {
                Code::ClassAssemblyUnavailable
            }
        }
    }
}

fn input_code(error: InputError) -> crate::DiagnosticCode {
    use crate::DiagnosticCode as Code;
    match error {
        InputError::Malformed { code, .. } => Code::InputMalformed(code),
        InputError::Unsupported { reason, .. } => Code::InputUnsupported(reason),
        InputError::PlatformCapacity { .. } => Code::PlatformCapacity,
        InputError::Allocation { .. } => Code::AllocationFailed,
        InputError::InternalInvariant(_) => Code::InternalInvariant,
        InputError::Control(error) => error.code(),
    }
}

fn method_code(error: &MethodError) -> crate::DiagnosticCode {
    use crate::DiagnosticCode as Code;
    match error {
        MethodError::Input(error) => input_code(*error),
        MethodError::Decode(error) => decode_code(*error),
        MethodError::Control(error) => error.code(),
        MethodError::ForeignInput => Code::ForeignInput,
        MethodError::Malformed { problem, .. } => Code::MethodMalformed(*problem),
        MethodError::ReferenceOutOfBounds { .. } => Code::ReferenceOutOfBounds,
        MethodError::PlatformCapacity => Code::PlatformCapacity,
        MethodError::Allocation(_) => Code::AllocationFailed,
        MethodError::InternalInvariant(_) => Code::InternalInvariant,
    }
}

fn decode_code(error: crate::raw::RawDecodeError) -> crate::DiagnosticCode {
    use crate::DiagnosticCode as Code;
    match error {
        crate::raw::RawDecodeError::UnknownEncoding { .. } => Code::UnknownInstructionWidth,
        crate::raw::RawDecodeError::UnsupportedVersion { .. } => {
            Code::UnsupportedInstructionVersion
        }
        crate::raw::RawDecodeError::Malformed { problem, .. } => {
            Code::MalformedInstruction(problem)
        }
    }
}

fn issue_code(issue: crate::DebugIssue) -> crate::DiagnosticCode {
    match issue {
        crate::DebugIssue::Code { error, .. } => decode_code(error),
        crate::DebugIssue::Input(error) | crate::DebugIssue::Reference { error, .. } => {
            input_code(error)
        }
        crate::DebugIssue::Metadata { .. } => crate::DiagnosticCode::MetadataUnavailable,
    }
}

fn debug_code(error: &DebugError) -> crate::DiagnosticCode {
    use crate::DiagnosticCode as Code;
    match error {
        DebugError::Input(error) => input_code(*error),
        DebugError::Method(error) => method_code(error),
        DebugError::Control(error) => error.code(),
        DebugError::PlatformCapacity => Code::PlatformCapacity,
        DebugError::Allocation(_) => Code::AllocationFailed,
        DebugError::MethodNotDefined { .. } => Code::MethodNotDefined,
        DebugError::AfterMalformed { issue, .. } => issue_code(*issue),
        DebugError::InternalInvariant(_) => Code::InternalInvariant,
    }
}

fn request_code(error: &crate::RequestError) -> crate::DiagnosticCode {
    use crate::{DiagnosticCode as Code, RequestError};
    match error {
        RequestError::Input(error) => input_code(*error),
        RequestError::Method(error) => method_code(error),
        RequestError::Debug(error) => debug_code(error),
        RequestError::Control(error) => error.code(),
        RequestError::Allocation(_) => Code::AllocationFailed,
        RequestError::PlatformCapacity => Code::PlatformCapacity,
        RequestError::UnknownDex => Code::UnknownDex,
        RequestError::UndefinedRegister { .. } => Code::UndefinedRegister,
        RequestError::InvalidWideRead { .. } => Code::InvalidWideRead,
        RequestError::JavaTargetLimit { .. } => Code::JavaTargetLimit,
        RequestError::BindingEvidenceMissing { .. } => Code::BindingEvidenceMissing,
        RequestError::InternalInvariant(_) => Code::InternalInvariant,
    }
}

#[derive(Debug)]
pub struct MethodFacts {
    pub key: MethodKey,
    pub name: Vec<u16>,
    pub prototype: Vec<u16>,
    pub access_flags: u32,
}

pub enum MethodOutput {
    Java {
        facts: MethodFacts,
        declaration: crate::VerifiedJavaMethod,
        source: String,
    },
    DeclarationOnly {
        facts: MethodFacts,
        declaration: JavaMethodDecl,
    },
    Debug {
        facts: Option<MethodFacts>,
        key: MethodKey,
        source: DebugSource,
        reason: DiagnosticId,
    },
    Unavailable {
        facts: Option<MethodFacts>,
        key: MethodKey,
        reason: DiagnosticId,
        excerpt: Option<DebugSource>,
    },
}
impl MethodOutput {
    pub fn key(&self) -> MethodKey {
        match self {
            Self::Java { facts, .. } | Self::DeclarationOnly { facts, .. } => facts.key,
            Self::Debug { key, .. } | Self::Unavailable { key, .. } => *key,
        }
    }
}

pub enum DeclarationOutput {
    Unavailable(DiagnosticId),
}

pub enum ClassOutput {
    Mixed {
        key: ClassKey,
        descriptor: Vec<u16>,
        access_flags: u32,
        shell: DeclarationOutput,
        members: Vec<MethodOutput>,
    },
}

/// Owns presentation facts and original sources, never a borrowed input or
/// analysis arena. Only the engine constructs a report with linked diagnostics.
pub struct Report {
    pub(crate) class: ClassOutput,
    pub(crate) diagnostics: Vec<Diagnostic>,
    pub(crate) stats: ResourceUsage,
    pub(crate) image: ImageFact,
    pub(crate) environment: EnvironmentId,
}
impl Report {
    pub fn class(&self) -> &ClassOutput {
        &self.class
    }
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }
    pub fn stats(&self) -> ResourceUsage {
        self.stats
    }
    pub fn image(&self) -> ImageFact {
        self.image
    }
    pub fn environment(&self) -> EnvironmentId {
        self.environment
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceLanguage {
    Java,
    DexDebug,
    OriginalExcerpt,
    Diagnostic,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OriginSpan {
    pub method: MethodKey,
    pub start: SourceByteOffset,
    pub end: SourceByteOffset,
    pub language: SourceLanguage,
}

pub struct RenderedReport {
    pub source: String,
    pub spans: Vec<OriginSpan>,
}
