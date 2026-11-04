pub(crate) mod constructor_entry;
mod copies;
pub(crate) mod empty_fill;
pub(crate) mod exceptional;
pub(crate) mod fresh_fill;
mod normal;
pub(crate) use normal::{Initializer, Parameter, StorageBody, allocate, local_type};
pub(crate) mod object_allocation;

pub(crate) use copies::{
    Copy, CopyPlan, CopySource, LocalId, LocalType, LocalTypeId, Temporary, Width, plan, verify,
};
