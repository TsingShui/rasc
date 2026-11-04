use super::code::read_code;
use super::header::{SUPPORTED_MAP_KINDS, map_kind};
use super::strings::{DexString, TypeShape};
use super::{
    Cursor, DexContainer, DexHeader, InputError, InputProblem, checked_range, count_usize,
    file_offset, malformed,
};
use crate::pipeline::control::InputControl;
use crate::{
    ByteLength, ByteOffset, ClassDefId, ClassKey, ControlError, FailureKind, FieldId, MethodId,
    ProtoId, RegisterWordCount, RunContext, StringId, TypeId, Utf16Offset,
};

#[derive(Clone, Copy)]
struct Section {
    kind: u16,
    count: u32,
    start_byte: ByteOffset,
    end_byte: ByteOffset,
}

/// A single-image index borrowing original bytes. Variable metadata is resolved
/// on demand; it owns neither a decoded string pool nor method result buffers.
pub struct ProgramIndex<'a> {
    pub(crate) bytes: &'a [u8],
    dex: crate::DexId,
    class_facts: std::sync::Mutex<
        std::collections::HashMap<
            ClassDefId,
            Result<std::sync::Arc<crate::p0::ClassFacts>, InputError>,
        >,
    >,
    facts_build: std::sync::Mutex<()>,
    ownership: std::sync::OnceLock<Result<crate::input::ownership::Ownership, InputError>>,
    cache_build: std::sync::Mutex<()>,
    class_locations: std::sync::OnceLock<super::class_locations::ClassLocations>,
    pub(crate) header: DexHeader,
    container: DexContainer<'a>,
    sections: [Option<Section>; SUPPORTED_MAP_KINDS],
}

impl<'a> ProgramIndex<'a> {
    /// Original physical bytes this index borrows.
    pub(crate) fn bytes(&self) -> &'a [u8] {
        self.bytes
    }

    /// Binds the logical DEX identity used for original class keys.
    pub(crate) fn with_dex(mut self, dex: crate::DexId) -> Self {
        self.dex = dex;
        self
    }

    /// The current class's P0 facts, built once per logical class and shared by
    /// later declaration queries. Deterministic input failures are cached;
    /// cancellation and resource failures are not.
    pub(crate) fn class_facts_cached(
        &self,
        class: ClassDefId,
        control: &mut RunContext<'_>,
    ) -> Result<std::sync::Arc<crate::p0::ClassFacts>, crate::RequestError> {
        control.checkpoint()?;
        {
            let cache = self.class_facts.lock().map_err(|_| {
                crate::RequestError::InternalInvariant("class-fact cache was poisoned")
            })?;
            if let Some(entry) = cache.get(&class) {
                return match entry {
                    Ok(facts) => Ok(std::sync::Arc::clone(facts)),
                    Err(error) => Err((*error).into()),
                };
            }
        }
        let _build = self.facts_build.lock().map_err(|_| {
            crate::RequestError::InternalInvariant("class-fact build lock was poisoned")
        })?;
        {
            let cache = self.class_facts.lock().map_err(|_| {
                crate::RequestError::InternalInvariant("class-fact cache was poisoned")
            })?;
            if let Some(entry) = cache.get(&class) {
                return match entry {
                    Ok(facts) => Ok(std::sync::Arc::clone(facts)),
                    Err(error) => Err((*error).into()),
                };
            }
        }
        let key = ClassKey {
            dex: self.dex,
            class_def: class,
        };
        let result = crate::p0::collect(self, key, control);
        let mut cache = self
            .class_facts
            .lock()
            .map_err(|_| crate::RequestError::InternalInvariant("class-fact cache was poisoned"))?;
        match result {
            Ok(facts) => {
                let facts = std::sync::Arc::new(facts);
                cache.insert(class, Ok(std::sync::Arc::clone(&facts)));
                Ok(facts)
            }
            Err(crate::RequestError::Input(error)) => {
                cache.insert(class, Err(error));
                Err(error.into())
            }
            Err(error) => Err(error),
        }
    }

    /// Ownership metadata is immutable for this image; build it once and reuse.
    pub(crate) fn ownership_cached(
        &self,
        control: &mut impl InputControl,
    ) -> Result<&crate::input::ownership::Ownership, crate::RequestError> {
        control.checkpoint()?;
        if self.ownership.get().is_none() {
            let _lock = self.cache_build.lock().map_err(|_| {
                crate::RequestError::InternalInvariant("input cache construction poisoned")
            })?;
            if let Some(result) = self.ownership.get() {
                return result.as_ref().map_err(|error| (*error).into());
            }
            let result = match crate::input::ownership::collect(self, control) {
                Ok(ownership) => Ok(ownership),
                Err(crate::RequestError::Input(error))
                    if matches!(
                        error.kind(),
                        FailureKind::Malformed | FailureKind::Unsupported
                    ) =>
                {
                    Err(error)
                }
                Err(error) => return Err(error),
            };
            let _ = self.ownership.set(result);
        }
        match self.ownership.get() {
            Some(Ok(ownership)) => Ok(ownership),
            Some(Err(error)) => Err((*error).into()),
            None => Err(crate::RequestError::InternalInvariant(
                "ownership cache was not initialized",
            )),
        }
    }

    pub(super) fn class_type_id(
        &self,
        class: ClassDefId,
        control: &mut impl InputControl,
    ) -> Result<TypeId, InputError> {
        let mut record = self.table(TableKey::Class(class), control)?;
        Ok(TypeId(record.read_u32_with(control)?))
    }

    fn class_location(
        &self,
        ty: TypeId,
        control: &mut impl InputControl,
    ) -> Result<Option<ClassDefId>, InputError> {
        if self.class_locations.get().is_none() {
            let _lock = self
                .cache_build
                .lock()
                .map_err(|_| InputError::InternalInvariant("input cache construction poisoned"))?;
            if let Some(locations) = self.class_locations.get() {
                return locations.get(ty, self, control);
            }
            let locations = super::class_locations::ClassLocations::build(self, control)?;
            let _ = self.class_locations.set(locations);
        }
        self.class_locations
            .get()
            .ok_or(InputError::InternalInvariant(
                "class location cache was not initialized",
            ))?
            .get(ty, self, control)
    }

    pub fn open(bytes: &'a [u8], control: &mut RunContext<'_>) -> Result<Self, InputError> {
        Self::open_with(bytes, control)
    }

    pub(crate) fn open_with(
        bytes: &'a [u8],
        control: &mut impl InputControl,
    ) -> Result<Self, InputError> {
        let container = DexContainer::open_with(bytes, control)?;
        Self::from_container(container, ByteOffset(0), control)
    }

    pub(crate) fn from_container(
        container: DexContainer<'a>,
        header_offset: ByteOffset,
        control: &mut impl InputControl,
    ) -> Result<Self, InputError> {
        let bytes = container.bytes;
        let header = super::header::read_member_header(container, header_offset, control)?;
        let mut cursor = Cursor::bounded(
            bytes,
            header.map_byte_range.start,
            header.map_byte_range.end,
        )?;
        let count = count_usize(cursor.read_u32_with(control)?, header.map_byte_range.start)?;
        let mut sections = [Section {
            kind: 0,
            count: 0,
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
            let item_count = cursor.read_u32_with(control)?;
            let start_byte = file_offset(
                cursor.read_u32_with(control)?,
                ByteOffset(cursor.position_byte().0 - 4),
            )?;
            if index > 0 {
                sections[index - 1].end_byte = start_byte;
            }
            sections[index] = Section {
                kind,
                count: item_count,
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
            dex: crate::DexId(0),
            class_facts: std::sync::Mutex::new(std::collections::HashMap::new()),
            facts_build: std::sync::Mutex::new(()),
            ownership: std::sync::OnceLock::new(),
            cache_build: std::sync::Mutex::new(()),
            class_locations: std::sync::OnceLock::new(),
            header,
            container,
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

    pub(crate) fn type_projection(
        &self,
        id: TypeId,
        control: &mut impl InputControl,
    ) -> Result<super::TypeProjection, InputError> {
        self.type_descriptor(id, control)?;
        Ok(super::type_projection::original(id))
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

    /// Number of original ``string_ids`` entries, not the count of strings that
    /// appear in code.
    pub fn string_ids_len(&self) -> u32 {
        self.header.string_ids.count
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
        control.checkpoint()?;
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
        let mut end = self
            .container
            .item_end(offset_byte, control)?
            .min(section.end_byte);
        let link = self.header.link_byte_range;
        if offset_byte >= link.start && offset_byte < link.end {
            return Err(malformed(InputProblem::OverlappingSections, offset_byte));
        }
        if offset_byte < link.start {
            end = end.min(link.start);
        }
        Cursor::bounded(self.bytes, offset_byte, end)
    }

    pub(crate) fn mapped_count(&self, kind: u16) -> u32 {
        map_kind(kind)
            .and_then(|(bit, _)| self.sections[bit.trailing_zeros() as usize])
            .map_or(0, |section| section.count)
    }
    pub(super) fn mapped_entry(
        &self,
        kind: u16,
        id: u32,
        stride: ByteLength,
        control: &mut impl InputControl,
    ) -> Result<Cursor<'a>, InputError> {
        let (bit, _) = map_kind(kind)
            .ok_or_else(|| malformed(InputProblem::InvalidReference, self.header.header_offset))?;
        let section = self.sections[bit.trailing_zeros() as usize]
            .ok_or_else(|| malformed(InputProblem::InvalidReference, self.header.header_offset))?;
        if id >= section.count {
            return Err(malformed(
                InputProblem::InvalidReference,
                section.start_byte,
            ));
        }
        let prefix = checked_range(
            ByteLength(section.end_byte.0),
            section.start_byte,
            count_usize(id, section.start_byte)?,
            stride,
        )?;
        let range = checked_range(ByteLength(section.end_byte.0), prefix.end, 1, stride)?;
        let cursor = self.item(kind, range.start, control)?;
        if cursor.remaining_byte().0 < stride.0 {
            return Err(malformed(InputProblem::Truncated, range.start));
        }
        control.checkpoint()?;
        Cursor::bounded(self.bytes, range.start, range.end)
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
        control.checkpoint()?;
        Cursor::bounded(self.bytes, range.start, range.end)
    }

    pub(crate) fn type_descriptor(
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
        let return_type_idx = TypeId(record.read_u32_with(control)?);
        let (return_type, return_shape) = self.type_descriptor(return_type_idx, control)?;
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
            return_type_idx,
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
        let (name, proto_idx) = self.method_header(method_idx, owner, control)?;
        let prototype = self.prototype(proto_idx, control)?;
        Ok(MethodDeclaration { name, prototype })
    }

    fn method_header(
        &self,
        method_idx: MethodId,
        owner: TypeId,
        control: &mut impl InputControl,
    ) -> Result<(DexString<'a>, ProtoId), InputError> {
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
        Ok((name, proto_idx))
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
        let field_type_idx = TypeId(u32::from(record.read_u16_with(control)?));
        let (field_type, shape) = self.type_descriptor(field_type_idx, control)?;
        let name = self.string_with(StringId(record.read_u32_with(control)?), control)?;
        name.validate_name(control)?;
        if shape.words() == 0 {
            return Err(malformed(
                InputProblem::InvalidDescriptor,
                ByteOffset(record.position_byte().0 - 6),
            ));
        }
        Ok(FieldDeclaration {
            name,
            field_type,
            field_type_idx,
        })
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

    pub(crate) fn local_field_flags(
        &self,
        field_id: FieldId,
        control: &mut impl InputControl,
    ) -> Result<Option<u32>, InputError> {
        let mut record = self.table(TableKey::Field(field_id), control)?;
        let owner = TypeId(u32::from(record.read_u16_with(control)?));
        let Some(class_id) = self.class_location(owner, control)? else {
            return Ok(None);
        };
        let mut found = None;
        self.class_with(class_id, control)?.visit_members_with(
            control,
            MemberSelection::Fields,
            |entry, _| -> Result<(), InputError> {
                let MemberEntry::Field(field) = entry else {
                    return Err(InputError::InternalInvariant(
                        "local field lookup acquired another member",
                    ));
                };
                if field.field_idx == field_id {
                    field.declaration?;
                    if found.is_some() {
                        return Err(malformed(
                            InputProblem::InvalidClassData,
                            self.header.class_defs.offset_byte,
                        ));
                    }
                    found = Some(field.access_flags);
                }
                Ok(())
            },
        )?;
        Ok(found)
    }
    pub(crate) fn local_method(
        &self,
        method_id: MethodId,
        control: &mut impl InputControl,
    ) -> Result<Option<LocalMethod<'a>>, InputError> {
        let mut method = self.table(TableKey::Method(method_id), control)?;
        let owner = TypeId(u32::from(method.read_u16_with(control)?));
        let Some(class_id) = self.class_location(owner, control)? else {
            return Ok(None);
        };
        let mut found = None;
        self.class_with(class_id, control)?.visit_members_with(
            control,
            MemberSelection::Header(method_id),
            |entry, control| -> Result<(), InputError> {
                if let MemberEntry::MethodHeader {
                    method_idx,
                    access_flags,
                    declaration,
                    ..
                } = entry
                {
                    declaration?;
                    if method_idx == method_id {
                        let class = self.class_with(class_id, control)?;
                        let context = self.class_context(class_id, control)?;
                        found = Some(LocalMethod {
                            access_flags,
                            owner_access_flags: class.access_flags,
                            parent: context.superclass?,
                            plain_scope: self
                                .no_class_annotations(context.annotations_offset, control)?
                                && context.interfaces?.is_empty(),
                        });
                    }
                }
                Ok(())
            },
        )?;
        Ok(found)
    }

    pub(crate) fn source_file(
        &self,
        id: ClassDefId,
        control: &mut impl InputControl,
    ) -> Result<Option<DexString<'a>>, InputError> {
        let mut record = self.table(TableKey::Class(id), control)?;
        record.skip(4, ByteLength(4), control)?;
        match record.read_u32_with(control)? {
            u32::MAX => Ok(None),
            id => self.string_with(StringId(id), control).map(Some),
        }
    }

    pub(super) fn class_annotations_offset(
        &self,
        id: ClassDefId,
        control: &mut impl InputControl,
    ) -> Result<ByteOffset, InputError> {
        let mut record = self.table(TableKey::Class(id), control)?;
        record.skip(5, ByteLength(4), control)?;
        let at = record.position_byte();
        file_offset(record.read_u32_with(control)?, at)
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
        control.checkpoint()?;
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
            owner_type: owner_idx,
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
            owner_type: owner_idx,
            owner,
            declaration: self.validate_field(id, owner_idx, control)?,
        })
    }
}

/// Original declared type relations of one class definition.
#[derive(Debug)]
pub struct ClassHierarchy<'a> {
    pub superclass: Option<DexString<'a>>,
    pub interfaces: Vec<DexString<'a>>,
}

/// Resolved facts of one original operand reference. Only the four xref target
/// families and prototypes are resolved; a call-site or method-handle slot keeps
/// its unsupported form rather than being expanded into members.
pub enum ReferenceFacts<'i, 'a> {
    String(DexString<'a>),
    Type(DexString<'a>),
    Method {
        owner: DexString<'a>,
        name: DexString<'a>,
        prototype: Prototype<'i, 'a>,
    },
    Field {
        owner: DexString<'a>,
        name: DexString<'a>,
        field_type: DexString<'a>,
    },
    Proto(Prototype<'i, 'a>),
    Unsupported,
}

impl std::fmt::Debug for ReferenceFacts<'_, '_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::String(_) => "ReferenceFacts::String",
            Self::Type(_) => "ReferenceFacts::Type",
            Self::Method { .. } => "ReferenceFacts::Method",
            Self::Field { .. } => "ReferenceFacts::Field",
            Self::Proto(_) => "ReferenceFacts::Proto",
            Self::Unsupported => "ReferenceFacts::Unsupported",
        })
    }
}

impl<'a> ProgramIndex<'a> {
    /// Resolves one original operand reference to displayable facts. This is a
    /// read-only table query; it starts no analysis and returns `Unsupported` for
    /// reference families outside the xref targets.
    pub fn reference_facts(
        &self,
        reference: crate::raw::OperandReference,
        control: &mut RunContext<'_>,
    ) -> Result<ReferenceFacts<'_, 'a>, InputError> {
        use crate::raw::OperandReference;
        Ok(match reference {
            OperandReference::String(id) => ReferenceFacts::String(self.string(id, control)?),
            OperandReference::Type(id) => {
                ReferenceFacts::Type(self.type_descriptor(id, control)?.0)
            }
            OperandReference::Method(id) => {
                let method = self.method_reference(id, control)?;
                ReferenceFacts::Method {
                    owner: method.owner,
                    name: method.declaration.name,
                    prototype: method.declaration.prototype,
                }
            }
            OperandReference::Field(id) => {
                let field = self.field_reference(id, control)?;
                ReferenceFacts::Field {
                    owner: field.owner,
                    name: field.declaration.name,
                    field_type: field.declaration.field_type,
                }
            }
            OperandReference::Proto(id) => ReferenceFacts::Proto(self.prototype(id, control)?),
            OperandReference::CallSite(_) | OperandReference::MethodHandle(_) => {
                ReferenceFacts::Unsupported
            }
        })
    }
}

pub(crate) struct ClassContext<'a> {
    pub superclass: Result<Option<DexString<'a>>, InputError>,
    pub interfaces: Result<&'a [u8], InputError>,
    pub values_offset: Result<Option<ByteOffset>, InputError>,
    pub annotations_offset: ByteOffset,
}

#[allow(dead_code)]
pub(crate) struct LocalMethod<'a> {
    pub access_flags: u32,
    pub owner_access_flags: u32,
    pub parent: Option<DexString<'a>>,
    pub plain_scope: bool,
}
pub(crate) struct MethodReference<'i, 'a> {
    pub owner_type: TypeId,
    pub owner: DexString<'a>,
    pub declaration: MethodDeclaration<'i, 'a>,
}

pub(crate) struct FieldReference<'a> {
    pub owner_type: TypeId,
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
    pub(crate) return_type_idx: TypeId,
    pub(super) parameters: &'a [u8],
    pub parameter_words: RegisterWordCount,
}

impl<'a> Prototype<'_, 'a> {
    pub(crate) fn visit_parameter_type_ids<C: InputControl, E: From<InputError>>(
        &self,
        control: &mut C,
        mut visit: impl FnMut(TypeId, DexString<'a>, &mut C) -> Result<(), E>,
    ) -> Result<(), E> {
        control.checkpoint().map_err(InputError::from)?;
        for word in self.parameters.as_chunks::<2>().0 {
            let id = TypeId(u32::from(u16::from_le_bytes(*word)));
            let (name, _) = self.index.type_descriptor(id, control)?;
            visit(id, name, control)?;
        }
        Ok(())
    }
    pub(crate) fn visit_parameter_types<C: InputControl, E: From<InputError>>(
        &self,
        control: &mut C,
        mut visit: impl FnMut(DexString<'a>, &mut C) -> Result<(), E>,
    ) -> Result<(), E> {
        self.visit_parameter_type_ids(control, |_, name, control| visit(name, control))
    }

    /// Visits the original parameter descriptors in declaration order.
    pub fn visit_parameters(
        &self,
        control: &mut RunContext<'_>,
        mut visit: impl FnMut(DexString<'a>, &mut RunContext<'_>) -> Result<(), InputError>,
    ) -> Result<(), InputError> {
        self.visit_parameter_types(control, |name, control| visit(name, control))
    }

    /// Compares a complete DEX descriptor, including the return type.
    pub fn matches(
        &self,
        descriptor: &[u16],
        control: &mut RunContext<'_>,
    ) -> Result<bool, InputError> {
        self.matches_with(descriptor, control)
    }

    pub(crate) fn matches_with(
        &self,
        descriptor: &[u16],
        control: &mut impl InputControl,
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

pub struct FieldDeclaration<'a> {
    pub name: DexString<'a>,
    pub field_type: DexString<'a>,
    pub field_type_idx: TypeId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FieldStorage {
    Static { ordinal: u32 },
    Instance,
}

/// Original class-data method group. It comes from the encoded group, never from
/// `access_flags`; `ordinal` is the member's position inside that group.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MethodGroup {
    Direct { ordinal: u32 },
    Virtual { ordinal: u32 },
}

/// The four original `class_data` group counts in encoded order. They describe
/// what the header declares, not how many members were read successfully.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ClassDataShape {
    pub static_fields: u32,
    pub instance_fields: u32,
    pub direct_methods: u32,
    pub virtual_methods: u32,
}

impl ClassDataShape {
    pub fn declared_members(&self) -> u64 {
        u64::from(self.static_fields)
            + u64::from(self.instance_fields)
            + u64::from(self.direct_methods)
            + u64::from(self.virtual_methods)
    }
}

pub struct FieldEntry<'a> {
    pub field_idx: FieldId,
    pub access_flags: u32,
    pub storage: FieldStorage,
    /// Position inside the original static or instance group.
    pub ordinal: u32,
    pub declaration: Result<FieldDeclaration<'a>, InputError>,
}

pub(crate) enum MemberSelection {
    AllMethods,
    Method(MethodId),
    Header(MethodId),
    AllHeaders,
    /// Every field and method header in original group order, without reading code.
    AllMembers,
    Fields,
}

pub(crate) enum MemberEntry<'i, 'a> {
    Field(FieldEntry<'a>),
    Method(MethodEntry<'i, 'a>),
    MethodHeader {
        method_idx: MethodId,
        access_flags: u32,
        group: MethodGroup,
        code_offset: Option<ByteOffset>,
        declaration: Result<MethodDeclaration<'i, 'a>, InputError>,
    },
}

pub struct MethodEntry<'i, 'a> {
    pub method_idx: MethodId,
    pub access_flags: u32,
    pub declaration: Result<MethodDeclaration<'i, 'a>, InputError>,
    pub body: Result<super::MethodBody<'a>, InputError>,
}

impl<'i, 'a> ClassView<'i, 'a> {
    /// Read-only original type relations of this definition: the declared
    /// superclass and the interface list in their original order. It describes
    /// direct edges only and performs no closure or hierarchy analysis.
    pub fn hierarchy(
        &self,
        control: &mut RunContext<'_>,
    ) -> Result<ClassHierarchy<'a>, InputError> {
        let context = self.index.class_context(self.class_def_idx, control)?;
        let superclass = context.superclass?;
        let mut interfaces = Vec::new();
        for word in context.interfaces?.as_chunks::<2>().0 {
            let id = TypeId(u32::from(u16::from_le_bytes(*word)));
            interfaces
                .try_reserve(1)
                .map_err(|_| InputError::Allocation {
                    offset_byte: self.index.header.class_defs.offset_byte,
                })?;
            interfaces.push(self.index.type_descriptor(id, control)?.0);
        }
        Ok(ClassHierarchy {
            superclass,
            interfaces,
        })
    }

    /// Visits original field definitions only, in class-data order. External
    /// ``field_ids`` references are not definitions. A boundary error returns
    /// after the delivered prefix; callers retain that prefix.
    pub fn visit_fields(
        &self,
        control: &mut RunContext<'_>,
        mut visit: impl FnMut(FieldEntry<'a>, &mut RunContext<'_>) -> Result<(), InputError>,
    ) -> Result<(), InputError> {
        self.visit_members_with(
            control,
            MemberSelection::Fields,
            |entry, control| match entry {
                MemberEntry::Field(field) => visit(field, control),
                _ => Err(InputError::InternalInvariant(
                    "field traversal delivered a method",
                )),
            },
        )
    }

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
                MemberEntry::MethodHeader { .. } => Err(InputError::InternalInvariant(
                    "header-only selection reached code visitation",
                )),
            },
        )
    }

    /// Enumerates this definition's original members and reports how far the
    /// class-data traversal reached. A record whose index/identity cannot be
    /// established stops coverage without discarding the delivered prefix or any
    /// sibling declaration; per-declaration read failures stay in each entry.
    pub(crate) fn walk_members_with<C: InputControl, E: From<InputError> + From<ControlError>>(
        &self,
        control: &mut C,
        selection: MemberSelection,
        mut visit: impl FnMut(MemberEntry<'i, 'a>, &mut C) -> Result<(), E>,
    ) -> Result<ClassDataWalk, E> {
        let Some(offset) = self.class_data_offset else {
            control.checkpoint()?;
            return Ok(ClassDataWalk::default());
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
        let mut traversal_error = None;
        let mut terminated_at = None;
        let mut delivered = [0u32; 4];
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
                    control.checkpoint()?;
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
                        traversal_error
                            .get_or_insert(malformed(InputProblem::RangeOverflow, record));
                        terminated_at.get_or_insert(record);
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
                        // The record extent is known, but this member has no usable
                        // identity. Coverage stays incomplete; the delivered prefix
                        // and every independently valid sibling remain available.
                        traversal_error
                            .get_or_insert(malformed(InputProblem::InvalidClassData, record));
                        terminated_at.get_or_insert(record);
                        if group == 0 {
                            prior = PriorState::Invalid;
                        }
                        continue;
                    }
                    match member {
                        EncodedMember::Field => {
                            if matches!(
                                selection,
                                MemberSelection::Method(_)
                                    | MemberSelection::Header(_)
                                    | MemberSelection::AllHeaders
                            ) {
                                continue;
                            }
                            let declaration = local(self.index.validate_field(
                                FieldId(index),
                                self.type_idx,
                                control,
                            ))?;
                            let group_slot = match kind {
                                MemberKind::Field => group,
                                MemberKind::Method => group + 2,
                            };
                            delivered[group_slot] = delivered[group_slot].saturating_add(1);
                            visit(
                                MemberEntry::Field(FieldEntry {
                                    field_idx: FieldId(index),
                                    access_flags,
                                    storage: if group == 0 {
                                        FieldStorage::Static { ordinal: item }
                                    } else {
                                        FieldStorage::Instance
                                    },
                                    ordinal: item,
                                    declaration,
                                }),
                                control,
                            )?;
                        }
                        EncodedMember::Method { code_offset } => {
                            if let MemberSelection::Method(selected)
                            | MemberSelection::Header(selected) = selection
                                && selected != MethodId(index)
                            {
                                continue;
                            }
                            let declaration = local(self.index.method_declaration(
                                MethodId(index),
                                self.type_idx,
                                control,
                            ))?;
                            if matches!(
                                selection,
                                MemberSelection::Header(_)
                                    | MemberSelection::AllHeaders
                                    | MemberSelection::AllMembers
                            ) {
                                let group_slot = match kind {
                                    MemberKind::Field => group,
                                    MemberKind::Method => group + 2,
                                };
                                delivered[group_slot] = delivered[group_slot].saturating_add(1);
                                visit(
                                    MemberEntry::MethodHeader {
                                        method_idx: MethodId(index),
                                        access_flags,
                                        group: if group == 0 {
                                            MethodGroup::Direct { ordinal: item }
                                        } else {
                                            MethodGroup::Virtual { ordinal: item }
                                        },
                                        code_offset,
                                        declaration,
                                    },
                                    control,
                                )?;
                                continue;
                            }
                            let words = declaration.as_ref().ok().map(|declaration| {
                                RegisterWordCount(
                                    declaration.prototype.parameter_words.0
                                        + u64::from(
                                            access_flags & crate::substrate::flags::STATIC == 0,
                                        ),
                                )
                            });
                            let body = local(read_code(
                                self.index,
                                code_offset,
                                MethodId(index),
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
        Ok(ClassDataWalk {
            shape: ClassDataShape {
                static_fields: counts[0],
                instance_fields: counts[1],
                direct_methods: counts[2],
                virtual_methods: counts[3],
            },
            delivered,
            terminated_at,
            issue: traversal_error,
        })
    }

    pub(crate) fn visit_members_with<C: InputControl, E: From<InputError> + From<ControlError>>(
        &self,
        control: &mut C,
        selection: MemberSelection,
        visit: impl FnMut(MemberEntry<'i, 'a>, &mut C) -> Result<(), E>,
    ) -> Result<(), E> {
        let walk = self.walk_members_with(control, selection, visit)?;
        match walk.issue {
            Some(error) => Err(error.into()),
            None => Ok(()),
        }
    }
}

/// How far one original class-data traversal reached, together with the declared
/// four-group counts. `issue` reports unusable coverage, never a declaration read.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct ClassDataWalk {
    pub shape: ClassDataShape,
    /// Members actually delivered in each original group, including entries whose
    /// own declaration read failed. A prefix shorter than `shape` is explicit
    /// coverage loss, not an empty group.
    pub delivered: [u32; 4],
    /// First original class-data byte whose record could not be located.
    pub terminated_at: Option<ByteOffset>,
    pub issue: Option<InputError>,
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
