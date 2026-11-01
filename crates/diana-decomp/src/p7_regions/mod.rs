mod branch;
pub(crate) use branch::{Arm, BodyBranch};
mod flow;
mod graph;
mod plan;
pub(crate) mod protection;
mod protection_build;
pub(crate) use plan::{PlannedBody, Region, Shape, build};
mod integer_switch;
pub(crate) use integer_switch::{SwitchForm, choose as switch_form};
mod normal;
#[cfg(test)]
mod tests;
mod verify;
pub(crate) use verify::NaturalLoop;
