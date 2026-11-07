//! P4 value facts, per-use interpretations and position-related initialization.
mod allocation;
mod arrays;
mod build;
mod components;
mod constraints;
mod dataflow;
mod initialization;
mod product;
mod redundancy;
mod references;
mod refinements;
mod solve;
mod views;

pub use build::TypeError;
pub(crate) use build::{TypeBuild, build_types};
pub use product::*;
