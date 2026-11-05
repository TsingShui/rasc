//! P1 disassembly, structural checks and original instruction associations.

pub(crate) mod binding;
mod decode;
mod disassembly;
mod reference;
mod structure;

pub use disassembly::{
    ArrayPayload, DecodedInstruction, DecodedItem, DecodedMethod, DecodedOutcome, DecodedPayload,
    DecodedReference, EntryLayout, EntryParameter, InvokeLayout,
};
pub use reference::{
    CheckedMethodHandleFact, CheckedParameter, CheckedPrototype, CheckedReferenceFact, TypeForm,
};
pub use structure::{CheckedMethodLayout, MethodError, MethodProblem};
