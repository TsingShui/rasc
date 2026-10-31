//! Native DEX analysis with caller-owned input and request-scoped resource control.

mod engine;
mod p0_knowledge;
pub use p0_knowledge::{
    DeclarationCatalog, DeclarationCoverage, ExternalClassDeclaration, ExternalFieldEvidence,
    ExternalMethodDeclaration, ExternalMethodSignature, JavaConstantKind,
};
#[path = "p1_input/mod.rs"]
pub mod input;
mod p4_calls;
mod p5_types;
mod p6_variables;
mod p7_regions;
mod p8_java;
mod p9_emit;
#[cfg(test)]
#[allow(dead_code)]
#[path = "../tests/input/fixture.rs"]
mod test_fixture;
pub use engine::{
    DexImage, Engine, MethodSelector, Options, RecoveryPolicy, RepresentationPolicy, Request,
};
pub use p8_java::{
    CompilationContract, JavaCallSite, JavaInvokeKind, JavaMethodDecl, VerifiedJavaMethod,
};
pub use p9_emit::render;
pub use pipeline::request::{JavaTargetItem, RequestError};
pub use substrate::report::{
    ClassOutput, DeclarationOutput, Diagnostic, DiagnosticCause, ImageFact, ImageOrigin,
    MethodFacts, MethodOutput, OriginSpan, OutputReason, RenderedReport, Report, SourceLanguage,
};
mod pipeline;

pub use p9_emit::{
    DebugCoverage, DebugError, DebugIssue, DebugResourceError, DebugSource, DebugSourceKind,
};
mod substrate;

pub mod raw {
    pub use crate::input::formats::{
        DexVersion, Format, IndexedRef, InsnLayout, LayoutKind, LayoutProblem, OpcodeSpec,
        OperandReference, RawDecodeError, ReferenceKind, ReferenceSlot, instruction_layout,
    };
    pub use crate::input::operands::{
        ArrayElements, RawArrayElement, RawCodeItem, RawInstruction, RawLiteral, RawOperands,
        RawPayload, RawRegisters, RawSwitchCase, RegisterIter, SwitchCases,
    };
}

pub use substrate::ids::{
    ByteLength, ByteOffset, ByteRange, CallSiteId, ClassDefId, ClassKey, CodeUnitCount,
    CodeUnitDelta, CodeUnitOffset, DexId, DiagnosticId, EnvironmentId, FieldId, InputId,
    MethodHandleId, MethodId, MethodKey, ProtoId, RegisterId, RegisterWordCount, SourceByteOffset,
    StringId, TypeId, Utf16Count, Utf16Offset,
};

pub use pipeline::control::{
    ControlError, FallbackBudget, Limits, MethodBudget, RequestLimit, ResourceUsage, RunContext,
};
pub use pipeline::diagnostics::{DiagnosticCode, FailureKind};
