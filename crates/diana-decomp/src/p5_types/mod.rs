mod arrays;
pub(crate) mod calls;
pub(crate) mod initialization;
mod integral;
pub(crate) mod normal;
pub(crate) mod numeric;
pub(crate) use arrays::{ArrayElement, PrimitiveArray};
mod references;
pub(crate) use integral::{IntRange, Narrow};
pub(crate) use references::BuiltinReference;
#[cfg(test)]
mod tests;
