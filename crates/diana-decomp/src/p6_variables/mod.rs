pub(crate) mod allocation;
pub(crate) mod arrays;
pub(crate) mod constructor_entry;
mod copies;
pub(crate) mod empty_fill;
pub(crate) mod fresh_fill;
pub(crate) mod normal;
pub(crate) mod object_allocation;

pub(crate) use copies::{
    Copy, CopyPlan, LocalId, LocalType, LocalTypeId, Temporary, Width, plan, verify,
};

#[cfg(test)]
mod tests;
