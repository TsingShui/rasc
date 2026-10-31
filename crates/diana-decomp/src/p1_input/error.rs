use thiserror::Error;

use crate::{ByteLength, ByteOffset, ControlError, FailureKind};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InputProblem {
    RangeOverflow,
    Truncated,
    InvalidLeb32,
    FileSizeMismatch,
    InvalidHeaderSize,
    InvalidEndianTag,
    InvalidSectionPair,
    InvalidAlignment,
    OverlappingSections,
    InvalidTableCount,
    InvalidMap,
    InvalidReference,
    InvalidMutf8,
    InvalidName,
    InvalidDescriptor,
    InvalidPrototype,
    InvalidClassData,
    InvalidCode,
    InvalidHandler,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnsupportedInput {
    Container,
    ReverseEndian,
    MapType(u16),
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
    #[error(transparent)]
    Control(#[from] ControlError),
}

impl InputError {
    pub fn kind(&self) -> FailureKind {
        match self {
            Self::Malformed { .. } => FailureKind::Malformed,
            Self::Unsupported { .. } => FailureKind::Unsupported,
            Self::PlatformCapacity { .. } => FailureKind::BudgetExceeded,
            Self::Control(error) => error.kind(),
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
