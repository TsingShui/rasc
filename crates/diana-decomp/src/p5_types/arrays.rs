use crate::input::DexString;
use crate::{MethodBudget, RequestError};
#[derive(Clone, Copy)]
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
impl PrimitiveArray {
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
