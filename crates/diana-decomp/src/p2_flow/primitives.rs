#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum ScalarType {
    Int,
    Long,
    Float,
    Double,
}
impl ScalarType {
    pub(crate) fn words(self) -> usize {
        if matches!(self, Self::Long | Self::Double) {
            2
        } else {
            1
        }
    }
}
#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum Compare {
    Long,
    FloatL,
    FloatG,
    DoubleL,
    DoubleG,
}
impl Compare {
    pub(crate) fn operand(self) -> ScalarType {
        match self {
            Self::Long => ScalarType::Long,
            Self::FloatL | Self::FloatG => ScalarType::Float,
            Self::DoubleL | Self::DoubleG => ScalarType::Double,
        }
    }
    pub(crate) fn bias(self) -> Option<i8> {
        match self {
            Self::Long => None,
            Self::FloatL | Self::DoubleL => Some(-1),
            _ => Some(1),
        }
    }
}
#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum Unary {
    Neg,
    Not,
    Byte,
    Char,
    Short,
    NegLong,
    NotLong,
    IntToLong,
    LongToInt,
    NegFloat,
    NegDouble,
    Convert { from: ScalarType, to: ScalarType },
}
impl Unary {
    pub(crate) fn signature(self) -> (ScalarType, ScalarType) {
        use ScalarType::*;
        match self {
            Self::NegLong | Self::NotLong => (Long, Long),
            Self::IntToLong => (Int, Long),
            Self::LongToInt => (Long, Int),
            Self::NegFloat => (Float, Float),
            Self::NegDouble => (Double, Double),
            Self::Convert { from, to } => (from, to),
            _ => (Int, Int),
        }
    }
}
