use super::debug::{IssueLog, write_reference};
use super::json::{JsonBuffer, capacity_plan};
use super::{DebugError, values};
use crate::input::values::{EncodedKind, EncodedValue, EncodedValues, ValueStep};
use crate::input::{InputError, ProgramIndex};
use crate::raw::OperandReference;
use crate::{ByteOffset, FailureKind, RunContext};

pub(super) enum Initializers<'a> {
    Absent,
    Ready {
        values: EncodedValues<'a>,
        next: u32,
    },
    Failed(FailureKind),
}
impl<'a> Initializers<'a> {
    pub fn open(
        index: &ProgramIndex<'a>,
        offset: Result<Option<ByteOffset>, InputError>,
        issues: &mut IssueLog,
        budget: &mut RunContext<'_>,
    ) -> Result<Self, DebugError> {
        match offset {
            Ok(Some(offset)) => match EncodedValues::open(index, offset, None, budget) {
                Ok(values) => Ok(Self::Ready { values, next: 0 }),
                Err(error) => {
                    let kind = error.kind();
                    values::note(error, issues, budget)?;
                    Ok(Self::Failed(kind))
                }
            },
            Ok(None) => Ok(Self::Absent),
            Err(error) => Ok(Self::Failed(error.kind())),
        }
    }
    pub fn write(
        &mut self,
        index: &ProgramIndex<'a>,
        output: &mut JsonBuffer,
        ordinal: u32,
        issues: &mut IssueLog,
        budget: &mut RunContext<'_>,
    ) -> Result<(), DebugError> {
        let failed = match self {
            Self::Absent => return output.push("null", budget),
            Self::Failed(kind) => return output.unavailable(*kind, budget),
            Self::Ready { values, next } => {
                // Invalid/skipped field declarations still occupy their original
                // encoded-array positions. Each intervening value is read once.
                while *next < ordinal && *next < values.count {
                    if let Some(kind) = tree(index, values, None, *next, issues, budget)? {
                        *self = Self::Failed(kind);
                        return output.unavailable(kind, budget);
                    }
                    *next = next.checked_add(1).ok_or(DebugError::PlatformCapacity)?;
                }
                if ordinal >= values.count {
                    return output.push("null", budget);
                }
                if *next != ordinal {
                    return Err(DebugError::InternalInvariant(
                        "initializer stream moved backwards",
                    ));
                }
                let failed = tree(index, values, Some(output), ordinal, issues, budget)?;
                *next = next.checked_add(1).ok_or(DebugError::PlatformCapacity)?;
                failed
            }
        };
        if let Some(kind) = failed {
            *self = Self::Failed(kind);
        }
        Ok(())
    }
}
#[derive(Clone, Copy)]
enum Close {
    Array,
    Annotation,
}
impl Close {
    fn text(self) -> &'static str {
        match self {
            Self::Array => "]}",
            Self::Annotation => "}}}",
        }
    }
}

fn tree(
    index: &ProgramIndex<'_>,
    values: &mut EncodedValues<'_>,
    mut output: Option<&mut JsonBuffer>,
    ordinal: u32,
    issues: &mut IssueLog,
    budget: &mut RunContext<'_>,
) -> Result<Option<FailureKind>, DebugError> {
    let mut stack = Vec::new();
    let mut root = true;
    loop {
        budget.checkpoint()?;
        match values.next(index, budget) {
            Ok(Some(ValueStep::Value(value))) => {
                if root && value.ordinal.0 != ordinal {
                    return Err(DebugError::InternalInvariant(
                        "initializer has a different original ordinal",
                    ));
                }
                if let Some(output) = output.as_deref_mut() {
                    if !root && value.ordinal.0 > 0 {
                        output.push(",", budget)?;
                    }
                    if !root && let Some(name) = value.name {
                        output.string(index.string_with(name, budget)?, budget)?;
                        output.push(":", budget)?;
                    }
                }
                let close = match value.kind {
                    EncodedKind::Array { .. } => Some(Close::Array),
                    EncodedKind::Annotation { .. } => Some(Close::Annotation),
                    _ => None,
                };
                if let Some(output) = output.as_deref_mut() {
                    semantic(index, output, value, budget)?;
                }
                root = false;
                if let Some(close) = close {
                    if stack.len() == stack.capacity() {
                        let needed = stack
                            .len()
                            .checked_add(1)
                            .ok_or(DebugError::PlatformCapacity)?;
                        let capacity = capacity_plan(stack.capacity(), needed, size_of::<Close>())?;
                        budget.checkpoint()?;
                        stack
                            .try_reserve_exact(capacity - stack.len())
                            .map_err(DebugError::Allocation)?;
                    }
                    stack.push(close);
                } else if stack.is_empty() {
                    return Ok(None);
                }
            }
            Ok(Some(ValueStep::EndContainer)) => {
                let close = stack.pop().ok_or(DebugError::InternalInvariant(
                    "initializer close has no container",
                ))?;
                if let Some(output) = output.as_deref_mut() {
                    output.push(close.text(), budget)?;
                }
                if stack.is_empty() {
                    return Ok(None);
                }
            }
            Ok(None) => {
                return Err(DebugError::InternalInvariant(
                    "initializer ended before its original value",
                ));
            }
            Err(error) => {
                let kind = error.kind();
                values::note(error, issues, budget)?;
                if let Some(output) = output.as_deref_mut() {
                    if root {
                        output.unavailable(kind, budget)?;
                    }
                    while let Some(close) = stack.pop() {
                        budget.checkpoint()?;
                        output.push(close.text(), budget)?;
                    }
                }
                return Ok(Some(kind));
            }
        }
    }
}
fn semantic(
    index: &ProgramIndex<'_>,
    output: &mut JsonBuffer,
    value: EncodedValue<'_>,
    budget: &mut RunContext<'_>,
) -> Result<(), DebugError> {
    output.push("{\"tag\":", budget)?;
    output.unsigned(u64::from(value.tag), budget)?;
    output.push(",\"value\":", budget)?;
    match value.kind {
        EncodedKind::Signed(value) => output.signed(value, budget)?,
        EncodedKind::Unsigned(value) => output.unsigned(value, budget)?,
        EncodedKind::Bits { width, bits } => {
            let digits = match width.0 {
                4 => 8,
                8 => 16,
                _ => {
                    return Err(DebugError::InternalInvariant(
                        "encoded float has an invalid width",
                    ));
                }
            };
            output.push("\"", budget)?;
            for digit in (0..digits).rev() {
                budget.checkpoint()?;
                let nibble = u8::try_from((bits >> (digit * 4)) & 15).map_err(|_| {
                    DebugError::InternalInvariant("masked float digit is not a nibble")
                })?;
                output.character(char::from(b"0123456789abcdef"[usize::from(nibble)]), budget)?;
            }
            output.push("\"", budget)?;
        }
        EncodedKind::Reference(OperandReference::String(id)) => {
            output.units(index.string_with(id, budget)?, budget)?
        }
        EncodedKind::Reference(OperandReference::Type(id)) => {
            output.string(index.type_name(id, budget)?, budget)?
        }
        EncodedKind::Reference(reference) => write_reference(index, output, reference, budget)?,
        EncodedKind::Null => output.push("null", budget)?,
        EncodedKind::Boolean(value) => output.boolean(value, budget)?,
        EncodedKind::Array { .. } => return output.push("[", budget),
        EncodedKind::Annotation { type_id, .. } => {
            output.push("{\"type\":", budget)?;
            output.string(index.type_name(type_id, budget)?, budget)?;
            return output.push(",\"elements\":{", budget);
        }
    }
    output.push("}", budget)
}
