use super::values::{EncodedKind, ValueStep};
use super::{MethodInput, ProgramIndex};
use crate::pipeline::control::InputControl;
use crate::substrate::owned::{copy_units, push};
use crate::{ByteOffset, ByteRange, RequestError};

pub(crate) struct Signature {
    units: Vec<u16>,
    _range: ByteRange,
}
impl Signature {
    pub(crate) fn units(&self) -> &[u16] {
        &self.units
    }
}

pub(crate) fn class(
    index: &ProgramIndex<'_>,
    id: crate::ClassDefId,
    control: &mut impl InputControl,
) -> Result<Option<Signature>, RequestError> {
    let context = index.class_context(id, control)?;
    if context.annotations_offset == ByteOffset(0) {
        return Ok(None);
    }
    read(
        index,
        index
            .annotation_directory(context.annotations_offset, control)?
            .class,
        false,
        control,
    )
}

pub(crate) fn method(
    input: &MethodInput<'_, '_>,
    control: &mut impl InputControl,
) -> Result<Option<Signature>, RequestError> {
    let raw = match input.raw() {
        super::RawPrintable::Method(raw) => raw.annotations(),
        super::RawPrintable::Declaration(raw) => raw.annotations(),
    };
    let Some(range) = raw.method_range() else {
        return Ok(None);
    };
    read(input.program(), range.start, true, control)
}
fn read(
    index: &ProgramIndex<'_>,
    offset: ByteOffset,
    method_parameters: bool,
    control: &mut impl InputControl,
) -> Result<Option<Signature>, RequestError> {
    if offset == ByteOffset(0) {
        return Ok(None);
    }
    let mut offsets = index.annotation_offsets(0x1003, offset, control)?;
    let mut last = None;
    let mut result = None;
    while offsets.remaining_byte().0 != 0 {
        control.charge_work(1)?;
        let (at, visibility, ty, mut values) =
            index.next_annotation(&mut offsets, &mut last, control)?;
        let descriptor = index.type_descriptor(ty, control)?.0;
        // MethodParameters is independent system metadata, not a Signature fragment.
        if method_parameters
            && visibility == 2
            && descriptor.matches_with(
                &[
                    76, 100, 97, 108, 118, 105, 107, 47, 97, 110, 110, 111, 116, 97, 116, 105, 111,
                    110, 47, 77, 101, 116, 104, 111, 100, 80, 97, 114, 97, 109, 101, 116, 101, 114,
                    115, 59,
                ],
                control,
            )?
        {
            while values.next(index, control)?.is_some() {}
            continue;
        }
        if !descriptor.matches_with(
            &[
                76, 100, 97, 108, 118, 105, 107, 47, 97, 110, 110, 111, 116, 97, 116, 105, 111,
                110, 47, 83, 105, 103, 110, 97, 116, 117, 114, 101, 59,
            ],
            control,
        )? {
            return Ok(None);
        }
        if visibility != 2 || result.is_some() {
            return Ok(None);
        }
        let Some(ValueStep::Value(value)) = values.next(index, control)? else {
            return Ok(None);
        };
        let Some(name) = value.name else {
            return Ok(None);
        };
        if !index
            .string_with(name, control)?
            .matches_with(&[118, 97, 108, 117, 101], control)?
            || !matches!(value.kind, EncodedKind::Array { .. })
        {
            return Ok(None);
        }
        let mut units = Vec::new();
        loop {
            control.charge_work(1)?;
            match values.next(index, control)? {
                Some(ValueStep::Value(value)) => {
                    let EncodedKind::Reference(crate::raw::OperandReference::String(id)) =
                        value.kind
                    else {
                        return Ok(None);
                    };
                    for unit in copy_units(index.string_with(id, control)?, control)? {
                        push(&mut units, unit, control)?;
                    }
                }
                Some(ValueStep::EndContainer) => break,
                None => return Ok(None),
            }
        }
        if values.next(index, control)?.is_some() {
            return Ok(None);
        }
        result = Some(Signature {
            units,
            _range: ByteRange {
                start: at,
                end: values.position(),
            },
        });
    }
    Ok(result)
}
