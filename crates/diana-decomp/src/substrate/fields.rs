//! Field identities and storage groups retained by P0.

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
pub enum FieldStorage {
    Static(StaticFieldOrdinal),
    Instance,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
pub struct StaticFieldOrdinal(pub u32);
