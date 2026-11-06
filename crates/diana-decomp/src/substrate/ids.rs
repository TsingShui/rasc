use std::fmt;
use std::ops::Range;

/// A physical byte coordinate, not a proof that the byte exists in an input.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ByteOffset(pub usize);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ByteLength(pub usize);

impl ByteOffset {
    pub fn checked_advance(self, length: ByteLength) -> Option<Self> {
        self.0.checked_add(length.0).map(Self)
    }

    pub fn checked_distance_from(self, start: Self) -> Option<ByteLength> {
        self.0.checked_sub(start.0).map(ByteLength)
    }
}

impl fmt::Display for ByteOffset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// Half-open byte coordinates. Bounds are established by the owning reader.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct ByteRange {
    pub start: ByteOffset,
    pub end: ByteOffset,
}

impl ByteRange {
    pub fn as_range(self) -> Range<usize> {
        self.start.0..self.end.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, serde::Serialize)]
#[serde(transparent)]
pub struct CodeUnitOffset(pub u32);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CodeUnitCount(pub u32);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CodeUnitDelta(pub i32);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, serde::Serialize)]
#[serde(transparent)]
pub struct RegisterId(pub u32);

impl CodeUnitCount {
    pub fn checked_byte_length(self) -> Option<ByteLength> {
        usize::try_from(self.0).ok()?.checked_mul(2).map(ByteLength)
    }
}

impl CodeUnitOffset {
    pub fn checked_byte_offset(self) -> Option<ByteOffset> {
        usize::try_from(self.0).ok()?.checked_mul(2).map(ByteOffset)
    }

    pub fn checked_advance(self, count: CodeUnitCount) -> Option<Self> {
        self.0.checked_add(count.0).map(Self)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Utf16Offset(pub usize);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Utf16Count(pub u32);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RegisterWordCount(pub u64);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, serde::Serialize)]
#[serde(transparent)]
pub struct ClassDefId(pub u32);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, serde::Serialize)]
#[serde(transparent)]
pub struct TypeId(pub u32);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, serde::Serialize)]
#[serde(transparent)]
pub struct StringId(pub u32);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, serde::Serialize)]
#[serde(transparent)]
pub struct ProtoId(pub u32);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, serde::Serialize)]
#[serde(transparent)]
pub struct FieldId(pub u32);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, serde::Serialize)]
#[serde(transparent)]
pub struct MethodId(pub u32);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CallSiteId(pub u32);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, serde::Serialize)]
#[serde(transparent)]
pub struct MethodHandleId(pub u32);

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, serde::Serialize)]
#[serde(transparent)]
pub struct DexId(pub u32);

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct InputId(pub u64);

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
pub struct ClassKey {
    pub dex: DexId,
    pub class_def: ClassDefId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
pub struct MethodKey {
    pub class: ClassKey,
    pub method: MethodId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(transparent)]
pub struct SourceByteOffset(pub usize);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DiagnosticId(pub usize);
