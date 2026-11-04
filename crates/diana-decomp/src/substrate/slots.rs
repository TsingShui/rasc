//! Register slots shared by entry layout and later execution products.

use crate::RegisterId;

/// The word width of one value in the original register file.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum SlotWidth {
    One,
    Two,
}

impl SlotWidth {
    pub fn words(self) -> u32 {
        match self {
            Self::One => 1,
            Self::Two => 2,
        }
    }
}

/// A half-open register word interval. Overlap is decided by these intervals.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct RegisterSlot {
    pub start: RegisterId,
    pub width: SlotWidth,
}

impl RegisterSlot {
    pub fn new(start: RegisterId, width: SlotWidth) -> Self {
        Self { start, width }
    }

    pub fn words(self) -> u32 {
        self.width.words()
    }

    pub fn end(self) -> Option<RegisterId> {
        self.start.0.checked_add(self.words()).map(RegisterId)
    }
}
