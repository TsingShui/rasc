use super::catalog::{MemberEntry, MemberSelection};
use super::formats::FlowKind;
use super::strings::TypeShape;
use super::values::EncodedValues;
use super::{
    CheckedMethodLayout, Cursor, DexString, InputError, InputProblem, MethodBody,
    MethodDeclaration, MethodError, ProgramIndex, RawMethodView, file_offset, malformed,
};
use crate::pipeline::control::InputControl;
use crate::raw::{LayoutKind, OperandReference};
use crate::{
    ByteLength, ByteOffset, ByteRange, ClassDefId, CodeUnitOffset, MethodBudget, MethodId,
};

/// Original annotation associations checked for one selected member. Other
/// members' annotation bodies are independent and are not sealed by this value.
pub struct CheckedAnnotations {
    method: Option<ByteRange>,
    parameters: Option<ByteRange>,
}

impl CheckedAnnotations {
    pub fn method_range(&self) -> Option<ByteRange> {
        self.method
    }
    pub fn parameter_range(&self) -> Option<ByteRange> {
        self.parameters
    }
}

/// A raw executable method with checked layout, explicit reference contents,
/// return/allocation operand kinds, debug records and selected annotations.
/// This is not register-type, dispatch, exception-effect or Java verification.
pub struct ValidatedRawMethod<'a> {
    layout: CheckedMethodLayout<'a>,
    annotations: CheckedAnnotations,
}

impl<'a> ValidatedRawMethod<'a> {
    pub fn layout(&self) -> &CheckedMethodLayout<'a> {
        &self.layout
    }
    pub fn annotations(&self) -> &CheckedAnnotations {
        &self.annotations
    }
}

/// A checked no-code declaration. Code absence follows the original native or
/// abstract flags; annotations are checked without fabricating an empty body.
pub struct ValidatedRawDeclaration {
    annotations: CheckedAnnotations,
}

impl ValidatedRawDeclaration {
    pub fn annotations(&self) -> &CheckedAnnotations {
        &self.annotations
    }
}

pub enum RawPrintable<'a> {
    Method(ValidatedRawMethod<'a>),
    Declaration(ValidatedRawDeclaration),
}

pub(crate) enum ReferencedFieldMetadata {
    External,
    Absent,
    Declared(OwnFieldMetadata),
}
pub(crate) struct OwnFieldMetadata {
    _sealed: (),
    pub access_flags: u32,
    pub same_named_fields: usize,
}
/// The input seam for one original declaration in a selected logical program.
/// Construction is restricted to the owning input validator.
pub struct MethodInput<'i, 'a> {
    index: &'i ProgramIndex<'a>,
    class_id: ClassDefId,
    method_id: MethodId,
    header_offset: ByteOffset,
    owner: DexString<'a>,
    access_flags: u32,
    owner_access_flags: u32,
    declaration: MethodDeclaration<'i, 'a>,
    raw: RawPrintable<'a>,
}

impl<'i, 'a> MethodInput<'i, 'a> {
    pub(crate) fn program(&self) -> &'i ProgramIndex<'a> {
        self.index
    }

    pub(crate) fn type_projection(
        &self,
        id: crate::TypeId,
        budget: &mut MethodBudget<'_, '_>,
    ) -> Result<super::TypeProjection, crate::RequestError> {
        self.index.type_descriptor(id, budget)?;
        Ok(super::type_projection::original(id))
    }
    pub(crate) fn projected_name(
        &self,
        key: super::TypeProjection,
        budget: &mut MethodBudget<'_, '_>,
    ) -> Result<DexString<'a>, crate::RequestError> {
        super::type_projection::suffix(self.type_descriptor(key.root(), budget)?, key, budget)
    }
    pub(crate) fn component_projection(
        &self,
        key: super::TypeProjection,
        budget: &mut MethodBudget<'_, '_>,
    ) -> Result<super::TypeProjection, crate::RequestError> {
        super::type_projection::component(self, key, budget)
    }
    pub(super) fn exact_type(
        &self,
        suffix: DexString<'_>,
        budget: &mut MethodBudget<'_, '_>,
    ) -> Result<Option<crate::TypeId>, crate::RequestError> {
        let mut found = None;
        for index in 0..self.index.header.type_ids.count {
            budget.charge_work(1)?;
            let id = crate::TypeId(index);
            let name = self.type_descriptor(id, budget)?;
            if name.utf16_len() != suffix.utf16_len() {
                continue;
            }
            let mut left = name.reader()?;
            let mut right = suffix.reader()?;
            let mut equal = true;
            while let Some(unit) = left.next(budget)? {
                if right.next(budget)? != Some(unit) {
                    equal = false;
                    break;
                }
            }
            if equal {
                if found.is_some() {
                    return Ok(None);
                }
                found = Some(id);
            }
        }
        Ok(found)
    }

    pub(crate) fn type_descriptor(
        &self,
        id: crate::TypeId,
        budget: &mut MethodBudget<'_, '_>,
    ) -> Result<DexString<'a>, crate::RequestError> {
        Ok(self.index.type_descriptor(id, budget)?.0)
    }

    pub(crate) fn throws_metadata(
        &self,
        budget: &mut MethodBudget<'_, '_>,
    ) -> Result<super::ThrowsMetadata<'a>, crate::RequestError> {
        let annotations = match &self.raw {
            RawPrintable::Method(raw) => raw.annotations(),
            RawPrintable::Declaration(raw) => raw.annotations(),
        };
        super::throws::read(self.index, annotations.method_range(), budget)
    }
    pub(crate) fn own_field_metadata(
        &self,
        reference: crate::FieldId,
        budget: &mut MethodBudget<'_, '_>,
    ) -> Result<Option<OwnFieldMetadata>, crate::RequestError> {
        let field = self.index.field_reference(reference, budget)?;
        let owner = crate::substrate::owned::copy_units(self.owner, budget)?;
        if !field.owner.matches_with(&owner, budget)? {
            return Ok(None);
        }
        let name = crate::substrate::owned::copy_units(field.declaration.name, budget)?;
        let mut flags = None;
        let mut same_named_fields = 0usize;
        self.index
            .class_with(self.class_id, budget)?
            .visit_members_with(
                budget,
                MemberSelection::Fields,
                |entry, budget| -> Result<(), crate::RequestError> {
                    let MemberEntry::Field(field) = entry else {
                        return Err(crate::RequestError::InternalInvariant(
                            "field projection acquired another member",
                        ));
                    };
                    let declaration = field.declaration?;
                    if declaration.name.matches_with(&name, budget)? {
                        same_named_fields = same_named_fields
                            .checked_add(1)
                            .ok_or(crate::RequestError::PlatformCapacity)?;
                    }
                    if field.field_idx == reference {
                        flags = Some(field.access_flags);
                    }
                    Ok(())
                },
            )?;
        Ok(flags.map(|access_flags| OwnFieldMetadata {
            _sealed: (),
            access_flags,
            same_named_fields,
        }))
    }
    pub(crate) fn referenced_field_metadata(
        &self,
        reference: crate::FieldId,
        budget: &mut MethodBudget<'_, '_>,
    ) -> Result<ReferencedFieldMetadata, crate::RequestError> {
        let Some(class) = self.index.field_owner_class(reference, budget)? else {
            return Ok(ReferencedFieldMetadata::External);
        };
        let original = self.index.field_reference(reference, budget)?;
        let name = crate::substrate::owned::copy_units(original.declaration.name, budget)?;
        let mut flags = None;
        let mut same_named_fields = 0usize;
        self.index.class_with(class, budget)?.visit_members_with(
            budget,
            MemberSelection::Fields,
            |entry, budget| -> Result<(), crate::RequestError> {
                let MemberEntry::Field(field) = entry else {
                    return Err(crate::RequestError::InternalInvariant(
                        "field metadata acquired another member",
                    ));
                };
                let declaration = field.declaration?;
                if declaration.name.matches_with(&name, budget)? {
                    same_named_fields = same_named_fields
                        .checked_add(1)
                        .ok_or(crate::RequestError::PlatformCapacity)?;
                }
                if field.field_idx == reference {
                    flags = Some(field.access_flags);
                }
                Ok(())
            },
        )?;
        Ok(match flags {
            Some(access_flags) => ReferencedFieldMetadata::Declared(OwnFieldMetadata {
                _sealed: (),
                access_flags,
                same_named_fields,
            }),
            None => ReferencedFieldMetadata::Absent,
        })
    }
    pub(crate) fn type_name(
        &self,
        id: crate::TypeId,
        budget: &mut MethodBudget<'_, '_>,
    ) -> Result<DexString<'a>, InputError> {
        self.index.type_name(id, budget)
    }
    pub(crate) fn string_literal(
        &self,
        id: crate::StringId,
        budget: &mut MethodBudget<'_, '_>,
    ) -> Result<DexString<'a>, InputError> {
        self.index.string_with(id, budget)
    }
    pub(crate) fn local_method(
        &self,
        id: MethodId,
        budget: &mut MethodBudget<'_, '_>,
    ) -> Result<Option<super::catalog::LocalMethod<'a>>, InputError> {
        self.index.local_method(id, budget)
    }
    pub(crate) fn field_reference(
        &self,
        id: crate::FieldId,
        budget: &mut MethodBudget<'_, '_>,
    ) -> Result<super::catalog::FieldReference<'a>, InputError> {
        self.index.field_reference(id, budget)
    }
    pub(crate) fn method_reference(
        &self,
        id: MethodId,
        budget: &mut MethodBudget<'_, '_>,
    ) -> Result<super::catalog::MethodReference<'_, 'a>, InputError> {
        self.index.method_reference(id, budget)
    }
    pub(crate) fn plain_owner_scope(
        &self,
        parent: Option<&[u16]>,
        budget: &mut MethodBudget<'_, '_>,
    ) -> Result<bool, crate::RequestError> {
        let context = self.index.class_context(self.class_id, budget)?;
        if !self
            .index
            .no_class_annotations(context.annotations_offset, budget)?
            || !context.interfaces?.is_empty()
        {
            return Ok(false);
        }
        Ok(match (context.superclass?, parent) {
            (None, None) => true,
            (Some(original), Some(parent)) => original.matches_with(parent, budget)?,
            _ => false,
        })
    }
    pub(crate) fn qualifier_shadowed(
        &self,
        prefix: &[u16],
        budget: &mut MethodBudget<'_, '_>,
    ) -> Result<bool, crate::RequestError> {
        let mut shadowed = false;
        self.index
            .class_with(self.class_id, budget)?
            .visit_members_with(
                budget,
                MemberSelection::Fields,
                |entry, budget| -> Result<(), crate::RequestError> {
                    let MemberEntry::Field(field) = entry else {
                        return Err(crate::RequestError::InternalInvariant(
                            "field-only scope check acquired another member",
                        ));
                    };
                    if field.declaration?.name.matches_with(prefix, budget)? {
                        shadowed = true;
                    }
                    Ok(())
                },
            )?;
        Ok(shadowed)
    }
    pub fn class_id(&self) -> ClassDefId {
        self.class_id
    }
    pub fn method_id(&self) -> MethodId {
        self.method_id
    }
    pub fn header_offset(&self) -> ByteOffset {
        self.header_offset
    }
    pub fn owner(&self) -> DexString<'a> {
        self.owner
    }
    pub fn access_flags(&self) -> u32 {
        self.access_flags
    }
    pub fn owner_access_flags(&self) -> u32 {
        self.owner_access_flags
    }
    pub fn declaration(&self) -> &MethodDeclaration<'i, 'a> {
        &self.declaration
    }
    pub fn raw(&self) -> &RawPrintable<'a> {
        &self.raw
    }
    pub fn into_raw(self) -> RawPrintable<'a> {
        self.raw
    }
}

impl<'a> ProgramIndex<'a> {
    /// Validates a selected original method for analysis. Malformed declarations,
    /// code, references or metadata return classified errors before a sealed
    /// product is published. Work and fallible scratch use the normal method
    /// budget; the original input remains available to the independent debug exit.
    pub fn method_input<'i>(
        &'i self,
        class_id: ClassDefId,
        method_id: MethodId,
        budget: &mut MethodBudget<'_, '_>,
    ) -> Result<MethodInput<'i, 'a>, MethodError> {
        let class = self.class_with(class_id, budget)?;
        let mut selected = None;
        class.visit_members_with(budget, MemberSelection::Method(method_id), |entry, _| {
            if let MemberEntry::Method(method) = entry {
                if selected.is_some() {
                    return Err(MethodError::Input(malformed(
                        InputProblem::InvalidClassData,
                        self.header.class_defs.offset_byte,
                    )));
                }
                selected = Some(method);
            }
            Ok(())
        })?;
        let method = selected.ok_or_else(|| {
            malformed(
                InputProblem::InvalidReference,
                self.header.method_ids.offset_byte,
            )
        })?;
        let body = method.body?;
        let declaration = method.declaration?;
        let annotations = self.check_method_annotations(
            class_id,
            method_id,
            declaration.prototype.parameters.len() / 2,
            budget,
        )?;
        let raw = match body {
            MethodBody::DeclarationOnly => {
                RawPrintable::Declaration(ValidatedRawDeclaration { annotations })
            }
            MethodBody::Code(code) => {
                let layout = self.check_method_layout(&code, budget)?;
                self.check_reference_contents(
                    &code,
                    declaration.prototype.return_type.shape(budget)?,
                    budget,
                )?;
                RawPrintable::Method(ValidatedRawMethod {
                    layout,
                    annotations,
                })
            }
        };
        budget.checkpoint()?;
        Ok(MethodInput {
            index: self,
            class_id,
            method_id,
            header_offset: self.header.header_offset,
            owner: class.descriptor,
            access_flags: method.access_flags,
            owner_access_flags: class.access_flags,
            declaration,
            raw,
        })
    }

    fn check_reference_contents(
        &self,
        code: &RawMethodView<'_>,
        return_shape: TypeShape,
        budget: &mut MethodBudget<'_, '_>,
    ) -> Result<(), MethodError> {
        let mut pc = CodeUnitOffset(0);
        while pc.0 < code.code_units().0 {
            budget.charge_work(1)?;
            let layout = code.layout_at(pc)?;
            if let LayoutKind::Instruction(spec) = layout.kind() {
                for reference in layout.references().into_iter().flatten() {
                    match reference.target {
                        OperandReference::String(id) => {
                            self.string_with(id, budget)?;
                        }
                        OperandReference::Type(id) => {
                            let (descriptor, shape) = self.type_descriptor(id, budget)?;
                            let valid = match spec.opcode {
                                0x1f | 0x20 => matches!(shape, TypeShape::Class | TypeShape::Array),
                                0x22 => matches!(shape, TypeShape::Class),
                                0x23..=0x25 => matches!(shape, TypeShape::Array),
                                _ => true,
                            };
                            if !valid {
                                return Err(bad_type(pc));
                            }
                            if matches!(spec.opcode, 0x24 | 0x25) {
                                let mut units = descriptor.reader()?;
                                units.next(budget)?;
                                if matches!(units.next(budget)?, Some(0x4a | 0x44)) {
                                    return Err(bad_type(pc));
                                }
                            }
                        }
                        OperandReference::Field(id) => {
                            let field = self.field_reference(id, budget)?;
                            if matches!(spec.opcode, 0x52..=0x6d) {
                                let first = field.declaration.field_type.reader()?.next(budget)?;
                                let matches_kind = match (spec.opcode - 0x52) % 7 {
                                    0 => matches!(first, Some(73 | 70)),
                                    1 => matches!(first, Some(74 | 68)),
                                    2 => matches!(first, Some(76 | 91)),
                                    3 => first == Some(90),
                                    4 => first == Some(66),
                                    5 => first == Some(67),
                                    6 => first == Some(83),
                                    _ => false,
                                };
                                if !matches_kind {
                                    return Err(MethodError::Malformed {
                                        pc,
                                        problem: super::MethodProblem::InvalidFieldKind,
                                    });
                                }
                            }
                            if matches!(spec.opcode, 0x52..=0x6d)
                                && let Some(flags) = self.local_field_flags(id, budget)?
                                && (flags & 8 != 0) != matches!(spec.opcode, 0x60..=0x6d)
                            {
                                return Err(MethodError::Malformed {
                                    pc,
                                    problem: super::MethodProblem::InvalidFieldKind,
                                });
                            }
                        }
                        OperandReference::Method(id) => {
                            self.method_reference(id, budget)?;
                            if matches!(spec.opcode,0x6e..=0x72|0x74..=0x78)
                                && let Some(target) = self.local_method(id, budget)?
                                && (target.access_flags & 8 != 0)
                                    != matches!(spec.opcode, 0x71 | 0x77)
                            {
                                return Err(MethodError::Malformed {
                                    pc,
                                    problem: super::MethodProblem::InvalidInvokeKind,
                                });
                            }
                        }
                        OperandReference::Proto(id) => {
                            self.prototype(id, budget)?;
                        }
                        OperandReference::CallSite(_) | OperandReference::MethodHandle(_) => {
                            return Err(MethodError::ReferenceOutOfBounds {
                                pc,
                                reference: reference.target,
                            });
                        }
                    }
                }
                if matches!(spec.flow, FlowKind::Return) {
                    let expected = match return_shape {
                        TypeShape::Void => 0x0e,
                        TypeShape::Class | TypeShape::Array => 0x11,
                        other if other.words() == 2 => 0x10,
                        _ => 0x0f,
                    };
                    if spec.opcode != expected {
                        return Err(super::structure::bad(
                            pc,
                            super::MethodProblem::InvalidResultKind,
                        ));
                    }
                }
            }
            pc = pc
                .checked_advance(layout.width())
                .ok_or(MethodError::InternalInvariant(
                    "checked raw instruction extent overflowed",
                ))?;
        }
        Ok(())
    }

    fn check_method_annotations(
        &self,
        class_id: ClassDefId,
        method_id: MethodId,
        parameters: usize,
        budget: &mut MethodBudget<'_, '_>,
    ) -> Result<CheckedAnnotations, MethodError> {
        let offset = self.class_annotations_offset(class_id, budget)?;
        let mut checked = CheckedAnnotations {
            method: None,
            parameters: None,
        };
        if offset == ByteOffset(0) {
            return Ok(checked);
        }
        let directory = self.annotation_directory(offset, budget)?;
        if let Some(offset) = selected_offset(directory.methods, method_id, budget)? {
            checked.method = Some(self.check_annotation_set(offset, budget)?);
        }
        if let Some(offset) = selected_offset(directory.parameters, method_id, budget)? {
            let mut list = self.annotation_parameters(offset, parameters, budget)?;
            while list.remaining_byte() != ByteLength(0) {
                let at = list.position_byte();
                let set = file_offset(list.read_u32_with(budget)?, at)?;
                if set != ByteOffset(0) {
                    self.check_annotation_set(set, budget)?;
                }
            }
            checked.parameters = Some(ByteRange {
                start: offset,
                end: list.position_byte(),
            });
        }
        Ok(checked)
    }

    fn check_annotation_set(
        &self,
        offset: ByteOffset,
        budget: &mut impl InputControl,
    ) -> Result<ByteRange, MethodError> {
        let mut offsets = self.annotation_offsets(0x1003, offset, budget)?;
        let mut last = None;
        while offsets.remaining_byte() != ByteLength(0) {
            let (_, _, _, mut values) = self.next_annotation(&mut offsets, &mut last, budget)?;
            consume_values(self, &mut values, budget)?;
        }
        Ok(ByteRange {
            start: offset,
            end: offsets.position_byte(),
        })
    }
}

fn consume_values(
    index: &ProgramIndex<'_>,
    values: &mut EncodedValues<'_>,
    budget: &mut impl InputControl,
) -> Result<(), MethodError> {
    while values.next(index, budget)?.is_some() {}
    Ok(())
}

fn selected_offset(
    mut records: Cursor<'_>,
    method_id: MethodId,
    budget: &mut impl InputControl,
) -> Result<Option<ByteOffset>, InputError> {
    let mut previous = None;
    let mut selected = None;
    while records.remaining_byte() != ByteLength(0) {
        let at = records.position_byte();
        let id = MethodId(records.read_u32_with(budget)?);
        let offset = file_offset(records.read_u32_with(budget)?, at)?;
        if previous.is_some_and(|previous| previous >= id) {
            return Err(malformed(InputProblem::InvalidReference, at));
        }
        previous = Some(id);
        if id == method_id {
            selected = Some(offset);
        }
    }
    Ok(selected)
}

fn bad_type(pc: CodeUnitOffset) -> MethodError {
    super::structure::bad(pc, super::MethodProblem::InvalidTypeOperand)
}
