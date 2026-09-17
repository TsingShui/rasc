//! Class members in DEX order, and the lookups that go from an index to a member.
//!
//! Three callers need this and none of them can guess it: a script that has to know
//! which instance slots hold references before it dereferences them, and a reader that
//! holds a `field_ids` or `method_ids` index from the runtime and wants the member
//! behind it. Everything here is answered from the DEX the class actually came from.
//!
//! The order is the load-bearing part. `class_data_item` stores static fields, instance
//! fields, direct methods and virtual methods, each as a chain of index deltas, so every
//! sequence below is ascending `field_ids` / `method_ids` order within its kind - which is
//! exactly the order the runtime's own field and method arrays follow. The instance
//! reference mask is defined over the instance sequence: bit *j* describes the *j*-th
//! instance field.

use super::{Dex, read_uleb};
use crate::bytes::read_u16;
use anyhow::{Context, Result, bail};

/// `class_def_item` is a fixed 32-byte row; `class_data_off` sits at +24.
const CLASS_DEF_ITEM: usize = 32;
const CLASS_ACCESS_FLAGS: usize = 4;
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

/// One method as the DEX declares it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct MethodRow {
    /// The `method_ids` index - the value a runtime reports as its method index.
    pub method_index: u32,
    /// The `proto_ids` index, for rendering the prototype on demand.
    pub proto_idx: u32,
    pub name: String,
    pub access_flags: u32,
}

/// One class's members, every kind kept apart and each in declaration order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ClassMembers {
    pub descriptor: String,
    pub dex_class_def_idx: u32,
    pub dex_type_idx: u32,
    pub access_flags: u32,
    pub statics: Vec<FieldRow>,
    pub instance: Vec<FieldRow>,
    pub direct_methods: Vec<MethodRow>,
    pub virtual_methods: Vec<MethodRow>,
}

impl ClassMembers {
    /// The field at the position a runtime reports: instance fields first, then
    /// statics.
    ///
    /// This is the numbering a field probe carries (`ifields_` then `sfields_` order),
    /// and it is deliberately not the `field_ids` index each row also carries. The
    /// second element says whether the position landed in the static block.
    pub(crate) fn field_at_position(&self, position: u32) -> Option<(&FieldRow, bool)> {
        let position = position as usize;
        if position < self.instance.len() {
            return Some((&self.instance[position], false));
        }
        self.statics
            .get(position - self.instance.len())
            .map(|row| (row, true))
    }

    /// The method carrying `index`. Direct and virtual methods share one `method_ids`
    /// table, so the two sequences cannot both match.
    pub(crate) fn method(&self, index: u32) -> Option<&MethodRow> {
        self.direct_methods
            .iter()
            .chain(self.virtual_methods.iter())
            .find(|row| row.method_index == index)
    }

    /// The instance-fields-only view `field_plan` publishes.
    pub(crate) fn field_plan(&self) -> FieldPlan {
        FieldPlan {
            descriptor: self.descriptor.clone(),
            dex_class_def_idx: self.dex_class_def_idx,
            dex_type_idx: self.dex_type_idx,
            access_flags: self.access_flags,
            instance: self.instance.clone(),
            statics: self.statics.clone(),
        }
    }
}

/// One class's field layout, both kinds kept apart and each in declaration order.
///
/// The three identity fields are how a consumer ties this plan back to the class it
/// observed at runtime: a descriptor alone does not distinguish two definitions of the
/// same class name, the indices do.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct FieldPlan {
    pub descriptor: String,
    /// Index of the class's `class_defs` row in the DEX this plan came from.
    pub dex_class_def_idx: u32,
    /// The class's `type_ids` index in that DEX.
    pub dex_type_idx: u32,
    /// The class's own access flags, as the DEX declares them.
    pub access_flags: u32,
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
        out.push_str(",\"dex_class_def_idx\":");
        out.push_str(&self.dex_class_def_idx.to_string());
        out.push_str(",\"dex_type_idx\":");
        out.push_str(&self.dex_type_idx.to_string());
        out.push_str(",\"class_access_flags\":");
        out.push_str(&self.access_flags.to_string());
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
/// a user types before calling.
pub(crate) fn field_plan(data: &[u8], descriptor: &str) -> Result<Option<FieldPlan>> {
    Ok(class_members(data, descriptor)?.map(|members| members.field_plan()))
}

/// Walks one class's `class_data_item`: every field and method it declares.
///
/// Rows come from the class's own `class_data`, so a malformed entry - one that points
/// at a `field_ids` or `method_ids` row belonging to another class - is an error rather
/// than a row from the wrong class.
pub(crate) fn class_members(data: &[u8], descriptor: &str) -> Result<Option<ClassMembers>> {
    let dex = Dex::parse(data)?;
    let Some((class_index, class_type_idx)) = find_class(&dex, descriptor)? else {
        return Ok(None);
    };
    let class_data_off =
        dex.u32(dex.header.classes_off + class_index * CLASS_DEF_ITEM + CLASS_DATA_OFF)?;
    // A class with no members at all has no class_data.
    if class_data_off == 0 {
        return Ok(Some(ClassMembers {
            descriptor: descriptor.to_owned(),
            dex_class_def_idx: class_index as u32,
            dex_type_idx: class_type_idx as u32,
            access_flags: dex
                .u32(dex.header.classes_off + class_index * CLASS_DEF_ITEM + CLASS_ACCESS_FLAGS)?,
            statics: Vec::new(),
            instance: Vec::new(),
            direct_methods: Vec::new(),
            virtual_methods: Vec::new(),
        }));
    }

    let mut offset = class_data_off as usize;
    let static_size = read_uleb(data, &mut offset)?;
    let instance_size = read_uleb(data, &mut offset)?;
    let direct_size = read_uleb(data, &mut offset)?;
    let virtual_size = read_uleb(data, &mut offset)?;

    let mut statics = Vec::with_capacity(static_size as usize);
    let mut field_index = 0u32;
    for _ in 0..static_size {
        statics.push(read_encoded_field(
            &dex,
            data,
            &mut offset,
            &mut field_index,
            class_type_idx,
        )?);
    }

    let mut instance = Vec::with_capacity(instance_size as usize);
    let mut field_index = 0u32;
    for _ in 0..instance_size {
        instance.push(read_encoded_field(
            &dex,
            data,
            &mut offset,
            &mut field_index,
            class_type_idx,
        )?);
    }

    let mut direct_methods = Vec::with_capacity(direct_size as usize);
    let mut method_index = 0u32;
    for _ in 0..direct_size {
        direct_methods.push(read_encoded_method(
            &dex,
            data,
            &mut offset,
            &mut method_index,
            class_type_idx,
        )?);
    }

    let mut virtual_methods = Vec::with_capacity(virtual_size as usize);
    let mut method_index = 0u32;
    for _ in 0..virtual_size {
        virtual_methods.push(read_encoded_method(
            &dex,
            data,
            &mut offset,
            &mut method_index,
            class_type_idx,
        )?);
    }

    Ok(Some(ClassMembers {
        descriptor: descriptor.to_owned(),
        dex_class_def_idx: class_index as u32,
        dex_type_idx: class_type_idx as u32,
        access_flags: dex
            .u32(dex.header.classes_off + class_index * CLASS_DEF_ITEM + CLASS_ACCESS_FLAGS)?,
        statics,
        instance,
        direct_methods,
        virtual_methods,
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

/// Reads one `encoded_method` (`method_idx_diff`, `access_flags`, `code_off`) and
/// resolves its name through `method_ids.name_idx`.
///
/// The name is not parsed out of a rendered prototype: a name may contain characters
/// that no separated text form can carry unambiguously.
fn read_encoded_method(
    dex: &Dex<'_>,
    data: &[u8],
    offset: &mut usize,
    method_index: &mut u32,
    class_type_idx: usize,
) -> Result<MethodRow> {
    let difference = read_uleb(data, offset)?;
    let access_flags = read_uleb(data, offset)?;
    let _code_off = read_uleb(data, offset)?;
    *method_index = method_index
        .checked_add(difference)
        .context("method index overflow in class_data")?;

    let row_off = dex
        .header
        .methods_off
        .checked_add(*method_index as usize * 8)
        .context("method_ids row offset overflow")?;
    let row_class = read_u16(data, row_off)? as usize;
    if row_class != class_type_idx {
        bail!(
            "class_data of type {class_type_idx} references method_ids[{method_index}] of type {row_class}"
        );
    }
    let proto_idx = u32::from(read_u16(data, row_off + 2)?);
    let name_idx = dex.u32(row_off + 4)? as usize;

    Ok(MethodRow {
        method_index: *method_index,
        proto_idx,
        name: dex.string(name_idx)?,
        access_flags,
    })
}

/// The machine-readable member table for one class: one line per field and per method.
///
/// The lines read as comments to a human and as records to a program, which is what lets
/// them sit in front of unchanged source. They exist because a *name* is not an identity:
/// `a(II)V` and `a()V` are different methods, and on an obfuscated build the name carries
/// no information at all, so a runtime index cannot be matched to a member without the
/// prototype. `slot` is the position a runtime field index refers to (instance fields
/// first, then statics); `field_ids`/`method_ids` are the DEX indices.
///
/// `name=` is written last on purpose: a DEX member name may contain spaces, so everything
/// after the first `name=` is the name.
pub(crate) fn member_lines(data: &[u8], descriptor: &str) -> Result<Option<Vec<String>>> {
    let Some(members) = class_members(data, descriptor)? else {
        return Ok(None);
    };
    let dex = Dex::parse(data)?;
    let mut lines = Vec::with_capacity(
        members.instance.len()
            + members.statics.len()
            + members.direct_methods.len()
            + members.virtual_methods.len(),
    );
    for (slot, field) in members.instance.iter().enumerate() {
        lines.push(field_line(field, slot as u32, false));
    }
    let statics_base = members.instance.len() as u32;
    for (offset, field) in members.statics.iter().enumerate() {
        lines.push(field_line(field, statics_base + offset as u32, true));
    }
    for method in members
        .direct_methods
        .iter()
        .chain(members.virtual_methods.iter())
    {
        lines.push(format!(
            "# members methods: method_ids={} proto={} flags=0x{:x} name={}",
            method.method_index,
            dex.proto(method.proto_idx as usize)?,
            method.access_flags,
            method.name
        ));
    }
    Ok(Some(lines))
}

fn field_line(field: &FieldRow, slot: u32, is_static: bool) -> String {
    format!(
        "# members fields: field_ids={} slot={} static={} flags=0x{:x} type={} name={}",
        field.field_index, slot, is_static, field.access_flags, field.type_descriptor, field.name
    )
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
            dex_class_def_idx: 0,
            dex_type_idx: 0,
            access_flags: 0,
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

    /// The runtime position counts statics straight on after the instance fields.
    #[test]
    fn field_at_position_crosses_from_instance_into_static() {
        let members = ClassMembers {
            descriptor: "LFixture;".to_owned(),
            dex_class_def_idx: 0,
            dex_type_idx: 0,
            access_flags: 0,
            statics: vec![row(30, "I"), row(31, "J")],
            instance: vec![row(10, "Ljava/lang/String;"), row(20, "I")],
            direct_methods: Vec::new(),
            virtual_methods: Vec::new(),
        };
        assert_eq!(members.field_at_position(0).unwrap().0.field_index, 10);
        assert!(!members.field_at_position(0).unwrap().1);
        assert_eq!(members.field_at_position(1).unwrap().0.field_index, 20);
        assert_eq!(members.field_at_position(2).unwrap().0.field_index, 30);
        assert!(members.field_at_position(2).unwrap().1);
        assert_eq!(members.field_at_position(3).unwrap().0.field_index, 31);
        assert!(members.field_at_position(4).is_none());
    }

    #[test]
    fn render_json_reports_position_counts_and_mask() {
        let plan = FieldPlan {
            descriptor: "Lcom/foo/Bar;".to_owned(),
            dex_class_def_idx: 5,
            dex_type_idx: 6,
            access_flags: 0x400,
            instance: vec![row(7, "Ljava/lang/Object;"), row(9, "I")],
            statics: vec![row(3, "I")],
        };
        let json = plan.render_json();
        // The identity trio comes first: a consumer ties the plan to a runtime class by
        // these indices, and a descriptor alone does not distinguish two definitions.
        assert!(json.starts_with(concat!(
            "{\"schema\":\"rasc.fields-plan/v1\",\"descriptor\":\"Lcom/foo/Bar;\",",
            "\"dex_class_def_idx\":5,\"dex_type_idx\":6,\"class_access_flags\":1024,",
            "\"instance_fields\":[",
        )));
        assert!(json.contains(
            "{\"field_index\":7,\"name\":\"f7\",\"type\":\"Ljava/lang/Object;\",\"access_flags\":0}"
        ));
        assert!(json.contains(
            "{\"field_index\":3,\"name\":\"f3\",\"type\":\"I\",\"access_flags\":0}"
        ));
        assert!(json.ends_with(
            "\"counts\":{\"instance\":2,\"static\":1},\"instance_ref_mask\":\"0x1\"}"
        ));
    }
}
