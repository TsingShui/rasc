mod integral;
mod scalar;
pub(crate) use integral::{IntRange, Narrow};
#[cfg(test)]
mod tests;
pub(crate) use scalar::{Conversion, Solved, solve};
