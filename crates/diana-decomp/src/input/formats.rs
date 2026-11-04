use thiserror::Error;

use crate::{
    ByteLength, CallSiteId, CodeUnitCount, CodeUnitOffset, FailureKind, FieldId, MethodHandleId,
    MethodId, ProtoId, StringId, TypeId,
};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum DexVersion {
    V035,
    V037,
    V038,
    V039,
    V040,
    V041,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Format {
    F10x,
    F12x,
    F11n,
    F11x,
    F10t,
    F20t,
    F22x,
    F21t,
    F21s,
    F21h,
    F21c,
    F23x,
    F22b,
    F22t,
    F22s,
    F22c,
    F30t,
    F32x,
    F31i,
    F31t,
    F31c,
    F35c,
    F3rc,
    F45cc,
    F4rcc,
    F51l,
}

impl Format {
    fn width(self) -> CodeUnitCount {
        CodeUnitCount(match self {
            Self::F10x | Self::F12x | Self::F11n | Self::F11x | Self::F10t => 1,
            Self::F20t
            | Self::F22x
            | Self::F21t
            | Self::F21s
            | Self::F21h
            | Self::F21c
            | Self::F23x
            | Self::F22b
            | Self::F22t
            | Self::F22s
            | Self::F22c => 2,
            Self::F30t
            | Self::F32x
            | Self::F31i
            | Self::F31t
            | Self::F31c
            | Self::F35c
            | Self::F3rc => 3,
            Self::F45cc | Self::F4rcc => 4,
            Self::F51l => 5,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReferenceKind {
    String,
    Type,
    Field,
    Method,
    Proto,
    CallSite,
    MethodHandle,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OpcodeSpec {
    pub opcode: u8,
    pub mnemonic: &'static str,
    pub format: Format,
    pub minimum_version: DexVersion,
    pub references: [Option<ReferenceKind>; 2],
    pub(crate) registers: RegisterShape,
    pub(crate) flow: FlowKind,
    pub(crate) result: ResultRole,
    pub(crate) may_throw: bool,
}

impl OpcodeSpec {
    pub(crate) fn may_throw(self) -> bool {
        self.may_throw
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RegisterWidth {
    One,
    Two,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RegisterShape {
    Fixed([Option<RegisterWidth>; 3]),
    InvokeWords,
    ArrayWords,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum FlowKind {
    Next,
    Return,
    Throw,
    Goto,
    Conditional,
    PackedSwitch,
    SparseSwitch,
    FillArray,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ResultRole {
    Ordinary,
    MoveResult,
    MoveException,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReferenceSlot {
    First,
    Second,
}

/// Original operand identities; catalog bounds and binding remain unverified.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OperandReference {
    String(StringId),
    Type(TypeId),
    Field(FieldId),
    Method(MethodId),
    Proto(ProtoId),
    CallSite(CallSiteId),
    MethodHandle(MethodHandleId),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IndexedRef {
    pub target: OperandReference,
    pub operand_slot: ReferenceSlot,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LayoutKind {
    Instruction(OpcodeSpec),
    PackedSwitch {
        entries: u16,
    },
    SparseSwitch {
        entries: u16,
    },
    ArrayData {
        element_width: ByteLength,
        element_count: u32,
    },
}

/// A complete layout at one PC, constructed only after range/format checks.
/// It does not certify registers, branch targets, reference bounds or payload keys.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InsnLayout {
    kind: LayoutKind,
    width: CodeUnitCount,
    references: [Option<IndexedRef>; 2],
}

impl InsnLayout {
    pub fn kind(&self) -> LayoutKind {
        self.kind
    }
    pub fn width(&self) -> CodeUnitCount {
        self.width
    }
    pub fn references(&self) -> [Option<IndexedRef>; 2] {
        self.references
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LayoutProblem {
    Truncated,
    RangeOverflow,
    InvalidPadding,
    InvalidWordCount,
    UnalignedPayload,
    InvalidElementWidth,
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum RawDecodeError {
    #[error("unknown instruction encoding {word:#06x} at {pc:?}")]
    UnknownEncoding { pc: CodeUnitOffset, word: u16 },
    #[error("opcode {opcode:#04x} at {pc:?} requires {minimum:?}, not {actual:?}")]
    UnsupportedVersion {
        pc: CodeUnitOffset,
        opcode: u8,
        minimum: DexVersion,
        actual: DexVersion,
    },
    #[error("malformed instruction at {pc:?}: {problem:?}")]
    Malformed {
        pc: CodeUnitOffset,
        problem: LayoutProblem,
    },
}

impl RawDecodeError {
    pub fn kind(&self) -> FailureKind {
        match self {
            Self::Malformed { .. } => FailureKind::Malformed,
            Self::UnknownEncoding { .. } | Self::UnsupportedVersion { .. } => {
                FailureKind::Unsupported
            }
        }
    }
}

fn bad(pc: CodeUnitOffset, problem: LayoutProblem) -> RawDecodeError {
    RawDecodeError::Malformed { pc, problem }
}

/// Stateless, allocation-free and constant-work. Callers charge/check each loop
/// iteration. Payload contents and whole-method semantics need their own validator.
pub fn instruction_layout(
    code_bytes: &[u8],
    pc: CodeUnitOffset,
    version: DexVersion,
) -> Result<InsnLayout, RawDecodeError> {
    let first = word(code_bytes, pc, 0)?;
    let spec = match encoding_head(first, pc, version)? {
        EncodingHead::Instruction(spec) => spec,
        EncodingHead::Payload => return payload(code_bytes, pc, first),
    };
    let width = span(code_bytes, pc, u64::from(spec.format.width().0))?;
    if matches!(
        spec.format,
        Format::F10x | Format::F20t | Format::F30t | Format::F32x
    ) && first & 0xff00 != 0
    {
        return Err(bad(pc, LayoutProblem::InvalidPadding));
    }
    if matches!(spec.format, Format::F35c | Format::F45cc) && first >> 12 > 5 {
        return Err(bad(pc, LayoutProblem::InvalidWordCount));
    }
    let mut references = [None; 2];
    for (slot, kind) in spec.references.into_iter().enumerate() {
        if let Some(kind) = kind {
            let index = if slot == 1 {
                u32::from(word(code_bytes, pc, 3)?)
            } else if spec.format == Format::F31c {
                wide_word(code_bytes, pc, 1)?
            } else {
                u32::from(word(code_bytes, pc, 1)?)
            };
            let target = match kind {
                ReferenceKind::String => OperandReference::String(StringId(index)),
                ReferenceKind::Type => OperandReference::Type(TypeId(index)),
                ReferenceKind::Field => OperandReference::Field(FieldId(index)),
                ReferenceKind::Method => OperandReference::Method(MethodId(index)),
                ReferenceKind::Proto => OperandReference::Proto(ProtoId(index)),
                ReferenceKind::CallSite => OperandReference::CallSite(CallSiteId(index)),
                ReferenceKind::MethodHandle => {
                    OperandReference::MethodHandle(MethodHandleId(index))
                }
            };
            references[slot] = Some(IndexedRef {
                target,
                operand_slot: if slot == 0 {
                    ReferenceSlot::First
                } else {
                    ReferenceSlot::Second
                },
            });
        }
    }
    Ok(InsnLayout {
        kind: LayoutKind::Instruction(spec),
        width,
        references,
    })
}

pub(super) fn word(bytes: &[u8], pc: CodeUnitOffset, delta: u32) -> Result<u16, RawDecodeError> {
    let unit =
        pc.0.checked_add(delta)
            .ok_or_else(|| bad(pc, LayoutProblem::RangeOverflow))?;
    let start = usize::try_from(unit)
        .ok()
        .and_then(|n| n.checked_mul(2))
        .ok_or_else(|| bad(pc, LayoutProblem::RangeOverflow))?;
    let end = start
        .checked_add(2)
        .ok_or_else(|| bad(pc, LayoutProblem::RangeOverflow))?;
    let data = bytes
        .get(start..end)
        .ok_or_else(|| bad(pc, LayoutProblem::Truncated))?;
    Ok(u16::from_le_bytes([data[0], data[1]]))
}

pub(crate) fn wide_word(
    bytes: &[u8],
    pc: CodeUnitOffset,
    delta: u32,
) -> Result<u32, RawDecodeError> {
    let next = delta
        .checked_add(1)
        .ok_or_else(|| bad(pc, LayoutProblem::RangeOverflow))?;
    Ok(u32::from(word(bytes, pc, delta)?) | (u32::from(word(bytes, pc, next)?) << 16))
}

fn span(bytes: &[u8], pc: CodeUnitOffset, width: u64) -> Result<CodeUnitCount, RawDecodeError> {
    let start = usize::try_from(pc.0)
        .ok()
        .and_then(|n| n.checked_mul(2))
        .ok_or_else(|| bad(pc, LayoutProblem::RangeOverflow))?;
    let remaining = bytes
        .len()
        .checked_sub(start)
        .ok_or_else(|| bad(pc, LayoutProblem::Truncated))?;
    let size = width
        .checked_mul(2)
        .ok_or_else(|| bad(pc, LayoutProblem::RangeOverflow))?;
    if size > remaining as u64 {
        return Err(bad(pc, LayoutProblem::Truncated));
    }
    Ok(CodeUnitCount(
        u32::try_from(width).map_err(|_| bad(pc, LayoutProblem::RangeOverflow))?,
    ))
}

fn payload(bytes: &[u8], pc: CodeUnitOffset, signature: u16) -> Result<InsnLayout, RawDecodeError> {
    let (kind, width) = match signature {
        0x0100 | 0x0200 | 0x0300 if !pc.0.is_multiple_of(2) => {
            return Err(bad(pc, LayoutProblem::UnalignedPayload));
        }
        0x0100 | 0x0200 => {
            let entries = word(bytes, pc, 1)?;
            let (base, stride, kind) = if signature == 0x0100 {
                (4u64, 2u64, LayoutKind::PackedSwitch { entries })
            } else {
                (2, 4, LayoutKind::SparseSwitch { entries })
            };
            let width = u64::from(entries)
                .checked_mul(stride)
                .and_then(|n| n.checked_add(base))
                .ok_or_else(|| bad(pc, LayoutProblem::RangeOverflow))?;
            (kind, width)
        }
        0x0300 => {
            let element_width = word(bytes, pc, 1)?;
            if !matches!(element_width, 1 | 2 | 4 | 8) {
                return Err(bad(pc, LayoutProblem::InvalidElementWidth));
            }
            let element_count = wide_word(bytes, pc, 2)?;
            let data = u64::from(element_width)
                .checked_mul(u64::from(element_count))
                .ok_or_else(|| bad(pc, LayoutProblem::RangeOverflow))?;
            let width = data
                .div_ceil(2)
                .checked_add(4)
                .ok_or_else(|| bad(pc, LayoutProblem::RangeOverflow))?;
            (
                LayoutKind::ArrayData {
                    element_width: ByteLength(usize::from(element_width)),
                    element_count,
                },
                width,
            )
        }
        _ => {
            return Err(RawDecodeError::UnknownEncoding {
                pc,
                word: signature,
            });
        }
    };
    Ok(InsnLayout {
        kind,
        width: span(bytes, pc, width)?,
        references: [None; 2],
    })
}

pub(crate) enum EncodingHead {
    Instruction(OpcodeSpec),
    Payload,
}

pub(crate) fn encoding_head(
    first: u16,
    pc: CodeUnitOffset,
    version: DexVersion,
) -> Result<EncodingHead, RawDecodeError> {
    if first & 0xff == 0 && first != 0 {
        return match first {
            0x0100 | 0x0200 | 0x0300 => Ok(EncodingHead::Payload),
            _ => Err(RawDecodeError::UnknownEncoding { pc, word: first }),
        };
    }
    let opcode = first.to_le_bytes()[0];
    let spec = opcode_spec(opcode).ok_or(RawDecodeError::UnknownEncoding { pc, word: first })?;
    if version < spec.minimum_version {
        return Err(RawDecodeError::UnsupportedVersion {
            pc,
            opcode,
            minimum: spec.minimum_version,
            actual: version,
        });
    }
    Ok(EncodingHead::Instruction(spec))
}

pub(super) fn opcode_spec(opcode: u8) -> Option<OpcodeSpec> {
    use Format::*;
    use ReferenceKind as R;
    let mut references = [None; 2];
    let mut minimum_version = DexVersion::V035;
    let (mnemonic, format) = match opcode {
        0x00 => ("nop", F10x),
        0x01..=0x09 => {
            let index = usize::from(opcode - 1);
            (
                [
                    "move",
                    "move/from16",
                    "move/16",
                    "move-wide",
                    "move-wide/from16",
                    "move-wide/16",
                    "move-object",
                    "move-object/from16",
                    "move-object/16",
                ][index],
                [F12x, F22x, F32x][index % 3],
            )
        }
        0x0a..=0x0d => (
            [
                "move-result",
                "move-result-wide",
                "move-result-object",
                "move-exception",
            ][usize::from(opcode - 0x0a)],
            F11x,
        ),
        0x0e => ("return-void", F10x),
        0x0f..=0x11 => (
            ["return", "return-wide", "return-object"][usize::from(opcode - 0x0f)],
            F11x,
        ),
        0x12..=0x19 => {
            let index = usize::from(opcode - 0x12);
            (
                [
                    "const/4",
                    "const/16",
                    "const",
                    "const/high16",
                    "const-wide/16",
                    "const-wide/32",
                    "const-wide",
                    "const-wide/high16",
                ][index],
                [F11n, F21s, F31i, F21h, F21s, F31i, F51l, F21h][index],
            )
        }
        0x1a => {
            references[0] = Some(R::String);
            ("const-string", F21c)
        }
        0x1b => {
            references[0] = Some(R::String);
            ("const-string/jumbo", F31c)
        }
        0x1c => {
            references[0] = Some(R::Type);
            ("const-class", F21c)
        }
        0x1d..=0x1e => (
            ["monitor-enter", "monitor-exit"][usize::from(opcode - 0x1d)],
            F11x,
        ),
        0x1f => {
            references[0] = Some(R::Type);
            ("check-cast", F21c)
        }
        0x20 => {
            references[0] = Some(R::Type);
            ("instance-of", F22c)
        }
        0x21 => ("array-length", F12x),
        0x22 => {
            references[0] = Some(R::Type);
            ("new-instance", F21c)
        }
        0x23 => {
            references[0] = Some(R::Type);
            ("new-array", F22c)
        }
        0x24 => {
            references[0] = Some(R::Type);
            ("filled-new-array", F35c)
        }
        0x25 => {
            references[0] = Some(R::Type);
            ("filled-new-array/range", F3rc)
        }
        0x26 => ("fill-array-data", F31t),
        0x27 => ("throw", F11x),
        0x28 => ("goto", F10t),
        0x29 => ("goto/16", F20t),
        0x2a => ("goto/32", F30t),
        0x2b..=0x2c => (
            ["packed-switch", "sparse-switch"][usize::from(opcode - 0x2b)],
            F31t,
        ),
        0x2d..=0x31 => (
            [
                "cmpl-float",
                "cmpg-float",
                "cmpl-double",
                "cmpg-double",
                "cmp-long",
            ][usize::from(opcode - 0x2d)],
            F23x,
        ),
        0x32..=0x37 => (
            ["if-eq", "if-ne", "if-lt", "if-ge", "if-gt", "if-le"][usize::from(opcode - 0x32)],
            F22t,
        ),
        0x38..=0x3d => (
            ["if-eqz", "if-nez", "if-ltz", "if-gez", "if-gtz", "if-lez"]
                [usize::from(opcode - 0x38)],
            F21t,
        ),
        0x44..=0x51 => (
            [
                "aget",
                "aget-wide",
                "aget-object",
                "aget-boolean",
                "aget-byte",
                "aget-char",
                "aget-short",
                "aput",
                "aput-wide",
                "aput-object",
                "aput-boolean",
                "aput-byte",
                "aput-char",
                "aput-short",
            ][usize::from(opcode - 0x44)],
            F23x,
        ),
        0x52..=0x5f => {
            references[0] = Some(R::Field);
            (
                [
                    "iget",
                    "iget-wide",
                    "iget-object",
                    "iget-boolean",
                    "iget-byte",
                    "iget-char",
                    "iget-short",
                    "iput",
                    "iput-wide",
                    "iput-object",
                    "iput-boolean",
                    "iput-byte",
                    "iput-char",
                    "iput-short",
                ][usize::from(opcode - 0x52)],
                F22c,
            )
        }
        0x60..=0x6d => {
            references[0] = Some(R::Field);
            (
                [
                    "sget",
                    "sget-wide",
                    "sget-object",
                    "sget-boolean",
                    "sget-byte",
                    "sget-char",
                    "sget-short",
                    "sput",
                    "sput-wide",
                    "sput-object",
                    "sput-boolean",
                    "sput-byte",
                    "sput-char",
                    "sput-short",
                ][usize::from(opcode - 0x60)],
                F21c,
            )
        }
        0x6e..=0x72 => {
            references[0] = Some(R::Method);
            (
                [
                    "invoke-virtual",
                    "invoke-super",
                    "invoke-direct",
                    "invoke-static",
                    "invoke-interface",
                ][usize::from(opcode - 0x6e)],
                F35c,
            )
        }
        0x74..=0x78 => {
            references[0] = Some(R::Method);
            (
                [
                    "invoke-virtual/range",
                    "invoke-super/range",
                    "invoke-direct/range",
                    "invoke-static/range",
                    "invoke-interface/range",
                ][usize::from(opcode - 0x74)],
                F3rc,
            )
        }
        0x7b..=0x8f => (
            [
                "neg-int",
                "not-int",
                "neg-long",
                "not-long",
                "neg-float",
                "neg-double",
                "int-to-long",
                "int-to-float",
                "int-to-double",
                "long-to-int",
                "long-to-float",
                "long-to-double",
                "float-to-int",
                "float-to-long",
                "float-to-double",
                "double-to-int",
                "double-to-long",
                "double-to-float",
                "int-to-byte",
                "int-to-char",
                "int-to-short",
            ][usize::from(opcode - 0x7b)],
            F12x,
        ),
        0x90..=0xaf => (
            [
                "add-int",
                "sub-int",
                "mul-int",
                "div-int",
                "rem-int",
                "and-int",
                "or-int",
                "xor-int",
                "shl-int",
                "shr-int",
                "ushr-int",
                "add-long",
                "sub-long",
                "mul-long",
                "div-long",
                "rem-long",
                "and-long",
                "or-long",
                "xor-long",
                "shl-long",
                "shr-long",
                "ushr-long",
                "add-float",
                "sub-float",
                "mul-float",
                "div-float",
                "rem-float",
                "add-double",
                "sub-double",
                "mul-double",
                "div-double",
                "rem-double",
            ][usize::from(opcode - 0x90)],
            F23x,
        ),
        0xb0..=0xcf => (
            [
                "add-int/2addr",
                "sub-int/2addr",
                "mul-int/2addr",
                "div-int/2addr",
                "rem-int/2addr",
                "and-int/2addr",
                "or-int/2addr",
                "xor-int/2addr",
                "shl-int/2addr",
                "shr-int/2addr",
                "ushr-int/2addr",
                "add-long/2addr",
                "sub-long/2addr",
                "mul-long/2addr",
                "div-long/2addr",
                "rem-long/2addr",
                "and-long/2addr",
                "or-long/2addr",
                "xor-long/2addr",
                "shl-long/2addr",
                "shr-long/2addr",
                "ushr-long/2addr",
                "add-float/2addr",
                "sub-float/2addr",
                "mul-float/2addr",
                "div-float/2addr",
                "rem-float/2addr",
                "add-double/2addr",
                "sub-double/2addr",
                "mul-double/2addr",
                "div-double/2addr",
                "rem-double/2addr",
            ][usize::from(opcode - 0xb0)],
            F12x,
        ),
        0xd0..=0xd7 => (
            [
                "add-int/lit16",
                "rsub-int",
                "mul-int/lit16",
                "div-int/lit16",
                "rem-int/lit16",
                "and-int/lit16",
                "or-int/lit16",
                "xor-int/lit16",
            ][usize::from(opcode - 0xd0)],
            F22s,
        ),
        0xd8..=0xe2 => (
            [
                "add-int/lit8",
                "rsub-int/lit8",
                "mul-int/lit8",
                "div-int/lit8",
                "rem-int/lit8",
                "and-int/lit8",
                "or-int/lit8",
                "xor-int/lit8",
                "shl-int/lit8",
                "shr-int/lit8",
                "ushr-int/lit8",
            ][usize::from(opcode - 0xd8)],
            F22b,
        ),
        0xfa..=0xfb => {
            minimum_version = DexVersion::V038;
            references = [Some(R::Method), Some(R::Proto)];
            if opcode == 0xfa {
                ("invoke-polymorphic", F45cc)
            } else {
                ("invoke-polymorphic/range", F4rcc)
            }
        }
        0xfc..=0xfd => {
            minimum_version = DexVersion::V038;
            references[0] = Some(R::CallSite);
            if opcode == 0xfc {
                ("invoke-custom", F35c)
            } else {
                ("invoke-custom/range", F3rc)
            }
        }
        0xfe..=0xff => {
            minimum_version = DexVersion::V039;
            if opcode == 0xfe {
                references[0] = Some(R::MethodHandle);
                ("const-method-handle", F21c)
            } else {
                references[0] = Some(R::Proto);
                ("const-method-type", F21c)
            }
        }
        _ => return None,
    };
    Some(OpcodeSpec {
        opcode,
        mnemonic,
        format,
        minimum_version,
        references,
        registers: register_shape(opcode, format),
        flow: match opcode {
            0x0e..=0x11 => FlowKind::Return,
            0x26 => FlowKind::FillArray,
            0x27 => FlowKind::Throw,
            0x28..=0x2a => FlowKind::Goto,
            0x2b => FlowKind::PackedSwitch,
            0x2c => FlowKind::SparseSwitch,
            0x32..=0x3d => FlowKind::Conditional,
            _ => FlowKind::Next,
        },
        result: match opcode {
            0x0a..=0x0c => ResultRole::MoveResult,
            0x0d => ResultRole::MoveException,
            _ => ResultRole::Ordinary,
        },
        may_throw: matches!(opcode, 0x1a..=0x27 | 0x44..=0x72 | 0x74..=0x78 | 0x93..=0x94 | 0x9e..=0x9f | 0xb3..=0xb4 | 0xbe..=0xbf | 0xd3..=0xd4 | 0xdb..=0xdc | 0xfa..=0xff),
    })
}

fn register_shape(opcode: u8, format: Format) -> RegisterShape {
    use Format::*;
    use RegisterWidth::{One, Two};
    let mut widths = match format {
        F10x | F10t | F20t | F30t => [None; 3],
        F12x | F22x | F32x | F22b | F22t | F22s | F22c => [Some(One), Some(One), None],
        F23x => [Some(One); 3],
        F35c | F3rc | F45cc | F4rcc => {
            return if matches!(opcode, 0x24..=0x25) {
                RegisterShape::ArrayWords
            } else {
                RegisterShape::InvokeWords
            };
        }
        _ => [Some(One), None, None],
    };
    match opcode {
        0x04..=0x06 => widths = [Some(Two), Some(Two), None],
        0x0b | 0x10 | 0x16..=0x19 | 0x45 | 0x4c | 0x53 | 0x5a | 0x61 | 0x68 => {
            widths[0] = Some(Two)
        }
        0x2f..=0x31 => {
            widths[1] = Some(Two);
            widths[2] = Some(Two);
        }
        0x7b..=0x8f => {
            let (a, b) = [
                (One, One),
                (One, One),
                (Two, Two),
                (Two, Two),
                (One, One),
                (Two, Two),
                (Two, One),
                (One, One),
                (Two, One),
                (One, Two),
                (One, Two),
                (Two, Two),
                (One, One),
                (Two, One),
                (Two, One),
                (One, Two),
                (Two, Two),
                (One, Two),
                (One, One),
                (One, One),
                (One, One),
            ][usize::from(opcode - 0x7b)];
            widths = [Some(a), Some(b), None];
        }
        0x9b..=0xa5 | 0xab..=0xaf => {
            widths = [Some(Two); 3];
            if matches!(opcode, 0xa3..=0xa5) {
                widths[2] = Some(One);
            }
        }
        0xbb..=0xc5 | 0xcb..=0xcf => {
            widths = [Some(Two), Some(Two), None];
            if matches!(opcode, 0xc3..=0xc5) {
                widths[1] = Some(One);
            }
        }
        _ => {}
    }
    RegisterShape::Fixed(widths)
}
