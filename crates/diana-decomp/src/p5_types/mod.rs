mod arrays;
pub(crate) mod calls;
mod integral;
pub(crate) mod normal;
pub(crate) use arrays::{ArrayElement, PrimitiveArray};
mod references;
pub(crate) use references::BuiltinReference;
mod scalar;
pub(crate) use integral::{IntRange, Narrow};
#[cfg(test)]
mod tests;
pub(crate) use scalar::{Conversion, Solved, solve};
