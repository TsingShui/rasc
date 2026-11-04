//! Native DEX analysis with caller-owned input and request-scoped resource control.

mod context;
mod engine;
#[path = "p1_input/mod.rs"]
pub mod input;
#[path = "p0_class/mod.rs"]
pub mod p0;
mod p2_flow;
mod p3_ssa;
mod p4_types;
mod p5_variables;
mod p6_regions;
mod p7_java;
mod p8_emit;
pub use engine::{
    DexImage, Engine, MethodSelector, Options, RecoveryPolicy, RepresentationPolicy, Request,
};
pub use input::annotation_facts::{
    AnnotationExtent, AnnotationLocation, AnnotationParameterIndex, AnnotationPrefix,
    AnnotationRecord, AnnotationToken, AnnotationValue, AnnotationValueKind, OriginalAnnotations,
};
pub use input::ownership::{NestedClass, Ownership};
pub use p4_types::CaptureCertificate;
pub use p7_java::enums::{EnumFieldRole, EnumMemberRole, GeneratedEnumField, GeneratedEnumMember};
pub use p7_java::member_emission::GeneratedMember;
pub use p7_java::nested_plan::GeneratedCaptureField;
pub use p7_java::{
    GeneratedBridge, JavaCallSite, JavaField, JavaInitializer, JavaInvokeKind, JavaMethod,
    JavaMethodDecl, JavaUnit,
};
pub use p8_emit::render;
pub use pipeline::request::RequestError;
pub use substrate::fields::{
    FieldDeclarationOutput, FieldFacts, FieldInitialValue, FieldKey, FieldOutput, FieldStorage,
    GeneratedField, StaticFieldOrdinal,
};
pub use substrate::report::NestedReport;
pub use substrate::report::{AnnotationStatus, ClassAnnotations};
pub use substrate::report::{
    ClassOriginSpan, ClassOutput, DeclarationOutput, Diagnostic, DiagnosticCause, ImageFact,
    ImageOrigin, MethodFacts, MethodOutput, OriginSpan, OutputReason, RenderedReport, Report,
    SourceLanguage,
};
mod pipeline;

pub use p8_emit::{
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
    CodeUnitDelta, CodeUnitOffset, DexId, DiagnosticId, FieldId, InputId, MethodHandleId, MethodId,
    MethodKey, ProtoId, RegisterId, RegisterWordCount, SourceByteOffset, StringId, TypeId,
    Utf16Count, Utf16Offset,
};

pub use pipeline::control::{ControlError, RunContext};
pub use pipeline::diagnostics::{DiagnosticCode, FailureKind};
