use super::strings::TypeShape;
use super::{
    Cursor, InputError, InputProblem, MethodError, ProgramIndex, UnsupportedInput, malformed,
};
use crate::pipeline::control::InputControl;
use crate::raw::OperandReference;
use crate::{
    ByteLength, ByteOffset, ByteRange, FallbackBudget, FieldId, MethodId, StringId, TypeId,
};

#[derive(Clone, Copy)]
pub(crate) struct ValueOrdinal(pub u32);

pub(crate) enum EncodedKind {
    Signed(i64),
    Unsigned(u64),
    Bits { width: ByteLength, bits: u64 },
    Reference(OperandReference),
    Null,
    Boolean(bool),
    Array { count: u32 },
    Annotation { type_id: TypeId, count: u32 },
}

pub(crate) struct EncodedValue<'a> {
    pub ordinal: ValueOrdinal,
    pub name: Option<StringId>,
    pub range: ByteRange,
    pub tag: u8,
    pub arg: u8,
    pub kind: EncodedKind,
    pub bytes: &'a [u8],
}

pub(crate) enum ValueStep<'a> {
    Value(EncodedValue<'a>),
    EndContainer,
}

enum FrameKind {
    Array,
    Annotation { last_name: Option<StringId> },
}

struct Frame {
    kind: FrameKind,
    remaining: u32,
    next: ValueOrdinal,
}

/// Forward metadata grammar checks. This does not seal field initializer types
/// or annotation ownership. Scratch holds container depth, never all values.
pub(crate) struct EncodedValues<'a> {
    cursor: Cursor<'a>,
    bytes: &'a [u8],
    root: Frame,
    stack: Vec<Frame>,
    pub count: u32,
}

fn invalid(offset: ByteOffset) -> InputError {
    malformed(InputProblem::InvalidEncodedValue, offset)
}

fn count(
    cursor: &mut Cursor<'_>,
    minimum: u64,
    maximum: Option<u32>,
    budget: &mut FallbackBudget<'_, '_>,
) -> Result<u32, InputError> {
    let count = cursor.read_uleb32_with(budget)?;
    if u64::from(count) * minimum > cursor.remaining_byte().0 as u64 {
        return Err(malformed(InputProblem::Truncated, cursor.position_byte()));
    }
    if maximum.is_some_and(|maximum| count > maximum) {
        return Err(invalid(cursor.position_byte()));
    }
    budget.charge_metadata(u64::from(count))?;
    Ok(count)
}

impl<'a> EncodedValues<'a> {
    pub fn open(
        index: &ProgramIndex<'a>,
        offset: ByteOffset,
        static_fields: Option<u32>,
        budget: &mut FallbackBudget<'_, '_>,
    ) -> Result<Self, MethodError> {
        let mut cursor = index.item(0x2005, offset, budget)?;
        let count = count(&mut cursor, 1, static_fields, budget)?;
        Ok(Self {
            cursor,
            bytes: index.bytes,
            root: Frame {
                kind: FrameKind::Array,
                remaining: count,
                next: ValueOrdinal(0),
            },
            stack: Vec::new(),
            count,
        })
    }

    pub fn position(&self) -> ByteOffset {
        self.cursor.position_byte()
    }

    fn push(
        &mut self,
        frame: Frame,
        budget: &mut FallbackBudget<'_, '_>,
    ) -> Result<(), MethodError> {
        budget.charge_metadata(1)?;
        if self.stack.len() == self.stack.capacity() {
            let needed = self
                .stack
                .len()
                .checked_add(1)
                .ok_or(MethodError::PlatformCapacity)?;
            // Each nested container consumes original bytes before it is pushed.
            // The physical byte position bounds depth without a policy limit.
            let capacity = self
                .stack
                .capacity()
                .checked_mul(2)
                .unwrap_or(needed)
                .max(needed)
                .min(self.cursor.position_byte().0);
            if capacity < needed {
                return Err(MethodError::InternalInvariant(
                    "encoded-value depth exceeds consumed bytes",
                ));
            }
            let increment = capacity
                .checked_sub(self.stack.capacity())
                .and_then(|n| n.checked_mul(size_of::<Frame>()))
                .ok_or(MethodError::PlatformCapacity)?;
            budget.charge_allocation(
                u64::try_from(increment).map_err(|_| MethodError::PlatformCapacity)?,
            )?;
            self.stack
                .try_reserve_exact(capacity - self.stack.len())
                .map_err(MethodError::Allocation)?;
            if self.stack.capacity() != capacity {
                return Err(MethodError::InternalInvariant(
                    "encoded-value stack reservation changed capacity",
                ));
            }
            budget.checkpoint()?;
        }
        self.stack.push(frame);
        Ok(())
    }

    fn bits(&mut self, width: u8, budget: &mut FallbackBudget<'_, '_>) -> Result<u64, InputError> {
        if usize::from(width) > self.cursor.remaining_byte().0 {
            return Err(malformed(
                InputProblem::Truncated,
                self.cursor.position_byte(),
            ));
        }
        let mut bits = 0u64;
        for byte in 0..width {
            bits |= u64::from(self.cursor.read_array::<1>(budget)?[0]) << (u32::from(byte) * 8);
        }
        Ok(bits)
    }

    pub fn next(
        &mut self,
        index: &ProgramIndex<'_>,
        budget: &mut FallbackBudget<'_, '_>,
    ) -> Result<Option<ValueStep<'a>>, MethodError> {
        budget.charge_work(1)?;
        let frame = self.stack.last_mut().unwrap_or(&mut self.root);
        if frame.remaining == 0 {
            return if self.stack.pop().is_some() {
                Ok(Some(ValueStep::EndContainer))
            } else {
                Ok(None)
            };
        }
        let name = match &mut frame.kind {
            FrameKind::Array => None,
            FrameKind::Annotation { last_name } => {
                let id = StringId(self.cursor.read_uleb32_with(budget)?);
                index.string_with(id, budget)?.validate_name(budget)?;
                if last_name.is_some_and(|last| last >= id) {
                    return Err(invalid(self.cursor.position_byte()).into());
                }
                *last_name = Some(id);
                Some(id)
            }
        };
        let ordinal = frame.next;
        frame.next.0 = frame
            .next
            .0
            .checked_add(1)
            .ok_or_else(|| invalid(self.cursor.position_byte()))?;
        frame.remaining -= 1;
        let start = self.cursor.position_byte();
        let header = self.cursor.read_array::<1>(budget)?[0];
        let tag = header & 31;
        let arg = header >> 5;
        let maximum = match tag {
            0x00 | 0x1c..=0x1e => 0,
            0x02 | 0x03 | 0x1f => 1,
            0x04 | 0x10 | 0x15..=0x1b => 3,
            0x06 | 0x11 => 7,
            _ => return Err(invalid(start).into()),
        };
        if arg > maximum {
            return Err(invalid(start).into());
        }
        let width = arg + 1;
        let kind = match tag {
            0x00 | 0x02 | 0x04 | 0x06 => {
                let bits = self.bits(width, budget)?;
                let shift = 64 - u32::from(width) * 8;
                EncodedKind::Signed(i64::from_le_bytes((bits << shift).to_le_bytes()) >> shift)
            }
            0x03 => EncodedKind::Unsigned(self.bits(width, budget)?),
            0x10 | 0x11 => {
                let full = if tag == 0x10 { 4 } else { 8 };
                let bits = self.bits(width, budget)? << (u32::from(full - width) * 8);
                EncodedKind::Bits {
                    width: ByteLength(usize::from(full)),
                    bits,
                }
            }
            0x15 | 0x16 => {
                return Err(InputError::Unsupported {
                    reason: UnsupportedInput::EncodedValueTag(tag),
                    offset_byte: start,
                }
                .into());
            }
            0x17..=0x1b => {
                // The validated maximum width is four bytes.
                let id = u32::try_from(self.bits(width, budget)?).map_err(|_| invalid(start))?;
                let reference = match tag {
                    0x17 => {
                        index.string_with(StringId(id), budget)?;
                        OperandReference::String(StringId(id))
                    }
                    0x18 => {
                        index.type_name(TypeId(id), budget)?;
                        OperandReference::Type(TypeId(id))
                    }
                    0x19 | 0x1b => {
                        index.field_reference(FieldId(id), budget)?;
                        OperandReference::Field(FieldId(id))
                    }
                    0x1a => {
                        index.method_reference(MethodId(id), budget)?;
                        OperandReference::Method(MethodId(id))
                    }
                    _ => {
                        return Err(MethodError::InternalInvariant(
                            "unknown checked encoded reference",
                        ));
                    }
                };
                EncodedKind::Reference(reference)
            }
            0x1c => {
                let count = count(&mut self.cursor, 1, None, budget)?;
                self.push(
                    Frame {
                        kind: FrameKind::Array,
                        remaining: count,
                        next: ValueOrdinal(0),
                    },
                    budget,
                )?;
                EncodedKind::Array { count }
            }
            0x1d => {
                let type_id = TypeId(self.cursor.read_uleb32_with(budget)?);
                if !matches!(index.type_descriptor(type_id, budget)?.1, TypeShape::Class) {
                    return Err(invalid(start).into());
                }
                let count = count(&mut self.cursor, 2, None, budget)?;
                self.push(
                    Frame {
                        kind: FrameKind::Annotation { last_name: None },
                        remaining: count,
                        next: ValueOrdinal(0),
                    },
                    budget,
                )?;
                EncodedKind::Annotation { type_id, count }
            }
            0x1e => EncodedKind::Null,
            0x1f => EncodedKind::Boolean(arg != 0),
            _ => {
                return Err(MethodError::InternalInvariant(
                    "unknown checked encoded value",
                ));
            }
        };
        let range = ByteRange {
            start,
            end: self.cursor.position_byte(),
        };
        let bytes = self
            .bytes
            .get(range.as_range())
            .ok_or(MethodError::InternalInvariant(
                "checked encoded value range is missing",
            ))?;
        Ok(Some(ValueStep::Value(EncodedValue {
            ordinal,
            name,
            range,
            tag,
            arg,
            kind,
            bytes,
        })))
    }
}
