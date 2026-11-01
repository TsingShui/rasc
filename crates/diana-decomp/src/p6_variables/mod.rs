pub(crate) mod allocation;
pub(crate) mod arrays;
mod copies;
pub(crate) mod empty_fill;
pub(crate) mod normal;

pub(crate) use copies::{
    Copy, CopyPlan, LocalId, LocalType, LocalTypeId, Temporary, Width, plan, verify,
};

#[cfg(test)]
mod tests;
