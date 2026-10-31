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
