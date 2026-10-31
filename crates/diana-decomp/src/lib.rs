//! Native DEX analysis with caller-owned input and request-scoped resource control.

#[path = "p1_input/mod.rs"]
pub mod input;
mod p9_emit;
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
    ByteLength, ByteOffset, ByteRange, CallSiteId, ClassDefId, CodeUnitCount, CodeUnitDelta,
    CodeUnitOffset, FieldId, MethodHandleId, MethodId, ProtoId, RegisterId, RegisterWordCount,
    StringId, TypeId, Utf16Count, Utf16Offset,
};

pub use pipeline::control::{
    ControlError, FallbackBudget, Limits, MethodBudget, RequestLimit, ResourceUsage, RunContext,
};
pub use pipeline::diagnostics::{DiagnosticCode, FailureKind};
