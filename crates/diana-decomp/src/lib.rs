//! Native DEX analysis with caller-owned input and request-scoped resource control.

#[path = "p1_input/mod.rs"]
pub mod input;
mod pipeline;
mod substrate;

pub mod raw {
    pub use crate::input::formats::{
        DexVersion, Format, IndexedRef, InsnLayout, LayoutKind, LayoutProblem, OpcodeSpec,
        OperandReference, RawDecodeError, ReferenceKind, ReferenceSlot, instruction_layout,
    };
}

pub use substrate::ids::{
    ByteLength, ByteOffset, ByteRange, CallSiteId, ClassDefId, CodeUnitCount, CodeUnitOffset,
    FieldId, MethodHandleId, MethodId, ProtoId, RegisterWordCount, StringId, TypeId, Utf16Count,
    Utf16Offset,
};

pub use pipeline::control::{
    ControlError, FallbackBudget, Limits, MethodBudget, RequestLimit, ResourceUsage, RunContext,
};
pub use pipeline::diagnostics::{DiagnosticCode, FailureKind};
