//! P2 execution model and owner-local translation validation.

mod build;
mod check;
mod evidence;
mod facts;
mod oracle;
mod product;
mod replay;
mod scratch;
mod transfer;
mod wide_copy;

pub use build::FlowError;
pub(crate) use build::{FlowBuild, build_flow};
pub use evidence::MethodEvidence;
pub use facts::ExecutionFacts;
pub use product::*;
pub use transfer::*;
