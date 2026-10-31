mod copies;

pub(crate) use copies::{
    Copy, CopyPlan, LocalId, LocalType, LocalTypeId, Temporary, Width, plan, verify,
};

#[cfg(test)]
mod tests;
