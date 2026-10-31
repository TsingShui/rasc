use super::code::read_code;
use super::header::{SUPPORTED_MAP_KINDS, map_kind};
use super::strings::{DexString, TypeShape};
use super::{
    Cursor, DexHeader, InputError, InputProblem, checked_range, count_usize, file_offset, malformed,
};
use crate::pipeline::control::InputControl;
use crate::{
    ByteLength, ByteOffset, ClassDefId, ControlError, FailureKind, FieldId, MethodId, ProtoId,
    RegisterWordCount, RunContext, StringId, TypeId, Utf16Offset,
};

#[derive(Clone, Copy)]
struct Section {
    kind: u16,
    start_byte: ByteOffset,
    end_byte: ByteOffset,
}

/// A single-image index borrowing original bytes. Variable metadata is resolved
/// on demand; it owns neither a decoded string pool nor method result buffers.
pub struct ProgramIndex<'a> {
    pub(super) bytes: &'a [u8],
    pub(super) header: DexHeader,
    sections: [Option<Section>; SUPPORTED_MAP_KINDS],
}

impl<'a> ProgramIndex<'a> {
    pub fn open(bytes: &'a [u8], control: &mut RunContext<'_>) -> Result<Self, InputError> {
        Self::open_with(bytes, control)
    }

    pub(crate) fn open_with(
        bytes: &'a [u8],
        control: &mut impl InputControl,
    ) -> Result<Self, InputError> {
        let header = super::header::read_header_with(bytes, control)?;
        let mut cursor = Cursor::bounded(
            bytes,
            header.map_byte_range.start,
            header.map_byte_range.end,
        )?;
        let count = count_usize(cursor.read_u32_with(control)?, header.map_byte_range.start)?;
        // The 035 map has 18 distinct supported kinds, checked by read_header.
        let mut sections = [Section {
            kind: 0,
            start_byte: ByteOffset(0),
            end_byte: ByteOffset(0),
        }; SUPPORTED_MAP_KINDS];
        if count > sections.len() {
            return Err(malformed(
                InputProblem::InvalidMap,
                header.map_byte_range.start,
            ));
        }
        for index in 0..count {
            let kind = cursor.read_u16_with(control)?;
            cursor.read_u16_with(control)?;
            cursor.read_u32_with(control)?;
            let start_byte = file_offset(
                cursor.read_u32_with(control)?,
                ByteOffset(cursor.position_byte().0 - 4),
            )?;
            if index > 0 {
                sections[index - 1].end_byte = start_byte;
            }
            sections[index] = Section {
                kind,
                start_byte,
                end_byte: header.data_byte_range.end,
            };
        }
        let mut by_kind = [None; SUPPORTED_MAP_KINDS];
        for section in &sections[..count] {
            let (bit, _) = map_kind(section.kind)
                .ok_or_else(|| malformed(InputProblem::InvalidMap, section.start_byte))?;
            by_kind[bit.trailing_zeros() as usize] = Some(*section);
        }
        Ok(Self {
            bytes,
            header,
            sections: by_kind,
        })
    }

    pub fn string(
        &self,
        string_idx: StringId,
        control: &mut RunContext<'_>,
    ) -> Result<DexString<'a>, InputError> {
        self.string_with(string_idx, control)
    }

    pub(crate) fn string_with(
        &self,
        string_idx: StringId,
        control: &mut impl InputControl,
    ) -> Result<DexString<'a>, InputError> {
        let mut record = self.table(TableKey::String(string_idx), control)?;
        let offset = file_offset(
            record.read_u32_with(control)?,
            ByteOffset(record.position_byte().0 - 4),
        )?;
        DexString::read(self.bytes, self.item(0x2002, offset, control)?, control)
    }

    pub fn class(
        &self,
        class_def_idx: ClassDefId,
        control: &mut RunContext<'_>,
    ) -> Result<ClassView<'_, 'a>, InputError> {
        self.class_with(class_def_idx, control)
    }

    pub(crate) fn class_with(
        &self,
        class_def_idx: ClassDefId,
        control: &mut impl InputControl,
    ) -> Result<ClassView<'_, 'a>, InputError> {
        let mut record = self.table(TableKey::Class(class_def_idx), control)?;
        let type_idx = TypeId(record.read_u32_with(control)?);
        let access_flags = record.read_u32_with(control)?;
        record.skip(4, ByteLength(4), control)?;
        let class_data_offset = file_offset(
            record.read_u32_with(control)?,
            ByteOffset(record.position_byte().0 - 4),
        )?;
        record.read_u32_with(control)?;
        let (descriptor, shape) = self.type_descriptor(type_idx, control)?;
        if !matches!(shape, TypeShape::Class) {
            return Err(malformed(
                InputProblem::InvalidDescriptor,
                ByteOffset(record.position_byte().0 - 32),
            ));
        }
        Ok(ClassView {
            index: self,
            class_def_idx,
            type_idx,
            access_flags,
            descriptor,
            class_data_offset: (class_data_offset != ByteOffset(0)).then_some(class_data_offset),
        })
    }

    /// Visits original class_def identities independently. Malformed class
    /// metadata is delivered locally; cancellation/resources stop the request.
    pub fn visit_classes(
        &self,
        control: &mut RunContext<'_>,
        mut visit: impl FnMut(
            ClassDefId,
            Result<ClassView<'_, 'a>, InputError>,
            &mut RunContext<'_>,
        ) -> Result<(), InputError>,
    ) -> Result<(), InputError> {
        for index in 0..self.header.class_defs.count {
            control.checkpoint()?;
            let class = local(self.class(ClassDefId(index), control))?;
            visit(ClassDefId(index), class, control)?;
        }
        Ok(())
    }

    pub(super) fn item(
        &self,
        kind: u16,
        offset_byte: ByteOffset,
        control: &mut impl InputControl,
    ) -> Result<Cursor<'a>, InputError> {
        control.charge_work(1)?;
        let (bit, alignment) =
            map_kind(kind).ok_or_else(|| malformed(InputProblem::InvalidReference, offset_byte))?;
        let section = self.sections[bit.trailing_zeros() as usize]
            .ok_or_else(|| malformed(InputProblem::InvalidReference, offset_byte))?;
        if offset_byte < section.start_byte || offset_byte >= section.end_byte {
            return Err(malformed(InputProblem::InvalidReference, offset_byte));
        }
        if !offset_byte.0.is_multiple_of(alignment) {
            return Err(malformed(InputProblem::InvalidAlignment, offset_byte));
        }
        Cursor::bounded(self.bytes, offset_byte, section.end_byte)
    }

    fn table(
        &self,
        key: TableKey,
        control: &mut impl InputControl,
    ) -> Result<Cursor<'a>, InputError> {
        let (table, index, stride) = match key {
            TableKey::String(id) => (self.header.string_ids, id.0, ByteLength(4)),
            TableKey::Type(id) => (self.header.type_ids, id.0, ByteLength(4)),
            TableKey::Proto(id) => (self.header.proto_ids, id.0, ByteLength(12)),
            TableKey::Field(id) => (self.header.field_ids, id.0, ByteLength(8)),
            TableKey::Method(id) => (self.header.method_ids, id.0, ByteLength(8)),
            TableKey::Class(id) => (self.header.class_defs, id.0, ByteLength(32)),
        };
        if index >= table.count {
            return Err(malformed(InputProblem::InvalidReference, table.offset_byte));
        }
        let table_end = table
            .offset_byte
            .checked_advance(table.size_byte)
            .ok_or_else(|| malformed(InputProblem::RangeOverflow, table.offset_byte))?;
        let prefix = checked_range(
            ByteLength(table_end.0),
            table.offset_byte,
            count_usize(index, table.offset_byte)?,
            stride,
        )?;
        let range = checked_range(ByteLength(table_end.0), prefix.end, 1, stride)?;
        control.charge_metadata(1)?;
        Cursor::bounded(self.bytes, range.start, range.end)
    }

    pub(super) fn type_descriptor(
        &self,
        type_idx: TypeId,
        control: &mut impl InputControl,
    ) -> Result<(DexString<'a>, TypeShape), InputError> {
        let mut record = self.table(TableKey::Type(type_idx), control)?;
        let string_idx = StringId(record.read_u32_with(control)?);
        let descriptor = self.string_with(string_idx, control)?;
        let shape = descriptor.shape(control)?;
        Ok((descriptor, shape))
    }

    pub(crate) fn prototype(
        &self,
        proto_idx: ProtoId,
        control: &mut impl InputControl,
    ) -> Result<Prototype<'_, 'a>, InputError> {
        let mut record = self.table(TableKey::Proto(proto_idx), control)?;
        let shorty = self.string_with(StringId(record.read_u32_with(control)?), control)?;
        let (return_type, return_shape) =
            self.type_descriptor(TypeId(record.read_u32_with(control)?), control)?;
        let parameters_offset = file_offset(
            record.read_u32_with(control)?,
            ByteOffset(record.position_byte().0 - 4),
        )?;
        let parameters = if parameters_offset == ByteOffset(0) {
            &self.bytes[0..0]
        } else {
            let mut parameters = self.item(0x1001, parameters_offset, control)?;
            let count = parameters.read_u32_with(control)?;
            let range = parameters.skip(
                count_usize(count, parameters_offset)?,
                ByteLength(2),
                control,
            )?;
            &self.bytes[range.as_range()]
        };
        if u64::from(shorty.utf16_len().0) != (parameters.len() / 2) as u64 + 1 {
            return Err(malformed(InputProblem::InvalidPrototype, parameters_offset));
        }
        let mut shorty_units = shorty.reader()?;
        if shorty_units.next(control)? != Some(u16::from(return_shape.shorty())) {
            return Err(malformed(InputProblem::InvalidPrototype, parameters_offset));
        }
        let mut parameter_words = RegisterWordCount(0);
        for word in parameters.as_chunks::<2>().0 {
            let type_idx = TypeId(u32::from(u16::from_le_bytes(*word)));
            let (_, shape) = self.type_descriptor(type_idx, control)?;
            if shape.words() == 0 || shorty_units.next(control)? != Some(u16::from(shape.shorty()))
            {
                return Err(malformed(InputProblem::InvalidPrototype, parameters_offset));
            }
            parameter_words.0 += u64::from(shape.words());
        }
        Ok(Prototype {
            index: self,
            shorty,
            return_type,
            parameters,
            parameter_words,
        })
    }

    pub(super) fn method_declaration(
        &self,
        method_idx: MethodId,
        owner: TypeId,
        control: &mut impl InputControl,
    ) -> Result<MethodDeclaration<'_, 'a>, InputError> {
        let mut record = self.table(TableKey::Method(method_idx), control)?;
        if TypeId(u32::from(record.read_u16_with(control)?)) != owner {
            return Err(malformed(
                InputProblem::InvalidReference,
                ByteOffset(record.position_byte().0 - 2),
            ));
        }
        let proto_idx = ProtoId(u32::from(record.read_u16_with(control)?));
        let name = self.string_with(StringId(record.read_u32_with(control)?), control)?;
        name.validate_name(control)?;
        let prototype = self.prototype(proto_idx, control)?;
        Ok(MethodDeclaration { name, prototype })
    }

    pub(super) fn validate_field(
        &self,
        field_idx: FieldId,
        owner: TypeId,
        control: &mut impl InputControl,
    ) -> Result<FieldDeclaration<'a>, InputError> {
        let mut record = self.table(TableKey::Field(field_idx), control)?;
        if TypeId(u32::from(record.read_u16_with(control)?)) != owner {
            return Err(malformed(
                InputProblem::InvalidReference,
                ByteOffset(record.position_byte().0 - 2),
            ));
        }
        let (field_type, shape) =
            self.type_descriptor(TypeId(u32::from(record.read_u16_with(control)?)), control)?;
        let name = self.string_with(StringId(record.read_u32_with(control)?), control)?;
        name.validate_name(control)?;
        if shape.words() == 0 {
            return Err(malformed(
                InputProblem::InvalidDescriptor,
                ByteOffset(record.position_byte().0 - 6),
            ));
        }
        Ok(FieldDeclaration { name, field_type })
    }
}

impl<'a> ProgramIndex<'a> {
    pub(crate) fn type_name(
        &self,
        id: TypeId,
        control: &mut impl InputControl,
    ) -> Result<DexString<'a>, InputError> {
        Ok(self.type_descriptor(id, control)?.0)
    }

    pub(crate) fn class_context(
        &self,
        id: ClassDefId,
        control: &mut impl InputControl,
    ) -> Result<ClassContext<'a>, InputError> {
        let mut record = self.table(TableKey::Class(id), control)?;
        record.skip(2, ByteLength(4), control)?;
        let super_idx = record.read_u32_with(control)?;
        let interfaces_raw = record.read_u32_with(control)?;
        record.read_u32_with(control)?;
        let annotations_raw = record.read_u32_with(control)?;
        record.read_u32_with(control)?;
        let values_raw = record.read_u32_with(control)?;
        let superclass = local(self.superclass(super_idx, control))?;
        let interfaces = local(self.interfaces(interfaces_raw, control))?;
        let values_offset = local(self.values_offset(values_raw, control))?;
        Ok(ClassContext {
            superclass,
            interfaces,
            values_offset,
            annotations_offset: file_offset(annotations_raw, self.header.class_defs.offset_byte)?,
        })
    }

    fn superclass(
        &self,
        index: u32,
        control: &mut impl InputControl,
    ) -> Result<Option<DexString<'a>>, InputError> {
        if index == u32::MAX {
            return Ok(None);
        }
        let (name, shape) = self.type_descriptor(TypeId(index), control)?;
        if !matches!(shape, TypeShape::Class) {
            return Err(malformed(
                InputProblem::InvalidDescriptor,
                self.header.type_ids.offset_byte,
            ));
        }
        Ok(Some(name))
    }

    fn interfaces(
        &self,
        raw: u32,
        control: &mut impl InputControl,
    ) -> Result<&'a [u8], InputError> {
        let offset = file_offset(raw, self.header.class_defs.offset_byte)?;
        if offset == ByteOffset(0) {
            return Ok(&self.bytes[0..0]);
        }
        let mut list = self.item(0x1001, offset, control)?;
        let count = list.read_u32_with(control)?;
        let range = list.skip(count_usize(count, offset)?, ByteLength(2), control)?;
        control.charge_metadata(u64::from(count))?;
        let bytes = &self.bytes[range.as_range()];
        for word in bytes.as_chunks::<2>().0 {
            let (_, shape) =
                self.type_descriptor(TypeId(u32::from(u16::from_le_bytes(*word))), control)?;
            if !matches!(shape, TypeShape::Class) {
                return Err(malformed(InputProblem::InvalidDescriptor, offset));
            }
        }
        Ok(bytes)
    }

    fn values_offset(
        &self,
        raw: u32,
        control: &mut impl InputControl,
    ) -> Result<Option<ByteOffset>, InputError> {
        let offset = file_offset(raw, self.header.class_defs.offset_byte)?;
        if offset == ByteOffset(0) {
            return Ok(None);
        }
        self.item(0x2005, offset, control)?;
        Ok(Some(offset))
    }

    pub(crate) fn method_reference(
        &self,
        id: MethodId,
        control: &mut impl InputControl,
    ) -> Result<MethodReference<'_, 'a>, InputError> {
        let mut record = self.table(TableKey::Method(id), control)?;
        let owner_idx = TypeId(u32::from(record.read_u16_with(control)?));
        let (owner, shape) = self.type_descriptor(owner_idx, control)?;
        if !matches!(shape, TypeShape::Class | TypeShape::Array) {
            return Err(malformed(
                InputProblem::InvalidDescriptor,
                record.position_byte(),
            ));
        }
        Ok(MethodReference {
            owner,
            declaration: self.method_declaration(id, owner_idx, control)?,
        })
    }

    pub(crate) fn field_reference(
        &self,
        id: FieldId,
        control: &mut impl InputControl,
    ) -> Result<FieldReference<'a>, InputError> {
        let mut record = self.table(TableKey::Field(id), control)?;
        let owner_idx = TypeId(u32::from(record.read_u16_with(control)?));
        let (owner, shape) = self.type_descriptor(owner_idx, control)?;
        if !matches!(shape, TypeShape::Class) {
            return Err(malformed(
                InputProblem::InvalidDescriptor,
                record.position_byte(),
            ));
        }
        Ok(FieldReference {
            owner,
            declaration: self.validate_field(id, owner_idx, control)?,
        })
    }
}

pub(crate) struct ClassContext<'a> {
    pub superclass: Result<Option<DexString<'a>>, InputError>,
    pub interfaces: Result<&'a [u8], InputError>,
    pub values_offset: Result<Option<ByteOffset>, InputError>,
    pub annotations_offset: ByteOffset,
}

pub(crate) struct MethodReference<'i, 'a> {
    pub owner: DexString<'a>,
    pub declaration: MethodDeclaration<'i, 'a>,
}

pub(crate) struct FieldReference<'a> {
    pub owner: DexString<'a>,
    pub declaration: FieldDeclaration<'a>,
}

pub struct ClassView<'i, 'a> {
    index: &'i ProgramIndex<'a>,
    pub class_def_idx: ClassDefId,
    pub type_idx: TypeId,
    pub access_flags: u32,
    pub descriptor: DexString<'a>,
    pub(super) class_data_offset: Option<ByteOffset>,
}

pub struct MethodDeclaration<'i, 'a> {
    pub name: DexString<'a>,
    pub prototype: Prototype<'i, 'a>,
}

pub struct Prototype<'i, 'a> {
    index: &'i ProgramIndex<'a>,
    pub shorty: DexString<'a>,
    pub return_type: DexString<'a>,
    pub(super) parameters: &'a [u8],
    pub parameter_words: RegisterWordCount,
}

impl<'a> Prototype<'_, 'a> {
    pub(crate) fn visit_parameter_types<C: InputControl, E: From<InputError>>(
        &self,
        control: &mut C,
        mut visit: impl FnMut(DexString<'a>, &mut C) -> Result<(), E>,
    ) -> Result<(), E> {
        control.checkpoint().map_err(InputError::from)?;
        for word in self.parameters.as_chunks::<2>().0 {
            let (name, _) = self
                .index
                .type_descriptor(TypeId(u32::from(u16::from_le_bytes(*word))), control)?;
            visit(name, control)?;
        }
        Ok(())
    }

    /// Compares a complete DEX descriptor, including the return type.
    pub fn matches(
        &self,
        descriptor: &[u16],
        control: &mut RunContext<'_>,
    ) -> Result<bool, InputError> {
        control.checkpoint()?;
        if descriptor.first() != Some(&u16::from(b'(')) {
            return Ok(false);
        }
        let mut position = Utf16Offset(1);
        for word in self.parameters.as_chunks::<2>().0 {
            let (name, _) = self
                .index
                .type_descriptor(TypeId(u32::from(u16::from_le_bytes(*word))), control)?;
            let Some(next) = name.matches_prefix(descriptor, position, control)? else {
                return Ok(false);
            };
            position = next;
        }
        if descriptor.get(position.0) != Some(&u16::from(b')')) {
            return Ok(false);
        }
        Ok(self
            .return_type
            .matches_prefix(descriptor, Utf16Offset(position.0 + 1), control)?
            == Some(Utf16Offset(descriptor.len())))
    }
}

pub(crate) struct FieldDeclaration<'a> {
    pub name: DexString<'a>,
    pub field_type: DexString<'a>,
}

pub(crate) enum FieldStorage {
    Static { ordinal: u32 },
    Instance,
}

pub(crate) struct FieldEntry<'a> {
    pub field_idx: FieldId,
    pub access_flags: u32,
    pub storage: FieldStorage,
    pub declaration: Result<FieldDeclaration<'a>, InputError>,
}

pub(crate) enum MemberSelection {
    AllMethods,
    Method(MethodId),
    Fields,
}

pub(crate) enum MemberEntry<'i, 'a> {
    Field(FieldEntry<'a>),
    Method(MethodEntry<'i, 'a>),
}

pub struct MethodEntry<'i, 'a> {
    pub method_idx: MethodId,
    pub access_flags: u32,
    pub declaration: Result<MethodDeclaration<'i, 'a>, InputError>,
    pub body: Result<super::MethodBody<'a>, InputError>,
}

impl<'i, 'a> ClassView<'i, 'a> {
    /// Per-member failures retain their original IDs. A class-data boundary
    /// error returns after the delivered prefix; callers retain that prefix.
    pub fn visit_methods(
        &self,
        control: &mut RunContext<'_>,
        mut visit: impl FnMut(MethodEntry<'i, 'a>, &mut RunContext<'_>) -> Result<(), InputError>,
    ) -> Result<(), InputError> {
        self.visit_members_with(
            control,
            MemberSelection::AllMethods,
            |entry, control| match entry {
                MemberEntry::Method(method) => visit(method, control),
                MemberEntry::Field(_) => Ok(()),
            },
        )
    }

    pub(crate) fn visit_members_with<C: InputControl, E: From<InputError> + From<ControlError>>(
        &self,
        control: &mut C,
        selection: MemberSelection,
        mut visit: impl FnMut(MemberEntry<'i, 'a>, &mut C) -> Result<(), E>,
    ) -> Result<(), E> {
        let Some(offset) = self.class_data_offset else {
            control.checkpoint()?;
            return Ok(());
        };
        let mut cursor = self.index.item(0x2000, offset, control)?;
        let mut counts = [0u32; 4];
        for count in &mut counts {
            *count = cursor.read_uleb32_with(control)?;
        }
        let minimum = u64::from(counts[0]) * 2
            + u64::from(counts[1]) * 2
            + u64::from(counts[2]) * 3
            + u64::from(counts[3]) * 3;
        if minimum > cursor.remaining_byte().0 as u64 {
            return Err(malformed(InputProblem::Truncated, cursor.position_byte()).into());
        }
        let mut first_error = None;
        for (groups, kind) in [
            ([counts[0], counts[1]], MemberKind::Field),
            ([counts[2], counts[3]], MemberKind::Method),
        ] {
            if matches!(selection, MemberSelection::Fields) && matches!(kind, MemberKind::Method) {
                break;
            }
            if groups == [0, 0] {
                continue;
            }
            let table_count = match kind {
                MemberKind::Field => self.index.header.field_ids.count,
                MemberKind::Method => self.index.header.method_ids.count,
            };
            let mut prior = PriorState::Comparable(PriorGroup {
                cursor: self.index.item(0x2000, cursor.position_byte(), control)?,
                remaining: groups[0],
                next: None,
                index: 0,
                kind,
            });
            for (group, count) in groups.into_iter().enumerate() {
                let mut previous = Some(0u32);
                for item in 0..count {
                    control.charge_metadata(1)?;
                    let record = cursor.position_byte();
                    let delta = cursor.read_uleb32_with(control)?;
                    let access_flags = cursor.read_uleb32_with(control)?;
                    let member = match kind {
                        MemberKind::Field => EncodedMember::Field,
                        MemberKind::Method => {
                            let offset = file_offset(cursor.read_uleb32_with(control)?, record)?;
                            EncodedMember::Method {
                                code_offset: (offset != ByteOffset(0)).then_some(offset),
                            }
                        }
                    };
                    let Some(index) = previous.and_then(|value| value.checked_add(delta)) else {
                        first_error.get_or_insert(malformed(InputProblem::RangeOverflow, record));
                        previous = None;
                        if group == 0 {
                            prior = PriorState::Invalid;
                        }
                        continue;
                    };
                    previous = Some(index);
                    let invalid_index = index >= table_count || (item != 0 && delta == 0);
                    let duplicate = if !invalid_index && group == 1 {
                        match &mut prior {
                            PriorState::Comparable(prior) => prior.contains(index, control)?,
                            PriorState::Invalid => false,
                        }
                    } else {
                        false
                    };
                    if invalid_index || duplicate {
                        first_error
                            .get_or_insert(malformed(InputProblem::InvalidClassData, record));
                        if group == 0 {
                            prior = PriorState::Invalid;
                        }
                        continue;
                    }
                    match member {
                        EncodedMember::Field => {
                            let declaration = local(self.index.validate_field(
                                FieldId(index),
                                self.type_idx,
                                control,
                            ))?;
                            if let Err(error) = declaration {
                                first_error.get_or_insert(error);
                            }
                            visit(
                                MemberEntry::Field(FieldEntry {
                                    field_idx: FieldId(index),
                                    access_flags,
                                    storage: if group == 0 {
                                        FieldStorage::Static { ordinal: item }
                                    } else {
                                        FieldStorage::Instance
                                    },
                                    declaration,
                                }),
                                control,
                            )?;
                        }
                        EncodedMember::Method { code_offset } => {
                            if let MemberSelection::Method(selected) = selection
                                && selected != MethodId(index)
                            {
                                continue;
                            }
                            let declaration = local(self.index.method_declaration(
                                MethodId(index),
                                self.type_idx,
                                control,
                            ))?;
                            let words = declaration.as_ref().ok().map(|declaration| {
                                RegisterWordCount(
                                    declaration.prototype.parameter_words.0
                                        + u64::from(access_flags & 8 == 0),
                                )
                            });
                            let body = local(read_code(
                                self.index,
                                code_offset,
                                access_flags,
                                words,
                                control,
                            ))?;
                            visit(
                                MemberEntry::Method(MethodEntry {
                                    method_idx: MethodId(index),
                                    access_flags,
                                    declaration,
                                    body,
                                }),
                                control,
                            )?;
                        }
                    }
                }
            }
        }
        if let Some(error) = first_error {
            return Err(error.into());
        }
        Ok(())
    }
}

enum TableKey {
    String(StringId),
    Type(TypeId),
    Proto(ProtoId),
    Field(FieldId),
    Method(MethodId),
    Class(ClassDefId),
}

#[derive(Clone, Copy)]
enum MemberKind {
    Field,
    Method,
}

enum EncodedMember {
    Field,
    Method { code_offset: Option<ByteOffset> },
}

enum PriorState<'a> {
    Comparable(PriorGroup<'a>),
    Invalid,
}

struct PriorGroup<'a> {
    cursor: Cursor<'a>,
    remaining: u32,
    next: Option<u32>,
    index: u32,
    kind: MemberKind,
}

impl PriorGroup<'_> {
    // Sorted group replay detects cross-group duplicates in linear time.
    fn contains(
        &mut self,
        target: u32,
        control: &mut impl InputControl,
    ) -> Result<bool, InputError> {
        loop {
            if let Some(next) = self.next {
                if next >= target {
                    return Ok(next == target);
                }
                self.next = None;
            }
            if self.remaining == 0 {
                return Ok(false);
            }
            self.index = self
                .index
                .checked_add(self.cursor.read_uleb32_with(control)?)
                .ok_or_else(|| {
                    malformed(InputProblem::RangeOverflow, self.cursor.position_byte())
                })?;
            self.cursor.read_uleb32_with(control)?;
            if matches!(self.kind, MemberKind::Method) {
                self.cursor.read_uleb32_with(control)?;
            }
            self.remaining -= 1;
            self.next = Some(self.index);
        }
    }
}

pub(super) fn local<T>(result: Result<T, InputError>) -> Result<Result<T, InputError>, InputError> {
    match result {
        Err(error)
            if !matches!(
                error.kind(),
                FailureKind::Malformed | FailureKind::Unsupported
            ) =>
        {
            Err(error)
        }
        result => Ok(result),
    }
}
