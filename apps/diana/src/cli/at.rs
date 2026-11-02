//! Canonical `at` locators: original identity, not a result row number.

use crate::analysis::dex::{ClassRow, FieldRow, MethodRow, StringRow};
use crate::analysis::query::ImageInfo;

fn logical(image: &ImageInfo) -> String {
    // A root image is one logical DEX (0) inside its physical entry; a bare DEX
    // is the single entry `e0`.
    let entry = image.entry.map_or(0, |entry| entry.0);
    format!("e{entry}:l0")
}

fn units(text: &[u16]) -> String {
    super::table::escape_units(text)
}

pub fn class(image: &ImageInfo, row: &ClassRow) -> String {
    format!(
        "{}:c{}::{}",
        logical(image),
        row.class_def.0,
        units(&row.descriptor)
    )
}

pub fn method(image: &ImageInfo, row: &MethodRow) -> String {
    format!(
        "{}:c{}:m{}::{}->{}{}",
        logical(image),
        row.class_def.0,
        row.method.0,
        units(&row.class_descriptor),
        units(&row.name),
        units(&row.prototype)
    )
}

pub fn field(image: &ImageInfo, row: &FieldRow) -> String {
    format!(
        "{}:c{}:f{}::{}->{}:{}",
        logical(image),
        row.class_def.0,
        row.field.0,
        units(&row.class_descriptor),
        units(&row.name),
        units(&row.field_type)
    )
}

pub fn string(image: &ImageInfo, row: &StringRow) -> String {
    format!("{}:s{}", logical(image), row.string.0)
}
