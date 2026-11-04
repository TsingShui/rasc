use std::fmt;

use super::{Cursor, InputError, InputProblem, malformed};
use crate::pipeline::control::InputControl;
use crate::{ByteOffset, RunContext, Utf16Count, Utf16Offset};

/// A validated, borrowed MUTF-8 string. Literal surrogate units remain intact;
/// declaration names and descriptors undergo additional syntax checks.
#[derive(Clone, Copy)]
pub struct DexString<'a> {
    bytes: &'a [u8],
    start_byte: ByteOffset,
    end_byte: ByteOffset,
    utf16_len: Utf16Count,
}

impl fmt::Debug for DexString<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DexString")
            .field("start_byte", &self.start_byte)
            .field("end_byte", &self.end_byte)
            .field("utf16_len", &self.utf16_len)
            .finish()
    }
}

impl<'a> DexString<'a> {
    pub fn utf16_len(&self) -> Utf16Count {
        self.utf16_len
    }

    pub(super) fn read(
        bytes: &'a [u8],
        mut cursor: Cursor<'a>,
        control: &mut impl InputControl,
    ) -> Result<Self, InputError> {
        let utf16_len = cursor.read_uleb32_with(control)?;
        if u64::from(utf16_len) >= cursor.remaining_byte().0 as u64 {
            return Err(malformed(InputProblem::Truncated, cursor.position_byte()));
        }
        control.checkpoint()?;
        let start_byte = cursor.position_byte();
        for _ in 0..utf16_len {
            read_unit(&mut cursor, control)?;
        }
        let end_byte = cursor.position_byte();
        if cursor.read_array::<1>(control)? != [0] {
            return Err(malformed(InputProblem::InvalidMutf8, end_byte));
        }
        Ok(Self {
            bytes,
            start_byte,
            end_byte,
            utf16_len: Utf16Count(utf16_len),
        })
    }

    /// Copies the complete original UTF-16 value under the request ledger.
    ///
    /// The copy is owned so a caller can display or match a name without holding
    /// the borrow; surrogate units are preserved exactly. Allocation and work are
    /// charged before the copy grows.
    pub fn units(&self, control: &mut RunContext<'_>) -> Result<Vec<u16>, InputError> {
        control.checkpoint()?;
        let length =
            usize::try_from(self.utf16_len.0).map_err(|_| InputError::PlatformCapacity {
                offset_byte: self.start_byte,
            })?;
        let mut units = Vec::new();
        units
            .try_reserve_exact(length)
            .map_err(|_| InputError::Allocation {
                offset_byte: self.start_byte,
            })?;
        control.checkpoint()?;
        let mut reader = self.reader()?;
        while let Some(unit) = reader.next(control)? {
            units.push(unit);
        }
        if units.len() != length {
            return Err(InputError::InternalInvariant(
                "checked UTF-16 length disagrees with its reader",
            ));
        }
        Ok(units)
    }

    pub fn matches(&self, units: &[u16], control: &mut RunContext<'_>) -> Result<bool, InputError> {
        self.matches_with(units, control)
    }

    pub(crate) fn matches_with(
        &self,
        units: &[u16],
        control: &mut impl InputControl,
    ) -> Result<bool, InputError> {
        control.checkpoint()?;
        if u64::from(self.utf16_len.0) != units.len() as u64 {
            return Ok(false);
        }
        Ok(self.matches_prefix(units, Utf16Offset(0), control)? == Some(Utf16Offset(units.len())))
    }

    pub(super) fn matches_prefix(
        &self,
        units: &[u16],
        mut position: Utf16Offset,
        control: &mut impl InputControl,
    ) -> Result<Option<Utf16Offset>, InputError> {
        let mut reader = self.reader()?;
        while let Some(unit) = reader.next(control)? {
            if units.get(position.0) != Some(&unit) {
                return Ok(None);
            }
            position.0 += 1;
        }
        Ok(Some(position))
    }

    pub(crate) fn reader(&self) -> Result<Units<'a>, InputError> {
        Ok(Units {
            cursor: Cursor::bounded(self.bytes, self.start_byte, self.end_byte)?,
            left: self.utf16_len.0,
        })
    }

    pub(super) fn validate_name(&self, control: &mut impl InputControl) -> Result<(), InputError> {
        let mut reader = self.reader()?;
        if self.bytes.get(self.start_byte.0) == Some(&b'<') {
            if self.utf16_len.0 < 3 || self.bytes.get(self.end_byte.0 - 1) != Some(&b'>') {
                return Err(malformed(InputProblem::InvalidName, self.start_byte));
            }
            reader = Units {
                cursor: Cursor::bounded(
                    self.bytes,
                    ByteOffset(self.start_byte.0 + 1),
                    ByteOffset(self.end_byte.0 - 1),
                )?,
                left: self.utf16_len.0 - 2,
            };
        }
        validate_name_units(reader, NameSyntax::Simple, control)
    }

    pub(super) fn shape(&self, control: &mut impl InputControl) -> Result<TypeShape, InputError> {
        let mut reader = self.reader()?;
        let mut tag = reader
            .next(control)?
            .ok_or_else(|| malformed(InputProblem::InvalidDescriptor, self.start_byte))?;
        let mut dimensions = 0;
        while tag == u16::from(b'[') {
            dimensions += 1;
            if dimensions > 255 {
                return Err(malformed(InputProblem::InvalidDescriptor, self.start_byte));
            }
            tag = reader
                .next(control)?
                .ok_or_else(|| malformed(InputProblem::InvalidDescriptor, self.start_byte))?;
        }
        if tag == u16::from(b'L') {
            if reader.left < 2 || self.bytes.get(self.end_byte.0 - 1) != Some(&b';') {
                return Err(malformed(InputProblem::InvalidDescriptor, self.start_byte));
            }
            reader.left -= 1;
            reader.cursor = Cursor::bounded(
                self.bytes,
                reader.cursor.position_byte(),
                ByteOffset(self.end_byte.0 - 1),
            )?;
            validate_name_units(reader, NameSyntax::ClassPath, control)?;
            return Ok(if dimensions == 0 {
                TypeShape::Class
            } else {
                TypeShape::Array
            });
        }
        if reader.left != 0 {
            return Err(malformed(InputProblem::InvalidDescriptor, self.start_byte));
        }
        if tag == u16::from(b'V') && dimensions == 0 {
            return Ok(TypeShape::Void);
        }
        let primitive = PrimitiveType::from_tag(tag)
            .ok_or_else(|| malformed(InputProblem::InvalidDescriptor, self.start_byte))?;
        Ok(if dimensions == 0 {
            TypeShape::Primitive(primitive)
        } else {
            TypeShape::Array
        })
    }
}

#[derive(Clone, Copy)]
pub(crate) enum TypeShape {
    Void,
    Primitive(PrimitiveType),
    Class,
    Array,
}

impl TypeShape {
    pub(super) fn shorty(self) -> u8 {
        match self {
            Self::Void => b'V',
            Self::Class | Self::Array => b'L',
            Self::Primitive(primitive) => primitive as u8,
        }
    }
    pub(crate) fn words(self) -> u8 {
        match self {
            Self::Void => 0,
            Self::Primitive(PrimitiveType::Long | PrimitiveType::Double) => 2,
            _ => 1,
        }
    }
}

#[derive(Clone, Copy)]
#[repr(u8)]
pub(crate) enum PrimitiveType {
    Boolean = b'Z',
    Byte = b'B',
    Short = b'S',
    Char = b'C',
    Int = b'I',
    Long = b'J',
    Float = b'F',
    Double = b'D',
}

impl PrimitiveType {
    fn from_tag(tag: u16) -> Option<Self> {
        Some(match tag {
            0x5a => Self::Boolean,
            0x42 => Self::Byte,
            0x53 => Self::Short,
            0x43 => Self::Char,
            0x49 => Self::Int,
            0x4a => Self::Long,
            0x46 => Self::Float,
            0x44 => Self::Double,
            _ => return None,
        })
    }
}

pub(crate) struct Units<'a> {
    cursor: Cursor<'a>,
    left: u32,
}

impl Units<'_> {
    pub(crate) fn next(
        &mut self,
        control: &mut impl InputControl,
    ) -> Result<Option<u16>, InputError> {
        if self.left == 0 {
            return Ok(None);
        }
        self.left -= 1;
        Ok(Some(read_unit(&mut self.cursor, control)?))
    }
}

fn read_unit(cursor: &mut Cursor<'_>, control: &mut impl InputControl) -> Result<u16, InputError> {
    let start = cursor.position_byte();
    let [first] = cursor.read_array(control)?;
    match first {
        1..=0x7f => Ok(u16::from(first)),
        0xc0..=0xdf => {
            let [second] = cursor.read_array(control)?;
            let unit = (u16::from(first & 31) << 6) | u16::from(second & 63);
            if second & 0xc0 != 0x80 || (unit < 0x80 && (first != 0xc0 || second != 0x80)) {
                return Err(malformed(InputProblem::InvalidMutf8, start));
            }
            Ok(unit)
        }
        0xe0..=0xef => {
            let [second, third] = cursor.read_array(control)?;
            let unit = (u16::from(first & 15) << 12)
                | (u16::from(second & 63) << 6)
                | u16::from(third & 63);
            if second & 0xc0 != 0x80 || third & 0xc0 != 0x80 || unit < 0x800 {
                return Err(malformed(InputProblem::InvalidMutf8, start));
            }
            Ok(unit)
        }
        _ => Err(malformed(InputProblem::InvalidMutf8, start)),
    }
}

enum NameSyntax {
    Simple,
    ClassPath,
}

fn validate_name_units(
    mut reader: Units<'_>,
    syntax: NameSyntax,
    control: &mut impl InputControl,
) -> Result<(), InputError> {
    let mut component = false;
    while let Some(unit) = reader.next(control)? {
        if matches!(syntax, NameSyntax::ClassPath) && unit == u16::from(b'/') {
            if !component {
                return Err(malformed(
                    InputProblem::InvalidName,
                    reader.cursor.position_byte(),
                ));
            }
            component = false;
            continue;
        }
        let scalar = if (0xd800..=0xdbff).contains(&unit) {
            let low = reader.next(control)?.ok_or_else(|| {
                malformed(InputProblem::InvalidName, reader.cursor.position_byte())
            })?;
            if !(0xdc00..=0xdfff).contains(&low) {
                return Err(malformed(
                    InputProblem::InvalidName,
                    reader.cursor.position_byte(),
                ));
            }
            0x10000 + ((u32::from(unit) - 0xd800) << 10) + u32::from(low) - 0xdc00
        } else {
            u32::from(unit)
        };
        if !matches!(scalar, 0x41..=0x5a | 0x61..=0x7a | 0x30..=0x39 | 0x24 | 0x2d | 0x5f | 0xa1..=0x1fff | 0x2010..=0x2027 | 0x2030..=0xd7ff | 0xe000..=0xffef | 0x10000..=0x10ffff)
        {
            return Err(malformed(
                InputProblem::InvalidName,
                reader.cursor.position_byte(),
            ));
        }
        component = true;
    }
    if !component {
        return Err(malformed(
            InputProblem::InvalidName,
            reader.cursor.position_byte(),
        ));
    }
    Ok(())
}
