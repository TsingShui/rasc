use super::MethodError;
use super::values::EncodedValues;
use super::{
    ClassView, Cursor, InputError, InputProblem, ProgramIndex, count_usize, file_offset, malformed,
};
use crate::pipeline::control::InputControl;
use crate::{ByteLength, ByteOffset, ByteRange, FieldId, MethodId, TypeId};

pub(crate) struct AnnotationDirectory<'a> {
    pub class: ByteOffset,
    pub fields: Cursor<'a>,
    pub methods: Cursor<'a>,
    pub parameters: Cursor<'a>,
}

fn records<'a>(
    index: &ProgramIndex<'a>,
    cursor: &mut Cursor<'a>,
    count: u32,
    stride: ByteLength,
    control: &mut impl InputControl,
) -> Result<Cursor<'a>, InputError> {
    let range = cursor.skip(count_usize(count, cursor.position_byte())?, stride, control)?;
    control.checkpoint()?;
    Cursor::bounded(index.bytes, range.start, range.end)
}

impl<'a> ProgramIndex<'a> {
    pub(crate) fn no_class_annotations(
        &self,
        directory: ByteOffset,
        control: &mut impl InputControl,
    ) -> Result<bool, InputError> {
        Ok(directory.0 == 0 || self.annotation_directory(directory, control)?.class.0 == 0)
    }
    pub(crate) fn only_ownership_annotations(
        &self,
        directory: ByteOffset,
        control: &mut impl InputControl,
    ) -> Result<bool, crate::RequestError> {
        if directory.0 == 0 {
            return Ok(true);
        }
        let offset = self.annotation_directory(directory, control)?.class;
        if offset.0 == 0 {
            return Ok(true);
        }
        let mut offsets = self.annotation_offsets(0x1003, offset, control)?;
        let mut last = None;
        while offsets.remaining_byte().0 != 0 {
            control.checkpoint()?;
            let (_, visibility, ty, _) = self.next_annotation(&mut offsets, &mut last, control)?;
            let name = crate::substrate::owned::copy_units(self.type_name(ty, control)?, control)?;
            let mut known = false;
            for allowed in [
                "Ldalvik/annotation/EnclosingClass;",
                "Ldalvik/annotation/InnerClass;",
                "Ldalvik/annotation/MemberClasses;",
            ] {
                control.checkpoint()?;
                known |= name.iter().copied().eq(allowed.encode_utf16());
            }
            if visibility != 2 || !known {
                return Ok(false);
            }
        }
        Ok(true)
    }
    pub(crate) fn annotation_directory(
        &self,
        offset: ByteOffset,
        control: &mut impl InputControl,
    ) -> Result<AnnotationDirectory<'a>, InputError> {
        let mut cursor = self.item(0x2006, offset, control)?;
        let class = file_offset(cursor.read_u32_with(control)?, offset)?;
        let fields = cursor.read_u32_with(control)?;
        let methods = cursor.read_u32_with(control)?;
        let parameters = cursor.read_u32_with(control)?;
        let total = u64::from(fields) + u64::from(methods) + u64::from(parameters);
        if total > (cursor.remaining_byte().0 / 8) as u64 {
            return Err(malformed(InputProblem::Truncated, cursor.position_byte()));
        }
        Ok(AnnotationDirectory {
            class,
            fields: records(self, &mut cursor, fields, ByteLength(8), control)?,
            methods: records(self, &mut cursor, methods, ByteLength(8), control)?,
            parameters: records(self, &mut cursor, parameters, ByteLength(8), control)?,
        })
    }

    pub(crate) fn annotation_parameters(
        &self,
        offset: ByteOffset,
        count: usize,
        control: &mut impl InputControl,
    ) -> Result<Cursor<'a>, InputError> {
        let cursor = self.annotation_offsets(0x1002, offset, control)?;
        if cursor.remaining_byte().0 / 4 != count {
            return Err(malformed(InputProblem::InvalidEncodedValue, offset));
        }
        Ok(cursor)
    }

    pub(crate) fn next_annotation(
        &self,
        offsets: &mut Cursor<'_>,
        last: &mut Option<TypeId>,
        budget: &mut impl InputControl,
    ) -> Result<(ByteOffset, u8, TypeId, EncodedValues<'a>), MethodError> {
        let at = offsets.position_byte();
        let offset = file_offset(offsets.read_u32_with(budget)?, at)?;
        let (visibility, type_id, values) = EncodedValues::annotation(self, offset, budget)?;
        if last.is_some_and(|last| last >= type_id) {
            return Err(malformed(InputProblem::InvalidEncodedValue, at).into());
        }
        *last = Some(type_id);
        Ok((offset, visibility, type_id, values))
    }

    pub(crate) fn annotation_offsets(
        &self,
        kind: u16,
        offset: ByteOffset,
        control: &mut impl InputControl,
    ) -> Result<Cursor<'a>, InputError> {
        let mut cursor = self.item(kind, offset, control)?;
        let count = cursor.read_u32_with(control)?;
        records(self, &mut cursor, count, ByteLength(4), control)
    }
}

struct Group<'a> {
    cursor: Cursor<'a>,
    index: u32,
    remaining: u32,
    next: Option<u32>,
    first: bool,
    method: bool,
}

/// Two sorted class-data groups replay alongside one sorted directory table.
/// Code offsets are consumed as integers; code items are never opened here.
pub(crate) struct AnnotationMembers<'a> {
    groups: [Group<'a>; 2],
}

impl<'a> AnnotationMembers<'a> {
    pub fn open(
        index: &ProgramIndex<'a>,
        class: &ClassView<'_, 'a>,
        method: bool,
        control: &mut impl InputControl,
    ) -> Result<Self, InputError> {
        let mut cursor = match class.class_data_offset {
            Some(offset) => index.item(0x2000, offset, control)?,
            None => Cursor::bounded(index.bytes, ByteOffset(0), ByteOffset(0))?,
        };
        let mut counts = [0; 4];
        if class.class_data_offset.is_some() {
            for count in &mut counts {
                *count = cursor.read_uleb32_with(control)?;
            }
        }
        let minimum = u64::from(counts[0]) * 2
            + u64::from(counts[1]) * 2
            + u64::from(counts[2]) * 3
            + u64::from(counts[3]) * 3;
        if minimum > cursor.remaining_byte().0 as u64 {
            return Err(malformed(InputProblem::Truncated, cursor.position_byte()));
        }
        let mut ranges = [ByteRange {
            start: cursor.position_byte(),
            end: cursor.position_byte(),
        }; 4];
        for (group, count) in counts.iter().enumerate() {
            let start = cursor.position_byte();
            for _ in 0..*count {
                control.checkpoint()?;
                cursor.read_uleb32_with(control)?;
                cursor.read_uleb32_with(control)?;
                if group >= 2 {
                    cursor.read_uleb32_with(control)?;
                }
            }
            ranges[group] = ByteRange {
                start,
                end: cursor.position_byte(),
            };
        }
        let start = if method { 2 } else { 0 };
        let make = |n: usize| -> Result<Group<'a>, InputError> {
            Ok(Group {
                cursor: Cursor::bounded(index.bytes, ranges[n].start, ranges[n].end)?,
                index: 0,
                remaining: counts[n],
                next: None,
                first: true,
                method,
            })
        };
        Ok(Self {
            groups: [make(start)?, make(start + 1)?],
        })
    }

    pub fn check(
        &mut self,
        target: u32,
        control: &mut impl InputControl,
    ) -> Result<(), InputError> {
        let mut matches = 0;
        for group in &mut self.groups {
            loop {
                if let Some(next) = group.next {
                    if next >= target {
                        matches += u8::from(next == target);
                        break;
                    }
                    group.next = None;
                }
                if group.remaining == 0 {
                    break;
                }
                control.checkpoint()?;
                let at = group.cursor.position_byte();
                let delta = group.cursor.read_uleb32_with(control)?;
                if !group.first && delta == 0 {
                    return Err(malformed(InputProblem::InvalidClassData, at));
                }
                group.first = false;
                group.index = group
                    .index
                    .checked_add(delta)
                    .ok_or_else(|| malformed(InputProblem::RangeOverflow, at))?;
                group.cursor.read_uleb32_with(control)?;
                if group.method {
                    group.cursor.read_uleb32_with(control)?;
                }
                group.remaining -= 1;
                group.next = Some(group.index);
            }
        }
        if matches != 1 {
            return Err(malformed(
                InputProblem::InvalidReference,
                self.groups[0].cursor.position_byte(),
            ));
        }
        Ok(())
    }
}

pub(crate) enum AnnotationOwner {
    Field(FieldId),
    Method(MethodId),
}

pub(crate) fn association(
    index: &ProgramIndex<'_>,
    class: &ClassView<'_, '_>,
    members: &mut AnnotationMembers<'_>,
    records: &mut Cursor<'_>,
    last: &mut Option<u32>,
    method: bool,
    control: &mut impl InputControl,
) -> Result<(AnnotationOwner, ByteOffset, usize), InputError> {
    let at = records.position_byte();
    let id = records.read_u32_with(control)?;
    let offset = file_offset(records.read_u32_with(control)?, at)?;
    if last.is_some_and(|last| last >= id) {
        return Err(malformed(InputProblem::InvalidReference, at));
    }
    *last = Some(id);
    members.check(id, control)?;
    let parameters = if method {
        index
            .method_declaration(MethodId(id), class.type_idx, control)?
            .prototype
            .parameters
            .len()
            / 2
    } else {
        index.validate_field(FieldId(id), class.type_idx, control)?;
        0
    };
    let owner = if method {
        AnnotationOwner::Method(MethodId(id))
    } else {
        AnnotationOwner::Field(FieldId(id))
    };
    Ok((owner, offset, parameters))
}
