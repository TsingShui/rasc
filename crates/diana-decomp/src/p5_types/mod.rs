mod arrays;
mod integral;
pub(crate) use arrays::PrimitiveArray;
mod references;
pub(crate) use references::BuiltinReference;
mod scalar;
pub(crate) use integral::{IntRange, Narrow};
#[cfg(test)]
mod tests;
pub(crate) use scalar::{Conversion, Solved, solve};
