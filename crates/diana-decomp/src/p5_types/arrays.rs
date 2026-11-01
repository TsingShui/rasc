use crate::input::DexString;
use crate::{MethodBudget, RequestError};
#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum PrimitiveArray {
    Boolean,
    Byte,
    Short,
    Char,
    Int,
    Float,
    Long,
    Double,
}
pub(crate) struct ArrayElement {
    kind: PrimitiveArray,
    bits: u64,
}
impl ArrayElement {
    pub(crate) fn kind(&self) -> PrimitiveArray {
        self.kind
    }
    pub(crate) fn bits(&self) -> u64 {
        self.bits
    }
}
impl PrimitiveArray {
    pub(crate) fn component_spelling(self) -> &'static str {
        match self {
            Self::Boolean => "boolean",
            Self::Byte => "byte",
            Self::Short => "short",
            Self::Char => "char",
            Self::Int => "int",
            Self::Float => "float",
            Self::Long => "long",
            Self::Double => "double",
        }
    }
    pub(crate) fn element(self, bits: u64) -> Option<ArrayElement> {
        let fits = match self.width() {
            1 => bits <= u64::from(u8::MAX),
            2 => bits <= u64::from(u16::MAX),
            4 => bits <= u64::from(u32::MAX),
            8 => true,
            _ => false,
        };
        let exact = match self {
            Self::Boolean => bits <= 1,
            Self::Float => bits & 0x7f80_0000 != 0x7f80_0000 || bits & 0x007f_ffff == 0,
            Self::Double => {
                bits & 0x7ff0_0000_0000_0000 != 0x7ff0_0000_0000_0000
                    || bits & 0x000f_ffff_ffff_ffff == 0
            }
            _ => true,
        };
        if fits && exact {
            Some(ArrayElement { kind: self, bits })
        } else {
            None
        }
    }
    pub(crate) fn descriptor(self) -> &'static [u16] {
        match self {
            Self::Boolean => &[91, 90],
            Self::Byte => &[91, 66],
            Self::Short => &[91, 83],
            Self::Char => &[91, 67],
            Self::Int => &[91, 73],
            Self::Float => &[91, 70],
            Self::Long => &[91, 74],
            Self::Double => &[91, 68],
        }
    }
    pub(crate) fn spelling(self) -> &'static str {
        match self {
            Self::Boolean => "boolean[]",
            Self::Byte => "byte[]",
            Self::Short => "short[]",
            Self::Char => "char[]",
            Self::Int => "int[]",
            Self::Float => "float[]",
            Self::Long => "long[]",
            Self::Double => "double[]",
        }
    }
    pub(crate) fn width(self) -> usize {
        match self {
            Self::Boolean | Self::Byte => 1,
            Self::Short | Self::Char => 2,
            Self::Int | Self::Float => 4,
            Self::Long | Self::Double => 8,
        }
    }
    pub(crate) fn parse(
        name: DexString<'_>,
        budget: &mut MethodBudget<'_, '_>,
    ) -> Result<Option<Self>, RequestError> {
        for kind in [
            Self::Boolean,
            Self::Byte,
            Self::Short,
            Self::Char,
            Self::Int,
            Self::Float,
            Self::Long,
            Self::Double,
        ] {
            if name.matches_with(kind.descriptor(), budget)? {
                return Ok(Some(kind));
            }
        }
        Ok(None)
    }
}
