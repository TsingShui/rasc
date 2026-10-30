//! Class members in DEX declaration order.
//!
//! `getclass --members` needs the exact member order a runtime sees, and it cannot be
//! guessed from the source: the runtime's field array is instance fields first and then
//! statics, and its method array follows `method_ids`. Everything here is answered from
//! the DEX the class actually came from.
//!
//! The order is the load-bearing part. `class_data_item` stores static fields, instance
//! fields, direct methods and virtual methods, each as a chain of index deltas, so every
//! sequence below is ascending `field_ids` / `method_ids` order within its kind - which is
//! exactly the order the runtime's own field and method arrays follow.

use super::{Dex, read_uleb};
use crate::analysis::bytes::read_u16;
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

impl ClassMembers {}

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
