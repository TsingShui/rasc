//! P3 exception-aware pruned SSA over immutable P2 execution facts.
mod build;
#[cfg(test)]
mod check;
mod dominance;
mod graph;
mod product;
mod rename;
mod scratch;

pub use build::SsaError;
pub(crate) use build::{SsaBuild, build_ssa};
pub use product::*;
