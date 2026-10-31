use super::formats::{
    Format, InsnLayout, LayoutKind, LayoutProblem, RawDecodeError, wide_word, word,
};
use super::{MethodError, RawMethodView};
use crate::{ByteLength, CodeUnitDelta, CodeUnitOffset, FallbackBudget, RegisterId};

/// Original literal bits, before selecting an int/float or long/double view.
/// Arithmetic immediates retain their signed encoded width.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RawLiteral {
    Bits32(u32),
    Bits64(u64),
    Signed8(i8),
    Signed16(i16),
}

#[derive(Clone, Copy, Debug)]
pub(super) enum RegisterEncoding {
    List { values: [RegisterId; 5], count: u8 },
    Range { first: RegisterId, count: u8 },
}

/// Encoded operands, not source arguments. Wide invoke arguments keep both words.
#[derive(Clone, Copy, Debug)]
pub struct RawRegisters {
    pub(super) encoding: RegisterEncoding,
}

impl RawRegisters {
    pub fn len(&self) -> usize {
        usize::from(self.count())
    }
    pub fn is_empty(&self) -> bool {
        self.count() == 0
    }

    pub fn iter(&self) -> RegisterIter<'_> {
        RegisterIter {
            registers: self,
            index: 0,
        }
    }

    fn count(&self) -> u8 {
        match self.encoding {
            RegisterEncoding::List { count, .. } | RegisterEncoding::Range { count, .. } => count,
        }
    }
}

pub struct RegisterIter<'a> {
    registers: &'a RawRegisters,
    index: u8,
}

impl Iterator for RegisterIter<'_> {
    type Item = RegisterId;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index == self.registers.count() {
            return None;
        }
        let value = match &self.registers.encoding {
            RegisterEncoding::List { values, .. } => values[usize::from(self.index)],
            // Encoded first <= u16::MAX and count <= u8::MAX; this cannot
            // overflow u32 even for an out-of-frame original register range.
            RegisterEncoding::Range { first, .. } => RegisterId(first.0 + u32::from(self.index)),
        };
        self.index += 1;
        Some(value)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = usize::from(self.registers.count() - self.index);
        (remaining, Some(remaining))
    }
}

impl ExactSizeIterator for RegisterIter<'_> {}

#[derive(Clone, Copy, Debug)]
pub struct RawOperands {
    registers: RawRegisters,
    literal: Option<RawLiteral>,
    offset: Option<CodeUnitDelta>,
}

impl RawOperands {
    pub fn registers(&self) -> &RawRegisters {
        &self.registers
    }
    pub fn literal(&self) -> Option<RawLiteral> {
        self.literal
    }
    pub fn offset(&self) -> Option<CodeUnitDelta> {
        self.offset
    }
}

#[derive(Clone, Copy, Debug)]
pub struct RawInstruction<'a> {
    pc: CodeUnitOffset,
    layout: InsnLayout,
    words: &'a [u8],
    operands: RawOperands,
}

impl<'a> RawInstruction<'a> {
    pub fn pc(&self) -> CodeUnitOffset {
        self.pc
    }
    pub fn layout(&self) -> InsnLayout {
        self.layout
    }
    pub fn words(&self) -> &'a [u8] {
        self.words
    }
    pub fn operands(&self) -> &RawOperands {
        &self.operands
    }
}

#[derive(Clone, Copy, Debug)]
enum PayloadData<'a> {
    Packed { first: i32, targets: &'a [u8] },
    Sparse { keys: &'a [u8], targets: &'a [u8] },
    Array { width: ByteLength, data: &'a [u8] },
}

/// A borrowed payload extent. Word bytes include any array padding; element
/// bytes exclude it. Targets are deltas from a referencing switch, not this data.
#[derive(Clone, Copy, Debug)]
pub struct RawPayload<'a> {
    pc: CodeUnitOffset,
    layout: InsnLayout,
    words: &'a [u8],
    data: PayloadData<'a>,
}

impl<'a> RawPayload<'a> {
    pub fn pc(&self) -> CodeUnitOffset {
        self.pc
    }
    pub fn layout(&self) -> InsnLayout {
        self.layout
    }
    pub fn words(&self) -> &'a [u8] {
        self.words
    }

    /// Caller charges/checks each iteration. Packed keys use i64 so a malformed
    /// implicit key overflow remains visible rather than wrapping to i32.
    pub fn switch_cases(&self) -> Option<SwitchCases<'a>> {
        let (keys, targets) = match self.data {
            PayloadData::Packed { first, targets } => {
                (SwitchKeys::Packed { first, index: 0 }, targets)
            }
            PayloadData::Sparse { keys, targets } => {
                (SwitchKeys::Sparse(keys.as_chunks::<4>().0.iter()), targets)
            }
            PayloadData::Array { .. } => return None,
        };
        Some(SwitchCases {
            keys,
            targets: targets.as_chunks::<4>().0.iter(),
        })
    }

    /// Caller charges/checks each iteration. Elements are zero-extended original
    /// bits, not FP values or normalized booleans.
    pub fn array_elements(&self) -> Option<ArrayElements<'a>> {
        let PayloadData::Array { width, data } = self.data else {
            return None;
        };
        Some(ArrayElements {
            width,
            chunks: data.chunks_exact(width.0),
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RawSwitchCase {
    pub key: i64,
    pub offset: CodeUnitDelta,
}

enum SwitchKeys<'a> {
    Packed { first: i32, index: u32 },
    Sparse(std::slice::Iter<'a, [u8; 4]>),
}

pub struct SwitchCases<'a> {
    keys: SwitchKeys<'a>,
    targets: std::slice::Iter<'a, [u8; 4]>,
}

impl Iterator for SwitchCases<'_> {
    type Item = RawSwitchCase;

    fn next(&mut self) -> Option<Self::Item> {
        let target = self.targets.next()?;
        let key = match &mut self.keys {
            SwitchKeys::Packed { first, index } => {
                let key = i64::from(*first) + i64::from(*index);
                *index += 1;
                key
            }
            SwitchKeys::Sparse(keys) => {
                // Construction proves one exact key chunk per target chunk.
                let key = keys.next()?;
                i64::from(i32::from_le_bytes(*key))
            }
        };
        Some(RawSwitchCase {
            key,
            offset: CodeUnitDelta(i32::from_le_bytes(*target)),
        })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.targets.size_hint()
    }
}

impl ExactSizeIterator for SwitchCases<'_> {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RawArrayElement {
    pub width: ByteLength,
    pub bits: u64,
}

pub struct ArrayElements<'a> {
    width: ByteLength,
    chunks: std::slice::ChunksExact<'a, u8>,
}

impl Iterator for ArrayElements<'_> {
    type Item = RawArrayElement;

    fn next(&mut self) -> Option<Self::Item> {
        let chunk = self.chunks.next()?;
        let mut bytes = [0; 8];
        // Layout construction admits only 1/2/4/8-byte elements.
        bytes[..self.width.0].copy_from_slice(chunk);
        Some(RawArrayElement {
            width: self.width,
            bits: u64::from_le_bytes(bytes),
        })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.chunks.size_hint()
    }
}

impl ExactSizeIterator for ArrayElements<'_> {}

#[derive(Clone, Copy, Debug)]
pub enum RawCodeItem<'a> {
    Instruction(RawInstruction<'a>),
    Payload(RawPayload<'a>),
}

impl<'a> RawMethodView<'a> {
    /// Visits allocation-free original facts in PC order. Frame/reference/target
    /// semantics remain unverified. Callback payload/register loops use the
    /// supplied fallback budget. Unknown width stops before the next item.
    pub fn visit_instructions(
        &self,
        budget: &mut FallbackBudget<'_, '_>,
        mut visit: impl FnMut(RawCodeItem<'a>, &mut FallbackBudget<'_, '_>) -> Result<(), MethodError>,
    ) -> Result<(), MethodError> {
        self.visit_layouts(budget, |pc, layout, budget| {
            let words = item_words(self.instructions(), pc, layout)?;
            let item = match layout.kind() {
                LayoutKind::Instruction(_) => RawCodeItem::Instruction(RawInstruction {
                    pc,
                    layout,
                    words,
                    operands: instruction_operands(self.instructions(), pc, layout)?,
                }),
                kind => {
                    let data = match kind {
                        LayoutKind::PackedSwitch { .. } => PayloadData::Packed {
                            first: i32::from_le_bytes(
                                wide_word(self.instructions(), pc, 2)?.to_le_bytes(),
                            ),
                            targets: &words[8..],
                        },
                        LayoutKind::SparseSwitch { entries } => {
                            // The checked extent covers 4 + entries * 8 bytes.
                            let middle = 4 + usize::from(entries) * 4;
                            PayloadData::Sparse {
                                keys: &words[4..middle],
                                targets: &words[middle..],
                            }
                        }
                        LayoutKind::ArrayData {
                            element_width,
                            element_count,
                        } => {
                            let length = usize::try_from(element_count)
                                .ok()
                                .and_then(|n| n.checked_mul(element_width.0))
                                .ok_or_else(|| overflow(pc))?;
                            let end = 8usize.checked_add(length).ok_or_else(|| overflow(pc))?;
                            let data = words.get(8..end).ok_or_else(|| overflow(pc))?;
                            PayloadData::Array {
                                width: element_width,
                                data,
                            }
                        }
                        LayoutKind::Instruction(_) => {
                            return Err(MethodError::InternalInvariant(
                                "instruction entered payload decoder",
                            ));
                        }
                    };
                    RawCodeItem::Payload(RawPayload {
                        pc,
                        layout,
                        words,
                        data,
                    })
                }
            };
            visit(item, budget)
        })
    }
}

fn overflow(pc: CodeUnitOffset) -> RawDecodeError {
    RawDecodeError::Malformed {
        pc,
        problem: LayoutProblem::RangeOverflow,
    }
}

fn item_words(
    bytes: &[u8],
    pc: CodeUnitOffset,
    layout: InsnLayout,
) -> Result<&[u8], RawDecodeError> {
    let start = pc.checked_byte_offset().ok_or_else(|| overflow(pc))?;
    let length = layout
        .width()
        .checked_byte_length()
        .ok_or_else(|| overflow(pc))?;
    let end = start.checked_advance(length).ok_or_else(|| overflow(pc))?;
    bytes.get(start.0..end.0).ok_or_else(|| overflow(pc))
}

pub(crate) fn instruction_operands(
    bytes: &[u8],
    pc: CodeUnitOffset,
    layout: InsnLayout,
) -> Result<RawOperands, RawDecodeError> {
    let LayoutKind::Instruction(spec) = layout.kind() else {
        return Err(overflow(pc));
    };
    let first = word(bytes, pc, 0)?;
    let high = first.to_le_bytes()[1];
    let a = u16::from(high & 15);
    let b = u16::from(high >> 4);
    let reg = |n: u16| RegisterId(u32::from(n));
    let list = |values: [u16; 5], count| RawRegisters {
        encoding: RegisterEncoding::List {
            values: values.map(reg),
            count,
        },
    };
    let registers = match spec.format {
        Format::F10x | Format::F10t | Format::F20t | Format::F30t => list([0; 5], 0),
        Format::F12x | Format::F22t | Format::F22s | Format::F22c => list([a, b, 0, 0, 0], 2),
        Format::F22x => list([u16::from(high), word(bytes, pc, 1)?, 0, 0, 0], 2),
        Format::F32x => list([word(bytes, pc, 1)?, word(bytes, pc, 2)?, 0, 0, 0], 2),
        Format::F23x => {
            let bc = word(bytes, pc, 1)?;
            list([u16::from(high), bc & 255, bc >> 8, 0, 0], 3)
        }
        Format::F22b => list([u16::from(high), word(bytes, pc, 1)? & 255, 0, 0, 0], 2),
        Format::F11n => list([a, 0, 0, 0, 0], 1),
        Format::F35c | Format::F45cc => {
            let packed = word(bytes, pc, 2)?;
            list(
                [
                    packed & 15,
                    (packed >> 4) & 15,
                    (packed >> 8) & 15,
                    packed >> 12,
                    a,
                ],
                high >> 4,
            )
        }
        Format::F3rc | Format::F4rcc => RawRegisters {
            encoding: RegisterEncoding::Range {
                first: reg(word(bytes, pc, 2)?),
                count: high,
            },
        },
        _ => list([u16::from(high), 0, 0, 0, 0], 1),
    };
    let signed16 = |delta| -> Result<i16, RawDecodeError> {
        Ok(i16::from_le_bytes(word(bytes, pc, delta)?.to_le_bytes()))
    };
    let signed32 = |delta| -> Result<i32, RawDecodeError> {
        Ok(i32::from_le_bytes(
            wide_word(bytes, pc, delta)?.to_le_bytes(),
        ))
    };
    let literal = match spec.format {
        Format::F11n => {
            let nibble = i32::from(high >> 4);
            let value = if nibble >= 8 { nibble - 16 } else { nibble };
            Some(RawLiteral::Bits32(u32::from_le_bytes(value.to_le_bytes())))
        }
        Format::F21s if spec.opcode == 0x16 => Some(RawLiteral::Bits64(u64::from_le_bytes(
            i64::from(signed16(1)?).to_le_bytes(),
        ))),
        Format::F21s => Some(RawLiteral::Bits32(u32::from_le_bytes(
            i32::from(signed16(1)?).to_le_bytes(),
        ))),
        Format::F31i if spec.opcode == 0x17 => Some(RawLiteral::Bits64(u64::from_le_bytes(
            i64::from(signed32(1)?).to_le_bytes(),
        ))),
        Format::F31i => Some(RawLiteral::Bits32(wide_word(bytes, pc, 1)?)),
        Format::F21h if spec.opcode == 0x19 => {
            Some(RawLiteral::Bits64(u64::from(word(bytes, pc, 1)?) << 48))
        }
        Format::F21h => Some(RawLiteral::Bits32(u32::from(word(bytes, pc, 1)?) << 16)),
        Format::F51l => Some(RawLiteral::Bits64(
            u64::from(wide_word(bytes, pc, 1)?) | (u64::from(wide_word(bytes, pc, 3)?) << 32),
        )),
        Format::F22b => Some(RawLiteral::Signed8(i8::from_le_bytes([word(
            bytes, pc, 1,
        )?
        .to_le_bytes()[1]]))),
        Format::F22s => Some(RawLiteral::Signed16(signed16(1)?)),
        _ => None,
    };
    let offset = match spec.format {
        Format::F10t => Some(CodeUnitDelta(i32::from(i8::from_le_bytes([high])))),
        Format::F20t | Format::F21t | Format::F22t => Some(CodeUnitDelta(i32::from(signed16(1)?))),
        Format::F30t | Format::F31t => Some(CodeUnitDelta(signed32(1)?)),
        _ => None,
    };
    Ok(RawOperands {
        registers,
        literal,
        offset,
    })
}
