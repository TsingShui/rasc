//! P1 disassembly, structural checks and original instruction associations.

pub(crate) mod binding;
mod decode;
mod disassembly;
mod structure;

pub use disassembly::{
    DecodedInstruction, DecodedItem, DecodedMethod, DecodedPayload, DecodedReference,
};
pub use structure::{CheckedMethodLayout, MethodError, MethodProblem};
