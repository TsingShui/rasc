use super::DebugError;
use super::debug::{DebugIssue, IssueLog, local_input, missing};
use super::json::JsonBuffer;
use crate::input::debug_program::{DebugEvent, DebugOp, DebugProgram};
use crate::input::{Cursor, MethodError, ProgramIndex, RawMethodView};
use crate::raw::RawDecodeError;
use crate::{ByteLength, ByteOffset, CodeUnitCount, MethodId, RunContext, StringId};

fn note_error(
    error: MethodError,
    offset: ByteOffset,
    code: &RawMethodView<'_>,
    issues: &mut IssueLog,
    budget: &mut RunContext<'_>,
) -> Result<(), DebugError> {
    match error {
        MethodError::Input(error) => {
            missing(issues, DebugIssue::Input(local_input(error)?), budget)
        }
        MethodError::Decode(error) => {
            let pc = match error {
                RawDecodeError::UnknownEncoding { pc, .. }
                | RawDecodeError::UnsupportedVersion { pc, .. }
                | RawDecodeError::Malformed { pc, .. } => pc,
            };
            let count =
                code.code_units()
                    .0
                    .checked_sub(pc.0)
                    .ok_or(DebugError::InternalInvariant(
                        "debug boundary failure outside original code",
                    ))?;
            missing(
                issues,
                DebugIssue::Code {
                    start: pc,
                    count: CodeUnitCount(count),
                    error,
                },
                budget,
            )?;
            missing(
                issues,
                DebugIssue::Metadata {
                    name: "debug_instruction_boundary",
                    offset,
                },
                budget,
            )
        }
        other => Err(other.into()),
    }
}

pub(super) fn write_program(
    index: &ProgramIndex<'_>,
    output: &mut JsonBuffer,
    code: &RawMethodView<'_>,
    method_id: MethodId,
    issues: &mut IssueLog,
    budget: &mut RunContext<'_>,
) -> Result<(), DebugError> {
    let Some(offset) = code.debug_info_offset_byte() else {
        return Ok(());
    };
    output.push(",\"debug_metadata\":{\"file_byte_start\":", budget)?;
    output.unsigned(
        u64::try_from(offset.0).map_err(|_| DebugError::PlatformCapacity)?,
        budget,
    )?;
    let mut program = match DebugProgram::open(index, code, method_id, offset, budget) {
        Ok(program) => program,
        Err(error) => {
            note_error(error, offset, code, issues, budget)?;
            return output.push(
                ",\"complete\":false,\"unavailable\":\"debug_header\"}",
                budget,
            );
        }
    };
    output.push(",\"line_start\":", budget)?;
    output.signed(i64::from(program.line_start), budget)?;
    output.push(",\"parameter_names\":[", budget)?;
    let names = program.parameter_names;
    let mut cursor = Cursor::bounded(code.source_bytes(), names.start, names.end)?;
    let mut first = true;
    while cursor.remaining_byte() != ByteLength(0) {
        if !first {
            output.push(",", budget)?;
        }
        first = false;
        let encoded = cursor.read_uleb32_with(budget)?;
        write_string(index, output, encoded.checked_sub(1).map(StringId), budget)?;
    }
    output.push("],\"events\":[", budget)?;
    let mut first = true;
    let mut complete = true;
    let mut decoded_end = program.position();
    loop {
        let offset = program.position();
        match program.next(index, code, budget) {
            Ok(Some(event)) => {
                decoded_end = event.range.end;
                if !first {
                    output.push(",", budget)?;
                }
                first = false;
                write_event(index, output, code, event, budget)?;
            }
            Ok(None) => break,
            Err(error) => {
                note_error(error, offset, code, issues, budget)?;
                complete = false;
                break;
            }
        }
    }
    output.push("],\"file_byte_end\":", budget)?;
    output.unsigned(
        u64::try_from(decoded_end.0).map_err(|_| DebugError::PlatformCapacity)?,
        budget,
    )?;
    output.push(",\"complete\":", budget)?;
    output.boolean(complete, budget)?;
    output.push("}", budget)
}

fn write_string(
    index: &ProgramIndex<'_>,
    output: &mut JsonBuffer,
    id: Option<StringId>,
    budget: &mut RunContext<'_>,
) -> Result<(), DebugError> {
    let Some(id) = id else {
        return output.push("null", budget);
    };
    output.push("{\"index\":", budget)?;
    output.unsigned(u64::from(id.0), budget)?;
    output.push(",\"units\":", budget)?;
    output.units(index.string_with(id, budget)?, budget)?;
    output.push("}", budget)
}

fn write_event(
    index: &ProgramIndex<'_>,
    output: &mut JsonBuffer,
    code: &RawMethodView<'_>,
    event: DebugEvent,
    budget: &mut RunContext<'_>,
) -> Result<(), DebugError> {
    output.push("{\"opcode\":", budget)?;
    output.unsigned(u64::from(event.opcode), budget)?;
    output.push(",\"file_byte_start\":", budget)?;
    output.unsigned(
        u64::try_from(event.range.start.0).map_err(|_| DebugError::PlatformCapacity)?,
        budget,
    )?;
    output.push(",\"pc\":", budget)?;
    output.unsigned(u64::from(event.pc.0), budget)?;
    output.push(",\"line\":", budget)?;
    output.signed(i64::from(event.line), budget)?;
    match event.op {
        DebugOp::AdvancePc(delta) => {
            output.push(",\"address_delta\":", budget)?;
            output.unsigned(u64::from(delta.0), budget)?;
        }
        DebugOp::AdvanceLine(delta) => {
            output.push(",\"line_delta\":", budget)?;
            output.signed(i64::from(delta), budget)?;
        }
        DebugOp::StartLocal {
            register,
            name,
            type_id,
            signature,
        } => {
            output.push(",\"register\":", budget)?;
            output.unsigned(u64::from(register.0), budget)?;
            output.push(",\"name\":", budget)?;
            write_string(index, output, name, budget)?;
            output.push(",\"type\":", budget)?;
            if let Some(id) = type_id {
                output.push("{\"index\":", budget)?;
                output.unsigned(u64::from(id.0), budget)?;
                output.push(",\"descriptor\":", budget)?;
                output.string(index.type_name(id, budget)?, budget)?;
                output.push("}", budget)?;
            } else {
                output.push("null", budget)?;
            }
            output.push(",\"signature\":", budget)?;
            write_string(index, output, signature, budget)?;
        }
        DebugOp::EndLocal(register) | DebugOp::RestartLocal(register) => {
            output.push(",\"register\":", budget)?;
            output.unsigned(u64::from(register.0), budget)?;
        }
        DebugOp::SetFile(id) => {
            output.push(",\"source_file\":", budget)?;
            write_string(index, output, id, budget)?;
        }
        DebugOp::End | DebugOp::PrologueEnd | DebugOp::EpilogueBegin | DebugOp::Position => {}
    }
    output.push(",\"bytes\":[", budget)?;
    let bytes =
        code.source_bytes()
            .get(event.range.as_range())
            .ok_or(DebugError::InternalInvariant(
                "checked debug event range is missing",
            ))?;
    for (position, byte) in bytes.iter().enumerate() {
        budget.checkpoint()?;
        if position > 0 {
            output.push(",", budget)?;
        }
        output.unsigned(u64::from(*byte), budget)?;
    }
    output.push("]}", budget)
}
