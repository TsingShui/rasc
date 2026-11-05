use super::class_locations::ClassLocations;
use super::header::{SUPPORTED_MAP_KINDS, map_kind};
use super::strings::{DexString, TypeShape};
use super::{
    Cursor, DexContainer, DexHeader, InputError, InputProblem, checked_range, count_usize,
    file_offset, malformed,
};
use crate::{ByteLength, ByteOffset, ClassDefId, FieldId, MethodId, ProtoId, StringId, TypeId};

#[derive(Clone, Copy)]
pub(super) struct Section {
    pub(super) kind: u16,
    pub(super) count: u32,
    pub(super) start_byte: ByteOffset,
    pub(super) end_byte: ByteOffset,
}

pub(crate) struct LocalDeclarations<'a> {
    pub(super) bytes: &'a [u8],
    pub(super) header: DexHeader,
    container: DexContainer<'a>,
    pub(super) sections: [Option<Section>; SUPPORTED_MAP_KINDS],
    locations: std::sync::OnceLock<ClassLocations>,
    build: std::sync::Mutex<()>,
}
pub(crate) enum LocalParent {
    Missing,
    Root,
    Type(TypeId),
}
pub(super) struct ClassRecord {
    pub type_id: TypeId,
    pub access_flags: u32,
    pub superclass: u32,
    pub interfaces: u32,
    pub source_file: u32,
    pub annotations: u32,
    pub class_data: u32,
    pub values: u32,
}
impl<'a> LocalDeclarations<'a> {
    pub(super) fn new(
        container: DexContainer<'a>,
        header: DexHeader,
        sections: [Option<Section>; SUPPORTED_MAP_KINDS],
    ) -> Self {
        Self {
            bytes: container.bytes,
            header,
            container,
            sections,
            locations: std::sync::OnceLock::new(),
            build: std::sync::Mutex::new(()),
        }
    }
    pub(crate) fn class_count(&self) -> u32 {
        self.header.class_defs.count
    }
    pub(super) fn class_type_id(&self, class: ClassDefId) -> Result<TypeId, InputError> {
        let mut record = self.table(TableKey::Class(class))?;
        Ok(TypeId(record.read_u32()?))
    }
    pub(super) fn class_record(&self, class: ClassDefId) -> Result<ClassRecord, InputError> {
        let mut record = self.table(TableKey::Class(class))?;
        Ok(ClassRecord {
            type_id: TypeId(record.read_u32()?),
            access_flags: record.read_u32()?,
            superclass: record.read_u32()?,
            interfaces: record.read_u32()?,
            source_file: record.read_u32()?,
            annotations: record.read_u32()?,
            class_data: record.read_u32()?,
            values: record.read_u32()?,
        })
    }
    pub(super) fn checked_class(
        &self,
        class: ClassDefId,
    ) -> Result<(ClassRecord, DexString<'a>), InputError> {
        let record = self.class_record(class)?;
        let (name, shape) = self.type_descriptor(record.type_id)?;
        if !matches!(shape, TypeShape::Class) {
            return Err(malformed(
                InputProblem::InvalidDescriptor,
                self.table(TableKey::Class(class))?.position_byte(),
            ));
        }
        Ok((record, name))
    }
    pub(super) fn class_location(&self, ty: TypeId) -> Result<Option<ClassDefId>, InputError> {
        if self.locations.get().is_none() {
            let _lock = self
                .build
                .lock()
                .map_err(|_| InputError::InternalInvariant("input cache construction poisoned"))?;
            if let Some(locations) = self.locations.get() {
                return locations.get(ty, self);
            }
            let locations = ClassLocations::build(self)?;
            let _ = self.locations.set(locations);
        }
        self.locations
            .get()
            .ok_or(InputError::InternalInvariant(
                "class location cache was not initialized",
            ))?
            .get(ty, self)
    }
    pub(crate) fn parent(&self, ty: TypeId) -> Result<LocalParent, InputError> {
        let Some(class) = self.class_location(ty)? else {
            return Ok(LocalParent::Missing);
        };
        let (record, _) = self.checked_class(class)?;
        self.superclass(record.superclass)?;
        Ok(if record.superclass == u32::MAX {
            LocalParent::Root
        } else {
            LocalParent::Type(TypeId(record.superclass))
        })
    }
    pub(super) fn item(
        &self,
        kind: u16,
        offset_byte: ByteOffset,
    ) -> Result<Cursor<'a>, InputError> {
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
        let mut end = self.container.item_end(offset_byte)?.min(section.end_byte);
        let link = self.header.link_byte_range;
        if offset_byte >= link.start && offset_byte < link.end {
            return Err(malformed(InputProblem::OverlappingSections, offset_byte));
        }
        if offset_byte < link.start {
            end = end.min(link.start);
        }
        Cursor::bounded(self.bytes, offset_byte, end)
    }

    pub(super) fn table(&self, key: TableKey) -> Result<Cursor<'a>, InputError> {
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

        Cursor::bounded(self.bytes, range.start, range.end)
    }

    pub(super) fn type_descriptor(
        &self,
        type_idx: TypeId,
    ) -> Result<(DexString<'a>, TypeShape), InputError> {
        let mut record = self.table(TableKey::Type(type_idx))?;
        let string_idx = StringId(record.read_u32()?);
        let descriptor = self.string(string_idx)?;
        let shape = descriptor.shape()?;
        Ok((descriptor, shape))
    }

    pub(super) fn string(&self, string_idx: StringId) -> Result<DexString<'a>, InputError> {
        let mut record = self.table(TableKey::String(string_idx))?;
        let offset = file_offset(record.read_u32()?, ByteOffset(record.position_byte().0 - 4))?;
        DexString::read(self.bytes, self.item(0x2002, offset)?)
    }

    pub(super) fn superclass(&self, index: u32) -> Result<Option<DexString<'a>>, InputError> {
        if index == u32::MAX {
            return Ok(None);
        }
        let (name, shape) = self.type_descriptor(TypeId(index))?;
        if !matches!(shape, TypeShape::Class) {
            return Err(malformed(
                InputProblem::InvalidDescriptor,
                self.header.type_ids.offset_byte,
            ));
        }
        Ok(Some(name))
    }
}

pub(super) enum TableKey {
    String(StringId),
    Type(TypeId),
    Proto(ProtoId),
    Field(FieldId),
    Method(MethodId),
    Class(ClassDefId),
}
