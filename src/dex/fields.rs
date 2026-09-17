//! The DEX field layout of one class, in the order the runtime lays the fields out.
//!
//! Two consumers need this and neither can guess it: a script that has to know which
//! instance slots hold references before it dereferences them, and a reader that has
//! an index from the runtime and wants the field behind it. Both are answered from
//! the DEX the class actually came from, not from a signature someone typed.
//!
//! The order is the load-bearing part. `class_data_item` stores static fields and then
//! instance fields, each as a chain of `field_idx` deltas, so the sequence below is
//! ascending `field_ids` order within each kind - which is exactly the order the
//! runtime's own field arrays follow. The instance reference mask is defined over that
//! sequence: bit *j* describes the *j*-th instance field.

use super::{Dex, read_uleb};
use crate::bytes::read_u16;
use anyhow::{Context, Result, bail};

/// `class_def_item` is a fixed 32-byte row; `class_data_off` sits at +24.
const CLASS_DEF_ITEM: usize = 32;
const CLASS_DATA_OFF: usize = 24;

/// One field as the DEX declares it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct FieldRow {
    /// The `field_ids` index. A runtime's field index refers to this, not to the
    /// position in this list, so it is reported alongside the position.
    pub field_index: u32,
    pub name: String,
    pub type_descriptor: String,
    pub access_flags: u32,
}

/// One class's field layout, both kinds kept apart and each in declaration order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct FieldPlan {
    pub descriptor: String,
    pub instance: Vec<FieldRow>,
    pub statics: Vec<FieldRow>,
}

impl FieldPlan {
    /// Bit *j* is set when the *j*-th instance field (ascending `field_index`) has a
    /// reference type (`L...;` or `[...`).
    ///
    /// This is a runtime contract, not a convenience: a collector uses
    /// `(mask >> i) & 1` to decide whether instance slot *i* may be dereferenced as a
    /// pointer, so a mask that is off by one bit makes it read a non-reference slot as
    /// a pointer. Fields from the 65th on are not represented (the mask is one word).
    pub(crate) fn instance_ref_mask(&self) -> u64 {
        let mut mask = 0u64;
        for (position, field) in self.instance.iter().enumerate() {
            if position >= 64 {
                break;
            }
            let descriptor = field.type_descriptor.as_str();
            if descriptor.starts_with('L') || descriptor.starts_with('[') {
                mask |= 1u64 << position;
            }
        }
        mask
    }

    /// The one-object JSONL record this plan is handed around as.
    ///
    /// `field_index` is in here as well as the position, because a caller that has an
    /// index from the runtime has to be able to find the row it belongs to.
    pub(crate) fn render_json(&self) -> String {
        let mut out = String::with_capacity(128 + 96 * (self.instance.len() + self.statics.len()));
        out.push_str("{\"schema\":\"rasc.fields-plan/v1\",\"descriptor\":");
        push_json_string(&mut out, &self.descriptor);
        out.push_str(",\"instance_fields\":");
        push_rows(&mut out, &self.instance);
        out.push_str(",\"static_fields\":");
        push_rows(&mut out, &self.statics);
        out.push_str(",\"counts\":{\"instance\":");
        out.push_str(&self.instance.len().to_string());
        out.push_str(",\"static\":");
        out.push_str(&self.statics.len().to_string());
        out.push_str("},\"instance_ref_mask\":\"");
        out.push_str(&format!("0x{:x}", self.instance_ref_mask()));
        out.push_str("\"}");
        out
    }
}

fn push_rows(out: &mut String, rows: &[FieldRow]) {
    out.push('[');
    for (position, row) in rows.iter().enumerate() {
        if position > 0 {
            out.push(',');
        }
        out.push_str("{\"field_index\":");
        out.push_str(&row.field_index.to_string());
        out.push_str(",\"name\":");
        push_json_string(out, &row.name);
        out.push_str(",\"type\":");
        push_json_string(out, &row.type_descriptor);
        out.push_str(",\"access_flags\":");
        out.push_str(&row.access_flags.to_string());
        out.push('}');
    }
    out.push(']');
}

/// Writes `value` as a JSON string.
///
/// Hand-rolled because this is the one record in rasc that a program parses, and the
/// names it carries come from an untrusted file: DEX member names may contain `"`,
/// `\`, and control characters, all of which have to be escaped or the record becomes
/// unparseable exactly when it matters. Byte-for-byte what a JSON writer produces for
/// ASCII and non-ASCII text alike, since a non-ASCII character is valid as written.
fn push_json_string(out: &mut String, value: &str) {
    out.push('"');
    for character in value.chars() {
        match character {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            // The remaining C0 controls have no short form; `\u00XX` is what the
            // JSON grammar asks for and what every parser accepts.
            character if (character as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", character as u32));
            }
            character => out.push(character),
        }
    }
    out.push('"');
}

/// Builds the field plan for `descriptor`, or `None` when this DEX does not define it.
///
/// `descriptor` must already be in DEX form (`Lcom/foo/Bar;`); the CLI normalizes what
/// a user types before calling. Rows come from the class's own `class_data`, so a
/// malformed entry - one that points at a `field_ids` row belonging to another class -
/// is an error rather than a row from the wrong class.
pub(crate) fn field_plan(data: &[u8], descriptor: &str) -> Result<Option<FieldPlan>> {
    let dex = Dex::parse(data)?;
    let Some((class_index, class_type_idx)) = find_class(&dex, descriptor)? else {
        return Ok(None);
    };
    let class_data_off =
        dex.u32(dex.header.classes_off + class_index * CLASS_DEF_ITEM + CLASS_DATA_OFF)?;
    // A class with no fields and no methods has no class_data at all.
    if class_data_off == 0 {
        return Ok(Some(FieldPlan {
            descriptor: descriptor.to_owned(),
            instance: Vec::new(),
            statics: Vec::new(),
        }));
    }

    let mut offset = class_data_off as usize;
    let static_size = read_uleb(data, &mut offset)?;
    let instance_size = read_uleb(data, &mut offset)?;
    // The two method counts follow the fields; the methods themselves are not walked.
    let _direct_size = read_uleb(data, &mut offset)?;
    let _virtual_size = read_uleb(data, &mut offset)?;

    let mut statics = Vec::with_capacity(static_size as usize);
    let mut field_index = 0u32;
    for _ in 0..static_size {
        let row = read_encoded_field(&dex, data, &mut offset, &mut field_index, class_type_idx)?;
        statics.push(row);
    }

    let mut instance = Vec::with_capacity(instance_size as usize);
    let mut field_index = 0u32;
    for _ in 0..instance_size {
        let row = read_encoded_field(&dex, data, &mut offset, &mut field_index, class_type_idx)?;
        instance.push(row);
    }

    Ok(Some(FieldPlan {
        descriptor: descriptor.to_owned(),
        instance,
        statics,
    }))
}

/// Reads one `encoded_field` (`field_idx_diff`, `access_flags`) and resolves it.
///
/// `class_type_idx` is the planned class's `type_ids` index: a `field_ids` row names its
/// declaring class by *type* index, not by its position in `class_defs`.
fn read_encoded_field(
    dex: &Dex<'_>,
    data: &[u8],
    offset: &mut usize,
    field_index: &mut u32,
    class_type_idx: usize,
) -> Result<FieldRow> {
    let difference = read_uleb(data, offset)?;
    let access_flags = read_uleb(data, offset)?;
    *field_index = field_index
        .checked_add(difference)
        .context("field index overflow in class_data")?;

    let row_off = dex
        .header
        .fields_off
        .checked_add(*field_index as usize * 8)
        .context("field_ids row offset overflow")?;
    // `Dex::parse` proved the whole table is in range, so this row is too.
    let row_class = read_u16(data, row_off)? as usize;
    if row_class != class_type_idx {
        bail!(
            "class_data of type {class_type_idx} references field_ids[{field_index}] of type {row_class}"
        );
    }
    let type_idx = read_u16(data, row_off + 2)? as usize;
    let name_idx = dex.u32(row_off + 4)? as usize;

    Ok(FieldRow {
        field_index: *field_index,
        name: dex.string(name_idx)?,
        type_descriptor: dex.type_name(type_idx)?,
        access_flags,
    })
}

/// The `class_defs` row index and the class's `type_ids` index, for `descriptor`.
fn find_class(dex: &Dex<'_>, descriptor: &str) -> Result<Option<(usize, usize)>> {
    for index in 0..dex.header.classes_size {
        let class_idx = dex.u32(dex.header.classes_off + index * CLASS_DEF_ITEM)? as usize;
        if dex.type_name(class_idx)? == descriptor {
            return Ok(Some((index, class_idx)));
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(field_index: u32, type_descriptor: &str) -> FieldRow {
        FieldRow {
            field_index,
            name: format!("f{field_index}"),
            type_descriptor: type_descriptor.to_owned(),
            access_flags: 0,
        }
    }

    fn plan(instance: Vec<FieldRow>) -> FieldPlan {
        FieldPlan {
            descriptor: "LFixture;".to_owned(),
            instance,
            statics: Vec::new(),
        }
    }

    /// Only `L...;` and `[...` are references; primitives are not.
    #[test]
    fn instance_ref_mask_marks_reference_slots_by_position() {
        let mask = plan(vec![
            row(10, "I"),
            row(11, "Ljava/lang/String;"),
            row(12, "[B"),
            row(13, "J"),
            row(14, "Lcom/foo/Bar;"),
        ])
        .instance_ref_mask();
        assert_eq!(mask, 0b1_0110);
    }

    /// The mask is one word: the 65th instance field is not represented, and its
    /// type must not shift anything into it either.
    #[test]
    fn instance_ref_mask_stops_after_the_64th_instance_field() {
        let mut fields: Vec<FieldRow> = (0..64).map(|i| row(i, "I")).collect();
        fields.push(row(64, "Ljava/lang/String;"));
        assert_eq!(plan(fields).instance_ref_mask(), 0);

        let mut fields: Vec<FieldRow> = (0..64).map(|i| row(i, "I")).collect();
        fields[63] = row(63, "[I");
        fields.push(row(64, "Ljava/lang/String;"));
        assert_eq!(plan(fields).instance_ref_mask(), 1 << 63);
    }

    /// DEX member names are arbitrary MUTF-8, so a name can carry characters that
    /// would end the record early. The escaping is what keeps the record parseable
    /// exactly when it matters.
    #[test]
    fn json_string_escapes_what_a_dex_name_can_carry() {
        let mut out = String::new();
        push_json_string(&mut out, "a\"b\\c\nd\te\u{1}f\u{2028}g");
        assert_eq!(out, "\"a\\\"b\\\\c\\nd\\te\\u0001f\u{2028}g\"");
    }

    #[test]
    fn render_json_reports_position_counts_and_mask() {
        let plan = FieldPlan {
            descriptor: "Lcom/foo/Bar;".to_owned(),
            instance: vec![row(7, "Ljava/lang/Object;"), row(9, "I")],
            statics: vec![row(3, "I")],
        };
        assert_eq!(
            plan.render_json(),
            "{\"schema\":\"rasc.fields-plan/v1\",\"descriptor\":\"Lcom/foo/Bar;\",\
             \"instance_fields\":[\
             {\"field_index\":7,\"name\":\"f7\",\"type\":\"Ljava/lang/Object;\",\"access_flags\":0},\
             {\"field_index\":9,\"name\":\"f9\",\"type\":\"I\",\"access_flags\":0}],\
             \"static_fields\":[{\"field_index\":3,\"name\":\"f3\",\"type\":\"I\",\"access_flags\":0}],\
             \"counts\":{\"instance\":2,\"static\":1},\"instance_ref_mask\":\"0x1\"}"
        );
    }
}
