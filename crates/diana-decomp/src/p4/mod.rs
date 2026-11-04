//! P4 value facts, per-use interpretations and position-related initialization.
mod build;
mod constraints;
mod initialization;
mod product;
mod solve;
mod views;

pub use build::TypeError;
pub(crate) use build::{TypeBuild, build_types};
pub use product::*;
