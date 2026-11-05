//! P6 region, transfer, protection and lock planning over established P5 facts.
mod build;
mod dominance;
mod graph;
mod locks;
mod loops;
mod product;
mod protection;
mod regions;
pub use build::RegionError;
pub(crate) use build::{RegionBuild, build_regions};
pub use product::*;
