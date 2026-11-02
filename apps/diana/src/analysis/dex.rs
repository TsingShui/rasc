use diana_decomp::input::{FieldStorage, ProgramIndex};
use diana_decomp::{ClassDefId, FieldId, MethodId, RunContext, StringId};

use super::query::QueryError;

/// App-local order of the root images in one request. It is display metadata and
/// never decides original identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct ImageIndex(pub usize);

/// One `class_defs` definition with its original identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClassRow {
    pub image: ImageIndex,
    pub class_def: ClassDefId,
    pub descriptor: Vec<u16>,
    pub access_flags: u32,
}

/// One method actually defined by `class_data`; an external `method_ids`
/// reference is never a definition.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MethodRow {
    pub image: ImageIndex,
    pub class_def: ClassDefId,
    pub method: MethodId,
    pub name: Vec<u16>,
    pub prototype: Vec<u16>,
    pub access_flags: u32,
}

/// One field actually defined by `class_data`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FieldRow {
    pub image: ImageIndex,
    pub class_def: ClassDefId,
    pub field: FieldId,
    pub name: Vec<u16>,
    pub field_type: Vec<u16>,
    pub access_flags: u32,
    pub storage: FieldStorage,
}

/// One original `string_ids` value, independent of where it is used.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StringRow {
    pub image: ImageIndex,
    pub string: StringId,
    pub units: Vec<u16>,
}

/// Opens one root image for read-only table queries. No method body or analysis
/// product is built here.
pub(super) fn open<'a>(
    bytes: &'a [u8],
    control: &mut RunContext<'_>,
) -> Result<ProgramIndex<'a>, QueryError> {
    Ok(ProgramIndex::open(bytes, control)?)
}

pub(super) fn classes(
    index: &ProgramIndex<'_>,
    image: ImageIndex,
    filter: Option<&[u16]>,
    control: &mut RunContext<'_>,
    emit: &mut impl FnMut(ClassRow) -> Result<(), QueryError>,
) -> Result<(), QueryError> {
    let mut failure: Option<QueryError> = None;
    let result = {
        let mut forward = |row: ClassRow| -> Result<(), diana_decomp::input::InputError> {
            if failure.is_none()
                && let Err(error) = emit(row)
            {
                failure = Some(error);
            }
            Ok(())
        };
        index.visit_classes(control, |class_def, class, control| {
            let class = class?;
            let descriptor = class.descriptor.units(control)?;
            if let Some(needle) = filter
                && !contains(&descriptor, needle)
            {
                return Ok(());
            }
            forward(ClassRow {
                image,
                class_def,
                descriptor,
                access_flags: class.access_flags,
            })?;
            Ok(())
        })
    };
    match failure {
        Some(error) => Err(error),
        None => result.map_err(QueryError::from),
    }
}

pub(super) fn methods(
    index: &ProgramIndex<'_>,
    image: ImageIndex,
    class_filter: Option<&[u16]>,
    filter: Option<&[u16]>,
    control: &mut RunContext<'_>,
    emit: &mut impl FnMut(MethodRow) -> Result<(), QueryError>,
) -> Result<(), QueryError> {
    let mut failure: Option<QueryError> = None;
    let result = {
        let mut forward = |row: MethodRow| -> Result<(), diana_decomp::input::InputError> {
            if failure.is_none()
                && let Err(error) = emit(row)
            {
                failure = Some(error);
            }
            Ok(())
        };
        index.visit_classes(control, |class_def, class, control| {
            let class = class?;
            if let Some(target) = class_filter
                && !class.descriptor.matches(target, control)?
            {
                return Ok(());
            }
            class.visit_methods(control, |method, control| {
                let declaration = method.declaration?;
                let name = declaration.name.units(control)?;
                if let Some(needle) = filter
                    && !contains(&name, needle)
                {
                    return Ok(());
                }
                let mut prototype = Vec::new();
                prototype.push(u16::from(b'('));
                declaration
                    .prototype
                    .visit_parameters(control, |parameter, control| {
                        prototype.extend(parameter.units(control)?);
                        Ok(())
                    })?;
                prototype.push(u16::from(b')'));
                prototype.extend(declaration.prototype.return_type.units(control)?);
                forward(MethodRow {
                    image,
                    class_def,
                    method: method.method_idx,
                    name,
                    prototype,
                    access_flags: method.access_flags,
                })
            })
        })
    };
    match failure {
        Some(error) => Err(error),
        None => result.map_err(QueryError::from),
    }
}

pub(super) fn fields(
    index: &ProgramIndex<'_>,
    image: ImageIndex,
    class_filter: Option<&[u16]>,
    filter: Option<&[u16]>,
    control: &mut RunContext<'_>,
    emit: &mut impl FnMut(FieldRow) -> Result<(), QueryError>,
) -> Result<(), QueryError> {
    let mut failure: Option<QueryError> = None;
    let result = {
        let mut forward = |row: FieldRow| -> Result<(), diana_decomp::input::InputError> {
            if failure.is_none()
                && let Err(error) = emit(row)
            {
                failure = Some(error);
            }
            Ok(())
        };
        index.visit_classes(control, |class_def, class, control| {
            let class = class?;
            if let Some(target) = class_filter
                && !class.descriptor.matches(target, control)?
            {
                return Ok(());
            }
            class.visit_fields(control, |field, control| {
                let declaration = field.declaration?;
                let name = declaration.name.units(control)?;
                if let Some(needle) = filter
                    && !contains(&name, needle)
                {
                    return Ok(());
                }
                forward(FieldRow {
                    image,
                    class_def,
                    field: field.field_idx,
                    name,
                    field_type: declaration.field_type.units(control)?,
                    access_flags: field.access_flags,
                    storage: field.storage,
                })
            })
        })
    };
    match failure {
        Some(error) => Err(error),
        None => result.map_err(QueryError::from),
    }
}

pub(super) fn strings(
    index: &ProgramIndex<'_>,
    image: ImageIndex,
    filter: Option<&[u16]>,
    control: &mut RunContext<'_>,
    emit: &mut impl FnMut(StringRow) -> Result<(), QueryError>,
) -> Result<(), QueryError> {
    for ordinal in 0..index.string_ids_len() {
        control.checkpoint()?;
        let string = StringId(ordinal);
        let units = index.string(string, control)?.units(control)?;
        if let Some(needle) = filter
            && !contains(&units, needle)
        {
            continue;
        }
        emit(StringRow {
            image,
            string,
            units,
        })?;
    }
    Ok(())
}

/// Converts a class name in dotted form to an original descriptor. A value that is
/// already a descriptor is kept, so a nested `$` name is not rewritten.
pub fn class_descriptor(text: &str) -> Vec<u16> {
    let units: Vec<u16> = text.encode_utf16().collect();
    if units.first() == Some(&u16::from(b'L')) && units.last() == Some(&u16::from(b';')) {
        return units;
    }
    let mut descriptor = Vec::with_capacity(units.len() + 2);
    descriptor.push(u16::from(b'L'));
    for unit in units {
        descriptor.push(if unit == u16::from(b'.') {
            u16::from(b'/')
        } else {
            unit
        });
    }
    descriptor.push(u16::from(b';'));
    descriptor
}

/// Literal UTF-16 substring match over the primary column text.
pub fn contains(haystack: &[u16], needle: &[u16]) -> bool {
    if needle.is_empty() {
        return true;
    }
    if needle.len() > haystack.len() {
        return false;
    }
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}
