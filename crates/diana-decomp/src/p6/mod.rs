//! P6 region, transfer, protection and lock planning over established P5 facts.
mod build;
mod compose;
mod condition_layout;
mod dominance;
mod graph;
mod locks;
mod loops;
mod ownership;
mod product;
mod protection;
mod scopes;
mod transfers;
pub use build::RegionError;
pub(crate) use build::{RegionBuild, build_regions};
pub use product::*;
