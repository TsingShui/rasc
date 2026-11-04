use thiserror::Error;

use crate::{ByteLength, ByteOffset, FailureKind, TypeId};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InputProblem {
    RangeOverflow,
    Truncated,
    InvalidLeb32,
    FileSizeMismatch,
    InvalidHeaderSize,
    InvalidHeaderChain,
    InvalidEndianTag,
    InvalidSectionPair,
    InvalidAlignment,
    OverlappingSections,
    InvalidTableCount,
    InvalidMap,
    InvalidReference,
    ConflictingOrigin,
    DuplicateImage,
    InvalidMutf8,
    InvalidName,
    InvalidDescriptor,
    InvalidPrototype,
    InvalidClassData,
    InvalidCode,
    InvalidHandler,
    InvalidDebug,
    InvalidEncodedValue,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnsupportedInput {
    Container,
    ReverseEndian,
    MapType(u16),
    EncodedValueTag(u8),
    OwnershipEvidenceMissing(TypeId),
    AnonymousOwnership,
}

/// Errors retain physical byte coordinates without allocating diagnostic strings.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum InputError {
    #[error("malformed DEX at file byte {offset_byte}: {code:?}")]
    Malformed {
        code: InputProblem,
        offset_byte: ByteOffset,
    },
    #[error("unsupported DEX at file byte {offset_byte}: {reason:?}")]
    Unsupported {
        reason: UnsupportedInput,
        offset_byte: ByteOffset,
    },
    #[error("input size is not representable on this platform at file byte {offset_byte}")]
    PlatformCapacity { offset_byte: ByteOffset },
    #[error("input index allocation failed at file byte {offset_byte}")]
    Allocation { offset_byte: ByteOffset },
    #[error("BUG: {0}")]
    InternalInvariant(&'static str),
}

impl InputError {
    pub fn kind(&self) -> FailureKind {
        match self {
            Self::Malformed { .. } => FailureKind::Malformed,
            Self::Unsupported { .. } => FailureKind::Unsupported,
            Self::PlatformCapacity { .. } | Self::Allocation { .. } => {
                FailureKind::ResourceExhausted
            }

            Self::InternalInvariant(_) => FailureKind::InternalInvariant,
        }
    }
}

pub(super) fn malformed(code: InputProblem, offset_byte: ByteOffset) -> InputError {
    InputError::Malformed { code, offset_byte }
}

pub(super) fn count_usize(value: u32, field_byte: ByteOffset) -> Result<usize, InputError> {
    usize::try_from(value).map_err(|_| InputError::PlatformCapacity {
        offset_byte: field_byte,
    })
}

pub(super) fn file_offset(value: u32, field_byte: ByteOffset) -> Result<ByteOffset, InputError> {
    count_usize(value, field_byte).map(ByteOffset)
}

pub(super) fn file_length(value: u32, field_byte: ByteOffset) -> Result<ByteLength, InputError> {
    count_usize(value, field_byte).map(ByteLength)
}
