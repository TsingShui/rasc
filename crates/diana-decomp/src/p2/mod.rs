//! P2 execution model; independent correspondence checks are test-only.

mod build;
#[cfg(test)]
mod check;
mod evidence;
mod facts;
#[cfg(test)]
mod oracle;
mod product;
#[cfg(test)]
mod replay;
#[cfg(test)]
mod scratch;
mod transfer;
#[cfg(test)]
mod wide_copy;

pub use build::FlowError;
pub(crate) use build::{FlowBuild, build_flow};
pub use evidence::MethodEvidence;
pub use facts::ExecutionFacts;
pub use product::*;
pub use transfer::*;
