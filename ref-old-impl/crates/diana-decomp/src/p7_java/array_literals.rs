use crate::RequestError;
use crate::substrate::primitive_types::PrimitiveArray;

pub(crate) enum Literal {
    Boolean(bool),
    Byte(i8),
    Short(i16),
    Char(u16),
    Int(i32),
    Long(i64),
    Float(u32),
    Double(u64),
}
pub(super) fn lower(value: &crate::p4_types::ArrayElement) -> Result<Literal, RequestError> {
    let bits = value.bits();
    let bad = || RequestError::InternalInvariant("typed array literal exceeded element width");
    Ok(match value.kind() {
        PrimitiveArray::Boolean => Literal::Boolean(bits != 0),
        PrimitiveArray::Byte => Literal::Byte(i8::from_le_bytes(
            u8::try_from(bits).map_err(|_| bad())?.to_le_bytes(),
        )),
        PrimitiveArray::Short => Literal::Short(i16::from_le_bytes(
            u16::try_from(bits).map_err(|_| bad())?.to_le_bytes(),
        )),
        PrimitiveArray::Char => Literal::Char(u16::try_from(bits).map_err(|_| bad())?),
        PrimitiveArray::Int => Literal::Int(i32::from_le_bytes(
            u32::try_from(bits).map_err(|_| bad())?.to_le_bytes(),
        )),
        PrimitiveArray::Long => Literal::Long(i64::from_le_bytes(bits.to_le_bytes())),
        PrimitiveArray::Float => Literal::Float(u32::try_from(bits).map_err(|_| bad())?),
        PrimitiveArray::Double => Literal::Double(bits),
    })
}
impl Literal {
    pub(super) fn kind(&self) -> PrimitiveArray {
        match self {
            Self::Boolean(_) => PrimitiveArray::Boolean,
            Self::Byte(_) => PrimitiveArray::Byte,
            Self::Short(_) => PrimitiveArray::Short,
            Self::Char(_) => PrimitiveArray::Char,
            Self::Int(_) => PrimitiveArray::Int,
            Self::Long(_) => PrimitiveArray::Long,
            Self::Float(_) => PrimitiveArray::Float,
            Self::Double(_) => PrimitiveArray::Double,
        }
    }
    pub(super) fn bits(&self) -> u64 {
        match *self {
            Self::Boolean(v) => u64::from(v),
            Self::Byte(v) => u64::from(u8::from_le_bytes(v.to_le_bytes())),
            Self::Short(v) => u64::from(u16::from_le_bytes(v.to_le_bytes())),
            Self::Char(v) => u64::from(v),
            Self::Int(v) => u64::from(u32::from_le_bytes(v.to_le_bytes())),
            Self::Long(v) => u64::from_le_bytes(v.to_le_bytes()),
            Self::Float(v) => u64::from(v),
            Self::Double(v) => v,
        }
    }
}
