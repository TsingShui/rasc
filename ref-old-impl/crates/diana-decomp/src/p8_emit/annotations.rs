use super::DebugError;
use super::debug::IssueLog;
use super::json::JsonBuffer;
use super::values::{note, write_entries};
use crate::input::annotations::{AnnotationMembers, AnnotationOwner, association};
use crate::input::{ClassView, Cursor, ProgramIndex};
use crate::{ByteLength, ByteOffset, RunContext};

fn write_set(
    index: &ProgramIndex<'_>,
    output: &mut JsonBuffer,
    offset: ByteOffset,
    issues: &mut IssueLog,
    budget: &mut RunContext<'_>,
) -> Result<bool, DebugError> {
    output.push("{\"items\":[", budget)?;
    let result = index.annotation_offsets(0x1003, offset, budget);
    let mut complete = true;
    match result {
        Err(error) => {
            note(error.into(), issues, budget)?;
            complete = false;
        }
        Ok(mut offsets) => {
            let mut last = None;
            let mut first = true;
            while offsets.remaining_byte() != ByteLength(0) {
                let parsed = index.next_annotation(&mut offsets, &mut last, budget);
                let (offset, visibility, type_id, mut values) = match parsed {
                    Ok(item) => item,
                    Err(error) => {
                        note(error, issues, budget)?;
                        complete = false;
                        break;
                    }
                };
                if !first {
                    output.push(",", budget)?;
                }
                first = false;
                output.push("{\"visibility\":", budget)?;
                output.unsigned(u64::from(visibility), budget)?;
                output.push(",\"type_idx\":", budget)?;
                output.unsigned(u64::from(type_id.0), budget)?;
                output.push(",\"type\":", budget)?;
                output.string(index.type_name(type_id, budget)?, budget)?;
                output.push(",\"file_byte_start\":", budget)?;
                output.unsigned(
                    u64::try_from(offset.0).map_err(|_| DebugError::PlatformCapacity)?,
                    budget,
                )?;
                let (decoded, end) = write_entries(index, output, &mut values, issues, budget)?;
                complete &= decoded;
                output.push(",\"file_byte_end\":", budget)?;
                output.unsigned(
                    u64::try_from(end.0).map_err(|_| DebugError::PlatformCapacity)?,
                    budget,
                )?;
                output.push(",\"complete\":", budget)?;
                output.boolean(decoded, budget)?;
                output.push("}", budget)?;
                if !decoded {
                    break;
                }
            }
        }
    }
    output.push("],\"complete\":", budget)?;
    output.boolean(complete, budget)?;
    output.push("}", budget)?;
    Ok(complete)
}

fn write_parameters(
    index: &ProgramIndex<'_>,
    output: &mut JsonBuffer,
    offset: ByteOffset,
    count: usize,
    issues: &mut IssueLog,
    budget: &mut RunContext<'_>,
) -> Result<bool, DebugError> {
    output.push("{\"items\":[", budget)?;
    let mut complete = true;
    match index.annotation_parameters(offset, count, budget) {
        Err(error) => {
            note(error.into(), issues, budget)?;
            complete = false;
        }
        Ok(mut offsets) => {
            let mut first = true;
            while offsets.remaining_byte() != ByteLength(0) {
                let raw = offsets.read_u32_with(budget)?;
                if !first {
                    output.push(",", budget)?;
                }
                first = false;
                if raw == 0 {
                    output.push("{\"items\":[],\"complete\":true}", budget)?;
                } else {
                    let offset =
                        ByteOffset(usize::try_from(raw).map_err(|_| DebugError::PlatformCapacity)?);
                    complete &= write_set(index, output, offset, issues, budget)?;
                }
            }
        }
    }
    output.push("],\"complete\":", budget)?;
    output.boolean(complete, budget)?;
    output.push("}", budget)?;
    Ok(complete)
}

fn write_associations(
    index: &ProgramIndex<'_>,
    class: &ClassView<'_, '_>,
    output: &mut JsonBuffer,
    mut records: Cursor<'_>,
    kind: AssociationKind,
    issues: &mut IssueLog,
    budget: &mut RunContext<'_>,
) -> Result<bool, DebugError> {
    output.push("[", budget)?;
    if records.remaining_byte() == ByteLength(0) {
        output.push("]", budget)?;
        return Ok(true);
    }
    let method = !matches!(kind, AssociationKind::Field);
    let mut members = match AnnotationMembers::open(index, class, method, budget) {
        Ok(members) => members,
        Err(error) => {
            note(error.into(), issues, budget)?;
            output.push("]", budget)?;
            return Ok(false);
        }
    };
    let mut last = None;
    let mut first = true;
    let mut complete = true;
    while records.remaining_byte() != ByteLength(0) {
        let (id, offset, count) = match association(
            index,
            class,
            &mut members,
            &mut records,
            &mut last,
            method,
            budget,
        ) {
            Ok(item) => item,
            Err(error) => {
                note(error.into(), issues, budget)?;
                complete = false;
                continue;
            }
        };
        if !first {
            output.push(",", budget)?;
        }
        first = false;
        output.push(
            if method {
                "{\"method_id_idx\":"
            } else {
                "{\"field_id_idx\":"
            },
            budget,
        )?;
        let id = match id {
            AnnotationOwner::Field(id) => u64::from(id.0),
            AnnotationOwner::Method(id) => u64::from(id.0),
        };
        output.unsigned(id, budget)?;
        if matches!(kind, AssociationKind::Parameter) {
            output.push(",\"parameters\":", budget)?;
            complete &= write_parameters(index, output, offset, count, issues, budget)?;
        } else {
            output.push(",\"annotations\":", budget)?;
            complete &= write_set(index, output, offset, issues, budget)?;
        }
        output.push("}", budget)?;
    }
    output.push("]", budget)?;
    Ok(complete)
}

enum AssociationKind {
    Field,
    Method,
    Parameter,
}

pub(super) fn write_directory(
    index: &ProgramIndex<'_>,
    class: &ClassView<'_, '_>,
    output: &mut JsonBuffer,
    offset: ByteOffset,
    issues: &mut IssueLog,
    budget: &mut RunContext<'_>,
) -> Result<(), DebugError> {
    output.push(",\"annotation_directory\":{\"file_byte_start\":", budget)?;
    output.unsigned(
        u64::try_from(offset.0).map_err(|_| DebugError::PlatformCapacity)?,
        budget,
    )?;
    let directory = match index.annotation_directory(offset, budget) {
        Ok(directory) => directory,
        Err(error) => {
            note(error.into(), issues, budget)?;
            return output.push(",\"complete\":false}", budget);
        }
    };
    output.push(",\"class\":", budget)?;
    let mut complete = if directory.class == ByteOffset(0) {
        output.push("{\"items\":[],\"complete\":true}", budget)?;
        true
    } else {
        write_set(index, output, directory.class, issues, budget)?
    };
    for (name, records, kind) in [
        (",\"fields\":", directory.fields, AssociationKind::Field),
        (",\"methods\":", directory.methods, AssociationKind::Method),
        (
            ",\"parameters\":",
            directory.parameters,
            AssociationKind::Parameter,
        ),
    ] {
        output.push(name, budget)?;
        complete &= write_associations(index, class, output, records, kind, issues, budget)?;
    }
    output.push(",\"complete\":", budget)?;
    output.boolean(complete, budget)?;
    output.push("}", budget)
}
