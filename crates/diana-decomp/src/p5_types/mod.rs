mod array_components;
mod arrays;
pub(crate) mod calls;
pub(crate) mod capture;
mod declaration_types;
pub(crate) mod entry_initialization;
pub(crate) mod initialization;
mod normal;
pub(crate) use normal::{
    ArrayEvidence, ArrayReceiver, ArrayUse, FieldUse, Kind, TypedBody, UseConversion, solve,
};
pub(crate) mod numeric;
mod primitive_stores;
pub(crate) use crate::substrate::primitive_types::PrimitiveArray;
pub(crate) use arrays::ArrayElement;
pub(crate) mod reference_roots;
mod references;
pub(crate) use crate::substrate::primitive_types::{BuiltinReference, IntRange, Narrow};
#[cfg(test)]
mod tests;
