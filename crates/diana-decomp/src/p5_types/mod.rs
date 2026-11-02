mod array_components;
mod arrays;
pub(crate) mod calls;
mod declaration_types;
pub(crate) mod entry_initialization;
pub(crate) mod initialization;
pub(crate) mod normal;
pub(crate) mod numeric;
mod primitive_stores;
pub(crate) use crate::substrate::primitive_types::PrimitiveArray;
pub(crate) use arrays::ArrayElement;
pub(crate) mod reference_roots;
mod references;
pub(crate) use crate::substrate::primitive_types::{BuiltinReference, IntRange, Narrow};
#[cfg(test)]
mod tests;
