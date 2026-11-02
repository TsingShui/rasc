use crate::substrate::primitive_types::PrimitiveArray;
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
}
