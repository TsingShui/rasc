use super::values::{EncodedKind, ValueStep};
use super::{DexString, InputProblem, ProgramIndex, malformed};
use crate::raw::OperandReference;
use crate::substrate::owned::reserve;
use crate::{ByteLength, ByteRange, RequestError, RunContext};

pub(crate) struct ThrowsMetadata<'a> {
    _sealed: (),
    pub exceptions: Vec<DexString<'a>>,
}
pub(super) fn read<'a>(
    index: &ProgramIndex<'a>,
    range: Option<ByteRange>,
    budget: &mut RunContext<'_>,
) -> Result<ThrowsMetadata<'a>, RequestError> {
    let mut result = ThrowsMetadata {
        _sealed: (),
        exceptions: Vec::new(),
    };
    let Some(range) = range else {
        return Ok(result);
    };
    let mut offsets = index.annotation_offsets(0x1003, range.start, budget)?;
    let mut last = None;
    while offsets.remaining_byte() != ByteLength(0) {
        budget.checkpoint()?;
        let (offset, visibility, ty, mut values) =
            index.next_annotation(&mut offsets, &mut last, budget)?;
        if !index.type_name(ty, budget)?.matches_with(
            &const { crate::substrate::owned::ascii_units(*b"Ldalvik/annotation/Throws;") },
            budget,
        )? {
            while values.next(index, budget)?.is_some() {}
            continue;
        }
        let invalid = || RequestError::Input(malformed(InputProblem::InvalidEncodedValue, offset));
        if visibility != 2 || values.count != 1 {
            return Err(invalid());
        }
        let Some(ValueStep::Value(value)) = values.next(index, budget)? else {
            return Err(invalid());
        };
        let Some(name) = value.name else {
            return Err(invalid());
        };
        if !index
            .string_with(name, budget)?
            .matches_with(&[118, 97, 108, 117, 101], budget)?
        {
            return Err(invalid());
        }
        let EncodedKind::Array { count } = value.kind else {
            return Err(invalid());
        };
        for _ in 0..count {
            budget.checkpoint()?;
            let Some(ValueStep::Value(value)) = values.next(index, budget)? else {
                return Err(invalid());
            };
            let EncodedKind::Reference(OperandReference::Type(ty)) = value.kind else {
                return Err(invalid());
            };
            let (name, shape) = index.type_descriptor(ty, budget)?;
            if !matches!(shape, super::strings::TypeShape::Class) {
                return Err(invalid());
            }
            reserve(&mut result.exceptions, 1, budget)?;
            result.exceptions.push(name);
        }
        if !matches!(values.next(index, budget)?, Some(ValueStep::EndContainer))
            || values.next(index, budget)?.is_some()
        {
            return Err(invalid());
        }
    }
    Ok(result)
}
