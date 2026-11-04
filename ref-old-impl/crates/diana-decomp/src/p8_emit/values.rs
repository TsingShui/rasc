use super::DebugError;
use super::debug::{DebugIssue, IssueLog, local_input, missing, write_reference};
use super::json::JsonBuffer;
use crate::input::values::{EncodedKind, EncodedValue, EncodedValues, ValueStep};
use crate::input::{MethodError, ProgramIndex};
use crate::{ByteOffset, RunContext};

pub(super) fn note(
    error: MethodError,
    issues: &mut IssueLog,
    budget: &mut RunContext<'_>,
) -> Result<(), DebugError> {
    match error {
        MethodError::Input(error) => {
            missing(issues, DebugIssue::Input(local_input(error)?), budget)
        }
        other => Err(other.into()),
    }
}

pub(super) fn write_values(
    index: &ProgramIndex<'_>,
    output: &mut JsonBuffer,
    offset: ByteOffset,
    static_fields: Option<u32>,
    issues: &mut IssueLog,
    budget: &mut RunContext<'_>,
) -> Result<(), DebugError> {
    output.push(",\"static_values\":{\"file_byte_start\":", budget)?;
    output.unsigned(
        u64::try_from(offset.0).map_err(|_| DebugError::PlatformCapacity)?,
        budget,
    )?;
    let mut values = match EncodedValues::open(index, offset, static_fields, budget) {
        Ok(values) => values,
        Err(error) => {
            note(error, issues, budget)?;
            return output.push(
                ",\"complete\":false,\"unavailable\":\"encoded_array_header\"}",
                budget,
            );
        }
    };
    output.push(",\"count\":", budget)?;
    output.unsigned(u64::from(values.count), budget)?;
    let (complete, decoded_end) = write_entries(index, output, &mut values, issues, budget)?;
    output.push(",\"file_byte_end\":", budget)?;
    output.unsigned(
        u64::try_from(decoded_end.0).map_err(|_| DebugError::PlatformCapacity)?,
        budget,
    )?;
    output.push(",\"complete\":", budget)?;
    output.boolean(complete, budget)?;
    if complete && static_fields.is_some_and(|fields| values.count < fields) {
        output.push(",\"implicit_defaults_from_ordinal\":", budget)?;
        output.unsigned(u64::from(values.count), budget)?;
    }
    output.push("}", budget)
}

pub(super) fn write_entries(
    index: &ProgramIndex<'_>,
    output: &mut JsonBuffer,
    values: &mut EncodedValues<'_>,
    issues: &mut IssueLog,
    budget: &mut RunContext<'_>,
) -> Result<(bool, ByteOffset), DebugError> {
    output.push(",\"entries\":[", budget)?;
    let mut open = 0usize;
    let mut complete = true;
    let mut decoded_end = values.position();
    loop {
        match values.next(index, budget) {
            Ok(Some(ValueStep::Value(value))) => {
                decoded_end = value.range.end;
                if value.ordinal.0 > 0 {
                    output.push(",", budget)?;
                }
                let container = write_value(index, output, value, budget)?;
                if container {
                    open = open.checked_add(1).ok_or(DebugError::PlatformCapacity)?;
                }
            }
            Ok(Some(ValueStep::EndContainer)) => {
                open = open.checked_sub(1).ok_or(DebugError::InternalInvariant(
                    "encoded-value close has no open container",
                ))?;
                output.push("]}", budget)?;
            }
            Ok(None) => break,
            Err(error) => {
                note(error, issues, budget)?;
                complete = false;
                break;
            }
        }
    }
    if complete && open != 0 {
        return Err(DebugError::InternalInvariant(
            "encoded-value stream ended inside container",
        ));
    }
    for _ in 0..open {
        output.push("]}", budget)?;
    }
    output.push("]", budget)?;
    Ok((complete, decoded_end))
}

fn write_value(
    index: &ProgramIndex<'_>,
    output: &mut JsonBuffer,
    value: EncodedValue<'_>,
    budget: &mut RunContext<'_>,
) -> Result<bool, DebugError> {
    output.push("{\"tag\":", budget)?;
    output.unsigned(u64::from(value.tag), budget)?;
    output.push(",\"value_arg\":", budget)?;
    output.unsigned(u64::from(value.arg), budget)?;
    output.push(",\"file_byte_start\":", budget)?;
    output.unsigned(
        u64::try_from(value.range.start.0).map_err(|_| DebugError::PlatformCapacity)?,
        budget,
    )?;
    if let Some(name) = value.name {
        output.push(",\"name_idx\":", budget)?;
        output.unsigned(u64::from(name.0), budget)?;
        output.push(",\"name\":", budget)?;
        output.string(index.string_with(name, budget)?, budget)?;
    }
    output.push(",\"bytes\":[", budget)?;
    for (position, byte) in value.bytes.iter().enumerate() {
        budget.checkpoint()?;
        if position > 0 {
            output.push(",", budget)?;
        }
        output.unsigned(u64::from(*byte), budget)?;
    }
    output.push("]", budget)?;
    match value.kind {
        EncodedKind::Signed(integer) => {
            output.push(",\"value\":", budget)?;
            output.signed(integer, budget)?;
        }
        EncodedKind::Unsigned(integer) => {
            output.push(",\"value\":", budget)?;
            output.unsigned(integer, budget)?;
        }
        EncodedKind::Bits { width, bits } => {
            output.push(",\"width\":", budget)?;
            output.unsigned(
                u64::try_from(width.0).map_err(|_| DebugError::PlatformCapacity)?,
                budget,
            )?;
            output.push(",\"bits\":", budget)?;
            output.unsigned(bits, budget)?;
        }
        EncodedKind::Reference(reference) => {
            output.push(",\"reference\":", budget)?;
            write_reference(index, output, reference, budget)?;
        }
        EncodedKind::Null => output.push(",\"value\":null", budget)?,
        EncodedKind::Boolean(boolean) => {
            output.push(",\"value\":", budget)?;
            output.boolean(boolean, budget)?;
        }
        EncodedKind::Array { count } | EncodedKind::Annotation { count, .. } => {
            if let EncodedKind::Annotation { type_id, .. } = value.kind {
                output.push(",\"annotation_type\":", budget)?;
                output.string(index.type_name(type_id, budget)?, budget)?;
            }
            output.push(",\"count\":", budget)?;
            output.unsigned(u64::from(count), budget)?;
            output.push(",\"entries\":[", budget)?;
            return Ok(true);
        }
    }
    output.push("}", budget)?;
    Ok(false)
}
