use crate::input::DexString;
use crate::{RequestError, RunContext};
#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) struct IntRange {
    low: i32,
    high: i32,
}
impl IntRange {
    pub(crate) const ALL: Self = Self {
        low: i32::MIN,
        high: i32::MAX,
    };
    pub(crate) const BOOLEAN: Self = Self { low: 0, high: 1 };
    pub(crate) fn literal(value: i32) -> Self {
        Self {
            low: value,
            high: value,
        }
    }
    pub(crate) fn hull(self, other: Self) -> Self {
        Self {
            low: self.low.min(other.low),
            high: self.high.max(other.high),
        }
    }
    pub(crate) fn within(self, target: Self) -> bool {
        self.low >= target.low && self.high <= target.high
    }
    pub(crate) fn known01(self) -> Option<Self> {
        let low = self.low.max(0);
        let high = self.high.min(1);
        if low <= high {
            Some(Self { low, high })
        } else {
            None
        }
    }
}
#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum Narrow {
    Byte,
    Short,
    Char,
}
impl Narrow {
    pub(crate) fn spelling(self) -> &'static str {
        match self {
            Self::Byte => "byte",
            Self::Short => "short",
            Self::Char => "char",
        }
    }
    pub(crate) fn range(self) -> IntRange {
        match self {
            Self::Byte => IntRange {
                low: i32::from(i8::MIN),
                high: i32::from(i8::MAX),
            },
            Self::Short => IntRange {
                low: i32::from(i16::MIN),
                high: i32::from(i16::MAX),
            },
            Self::Char => IntRange {
                low: 0,
                high: i32::from(u16::MAX),
            },
        }
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum BuiltinReference {
    Object,
    String,
}
impl BuiltinReference {
    pub(crate) fn descriptor(self) -> &'static [u16] {
        match self {
            Self::Object => &const { crate::substrate::owned::ascii_units(*b"Ljava/lang/Object;") },
            Self::String => &const { crate::substrate::owned::ascii_units(*b"Ljava/lang/String;") },
        }
    }
    pub(crate) fn spelling(self) -> &'static str {
        match self {
            Self::Object => "java.lang.Object",
            Self::String => "java.lang.String",
        }
    }
}

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
        budget: &mut RunContext<'_>,
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
