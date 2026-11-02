use super::values::{EncodedKind, ValueStep};
use super::{DexString, InputProblem, ProgramIndex, malformed};
use crate::raw::OperandReference;
use crate::substrate::owned::reserve;
use crate::{ByteLength, ByteRange, MethodBudget, RequestError};

pub(crate) struct ThrowsMetadata<'a> {
    _sealed: (),
    pub exceptions: Vec<DexString<'a>>,
    pub other_annotations: bool,
}
pub(super) fn read<'a>(
    index: &ProgramIndex<'a>,
    range: Option<ByteRange>,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<ThrowsMetadata<'a>, RequestError> {
    let mut result = ThrowsMetadata {
        _sealed: (),
        exceptions: Vec::new(),
        other_annotations: false,
    };
    let Some(range) = range else {
        return Ok(result);
    };
    let mut offsets = index.annotation_offsets(0x1003, range.start, budget)?;
    let mut last = None;
    while offsets.remaining_byte() != ByteLength(0) {
        budget.charge_work(1)?;
        let (offset, visibility, ty, mut values) =
            index.next_annotation(&mut offsets, &mut last, budget)?;
        if !index.type_name(ty, budget)?.matches_with(
            &[
                76, 100, 97, 108, 118, 105, 107, 47, 97, 110, 110, 111, 116, 97, 116, 105, 111,
                110, 47, 84, 104, 114, 111, 119, 115, 59,
            ],
            budget,
        )? {
            let parameters = visibility == 2
                && index.type_name(ty, budget)?.matches_with(
                    &[
                        76, 100, 97, 108, 118, 105, 107, 47, 97, 110, 110, 111, 116, 97, 116, 105,
                        111, 110, 47, 77, 101, 116, 104, 111, 100, 80, 97, 114, 97, 109, 101, 116,
                        101, 114, 115, 59,
                    ],
                    budget,
                )?;
            result.other_annotations |= !parameters;
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
            budget.charge_work(1)?;
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
