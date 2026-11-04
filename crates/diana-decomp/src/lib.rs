//! Native DEX analysis with caller-owned input and classified failures.

#[cfg(test)]
mod dynamic_instructions_test;

mod engine;
pub mod input;
pub mod p0;
pub mod p1;
pub mod p2;
pub mod p3;
pub use engine::{
    DexImage, Engine, MethodOutcome, MethodSelector, Options, RecoveryPolicy, RepresentationPolicy,
    Request,
};
pub use input::annotation_facts::{
    AnnotationExtent, AnnotationLocation, AnnotationParameterIndex, AnnotationPrefix,
    AnnotationRecord, AnnotationToken, AnnotationValue, AnnotationValueKind, OriginalAnnotations,
};
pub use pipeline::request::RequestError;
pub use substrate::fields::{FieldStorage, StaticFieldOrdinal};
pub use substrate::report::{Diagnostic, DiagnosticCause, ImageFact, ImageOrigin};
pub use substrate::slots::{RegisterSlot, SlotWidth};
mod pipeline;

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

pub use pipeline::diagnostics::{DiagnosticCode, FailureKind};
