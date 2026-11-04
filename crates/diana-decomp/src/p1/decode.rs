use crate::input::{MethodSource, ProgramIndex, RawPrintable, strings::TypeShape};
use crate::raw::OperandReference;
use crate::substrate::flags::STATIC;
use crate::substrate::owned::push;
use crate::substrate::slots::{RegisterSlot, SlotWidth};
use crate::{CodeUnitOffset, RegisterId, RequestError, TypeId};

use super::disassembly::DecodedFacts;
use super::reference::{
    CheckedMethodHandleFact, CheckedPrototype, CheckedReferenceFact, ReferenceFactEntry, type_form,
};
use super::{DecodedOutcome, EntryLayout, EntryParameter, MethodError, MethodProblem};

impl<'a> ProgramIndex<'a> {
    /// P1: validates the original instruction structure, decodes the complete
    /// instruction/payload directory and associates original references with
    /// checked targets. Consumes only the reader product and returns the P1
    /// product; it never selects members or reads class-data.
    pub fn decode_method_source(
        &self,
        source: &MethodSource<'_, 'a>,
    ) -> Result<DecodedOutcome<'a>, MethodError> {
        let RawPrintable::Method(code) = source.raw() else {
            return Err(MethodError::InternalInvariant(
                "P1 was asked to decode a declaration-only source",
            ));
        };
        let (_, return_shape) = source
            .program()
            .type_descriptor(crate::TypeId(source.declaration().return_type_id))?;
        let items = match self.build_items(code) {
            Ok(items) => items,
            Err((prefix, error)) => return outcome(prefix, error),
        };
        let layout = match self.check_method_layout(code, &items) {
            Ok(layout) => layout,
            Err(error) => return outcome(items, error),
        };
        let reference_facts = match self.check_reference_contents(&items, return_shape) {
            Ok(facts) => facts,
            Err(error) => return outcome(items, error),
        };
        if let Err(error) = check_reference_coverage(&items, &reference_facts) {
            return outcome(items, error);
        }
        let protection = match crate::input::protection::read(&layout).map_err(method_error) {
            Ok(protection) => protection,
            Err(error) => return outcome(items, error),
        };
        let entry = match build_entry_layout(source, code) {
            Ok(entry) => entry,
            Err(error) => return outcome(items, error),
        };
        Ok(DecodedOutcome {
            decoded: Some(crate::input::ProgramIndex::assemble_decoded(
                code,
                layout,
                protection,
                items,
                DecodedFacts {
                    class_id: source.class_id(),
                    declaration: source.declaration().clone(),
                    access_flags: source.access_flags(),
                    entry,
                    annotations: source.annotations(),
                    metadata_issue: source.metadata().is_err(),
                    reference_facts,
                },
            )),
            prefix: Vec::new(),
            error: None,
        })
    }

    fn check_reference_contents(
        &self,
        items: &[super::DecodedItem],
        return_shape: TypeShape,
    ) -> Result<Vec<ReferenceFactEntry<'a>>, MethodError> {
        let mut facts = Vec::new();
        for item in items {
            let super::DecodedItem::Instruction(instruction) = item else {
                continue;
            };
            let pc = instruction.pc();
            let spec = instruction.spec();
            for reference in instruction.references() {
                let fact = match reference.target {
                    OperandReference::String(id) => CheckedReferenceFact::String {
                        value: self.string(id)?,
                    },
                    OperandReference::Type(id) => {
                        let (descriptor, shape) = self.type_descriptor(id)?;
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
                            units.next()?;
                            if matches!(units.next()?, Some(0x4a | 0x44)) {
                                return Err(bad_type(pc));
                            }
                        }
                        CheckedReferenceFact::Type {
                            descriptor,
                            form: type_form(shape),
                            words: shape.words(),
                        }
                    }
                    OperandReference::Field(id) => {
                        let field = self.field_reference(id)?;
                        if matches!(spec.opcode, 0x52..=0x6d) {
                            let first = field.declaration.field_type.reader()?.next()?;
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
                                    problem: MethodProblem::InvalidFieldKind,
                                });
                            }
                        }
                        if matches!(spec.opcode, 0x52..=0x6d)
                            && let Some(flags) = self.local_field_flags(id)?
                            && (flags & 8 != 0) != matches!(spec.opcode, 0x60..=0x6d)
                        {
                            return Err(MethodError::Malformed {
                                pc,
                                problem: MethodProblem::InvalidFieldKind,
                            });
                        }
                        CheckedReferenceFact::Field {
                            owner: field.owner,
                            owner_type: field.owner_type,
                            name: field.declaration.name,
                            field_type: field.declaration.field_type,
                            field_type_id: field.declaration.field_type_idx,
                        }
                    }
                    OperandReference::Method(id) => {
                        let method = self.method_reference(id)?;
                        if matches!(spec.opcode, 0x6e..=0x72 | 0x74..=0x78)
                            && let Some(target) = self.local_method(id)?
                            && (target.access_flags & crate::substrate::flags::STATIC != 0)
                                != matches!(spec.opcode, 0x71 | 0x77)
                        {
                            return Err(MethodError::Malformed {
                                pc,
                                problem: MethodProblem::InvalidInvokeKind,
                            });
                        }
                        let prototype =
                            CheckedPrototype::from_prototype(&method.declaration.prototype)?;
                        CheckedReferenceFact::Method {
                            owner: method.owner,
                            owner_type: method.owner_type,
                            name: method.declaration.name,
                            prototype,
                        }
                    }
                    OperandReference::Proto(id) => {
                        let proto = self.prototype(id)?;
                        CheckedReferenceFact::Proto {
                            prototype: CheckedPrototype::from_prototype(&proto)?,
                        }
                    }
                    OperandReference::CallSite(id) => {
                        let callsite = self.call_site(id)?;
                        CheckedReferenceFact::CallSite {
                            name: callsite.name,
                            handle: CheckedMethodHandleFact::new(
                                callsite.handle.kind,
                                callsite.handle.member,
                            ),
                            prototype: CheckedPrototype::from_prototype(&callsite.prototype)?,
                        }
                    }
                    OperandReference::MethodHandle(id) => {
                        let handle = self.method_handle(id)?;
                        CheckedReferenceFact::MethodHandle {
                            handle: CheckedMethodHandleFact::new(handle.kind, handle.member),
                        }
                    }
                };
                push(
                    &mut facts,
                    ReferenceFactEntry {
                        pc,
                        slot: reference.slot,
                        fact,
                    },
                )
                .map_err(method_error)?;
            }
            if matches!(spec.flow, crate::input::formats::FlowKind::Return) {
                let expected = match return_shape {
                    TypeShape::Void => 0x0e,
                    TypeShape::Class | TypeShape::Array => 0x11,
                    other if other.words() == 2 => 0x10,
                    _ => 0x0f,
                };
                if spec.opcode != expected {
                    return Err(MethodError::Malformed {
                        pc,
                        problem: MethodProblem::InvalidResultKind,
                    });
                }
            }
        }
        Ok(facts)
    }
}

fn check_reference_coverage(
    items: &[super::DecodedItem],
    facts: &[ReferenceFactEntry<'_>],
) -> Result<(), MethodError> {
    let mut index = 0usize;
    for item in items {
        let super::DecodedItem::Instruction(instruction) = item else {
            continue;
        };
        for reference in instruction.references() {
            let Some(fact) = facts.get(index) else {
                return Err(MethodError::InternalInvariant(
                    "P1 reference facts lost an instruction reference",
                ));
            };
            if fact.pc != instruction.pc() || fact.slot != reference.slot {
                return Err(MethodError::InternalInvariant(
                    "P1 reference facts are not aligned with their instruction sites",
                ));
            }
            index += 1;
        }
    }
    if index != facts.len() {
        return Err(MethodError::InternalInvariant(
            "P1 reference facts contain a foreign site",
        ));
    }
    Ok(())
}

fn build_entry_layout(
    source: &MethodSource<'_, '_>,
    code: &crate::input::RawMethodView<'_>,
) -> Result<EntryLayout, MethodError> {
    let declaration = source.declaration();
    if declaration.parameter_types.len() != declaration.parameter_type_ids.len() {
        return Err(MethodError::InternalInvariant(
            "entry parameter type facts disagree",
        ));
    }
    let start = u32::from(code.registers_size())
        .checked_sub(u32::from(code.ins_size()))
        .ok_or(MethodError::InternalInvariant(
            "checked code has ins_size above registers_size",
        ))?;
    let mut cursor = start;
    let this = if source.access_flags() & STATIC == 0 {
        let slot = RegisterSlot::new(RegisterId(cursor), SlotWidth::One);
        cursor = cursor.checked_add(1).ok_or(MethodError::PlatformCapacity)?;
        Some(slot)
    } else {
        None
    };
    let mut parameters = Vec::new();
    for (index, type_id) in declaration.parameter_type_ids.iter().enumerate() {
        let descriptor =
            declaration
                .parameter_types
                .get(index)
                .ok_or(MethodError::InternalInvariant(
                    "entry parameter descriptor is missing",
                ))?;
        let width = match descriptor.units().first() {
            Some(0x4a | 0x44) => SlotWidth::Two,
            Some(0x4c | 0x5b) => SlotWidth::One,
            Some(0x5a | 0x42 | 0x53 | 0x43 | 0x49 | 0x46) => SlotWidth::One,
            _ => return Err(bad_entry()),
        };
        let slot = RegisterSlot::new(RegisterId(cursor), width);
        cursor = cursor
            .checked_add(width.words())
            .ok_or(MethodError::PlatformCapacity)?;
        push(&mut parameters, EntryParameter::new(TypeId(*type_id), slot)).map_err(method_error)?;
    }
    if cursor != u32::from(code.registers_size()) {
        return Err(MethodError::InternalInvariant(
            "entry layout does not cover the checked incoming window",
        ));
    }
    Ok(EntryLayout::new(this, parameters, RegisterId(start)))
}

fn bad_entry() -> MethodError {
    MethodError::Malformed {
        pc: CodeUnitOffset(0),
        problem: MethodProblem::InvalidEntryLayout,
    }
}

fn bad_type(pc: CodeUnitOffset) -> MethodError {
    MethodError::Malformed {
        pc,
        problem: MethodProblem::InvalidTypeOperand,
    }
}

fn method_error(error: RequestError) -> MethodError {
    match error {
        RequestError::Input(error) => MethodError::Input(error),
        RequestError::Method(error) => error,

        RequestError::Allocation(error) => MethodError::Allocation(error),
        RequestError::PlatformCapacity => MethodError::PlatformCapacity,
        RequestError::InternalInvariant(message) => MethodError::InternalInvariant(message),
        _ => MethodError::InternalInvariant("P1 returned a foreign error"),
    }
}

fn outcome<'a>(
    prefix: Vec<crate::p1::DecodedItem>,
    error: MethodError,
) -> Result<DecodedOutcome<'a>, MethodError> {
    if matches!(
        error.kind(),
        crate::FailureKind::ResourceExhausted | crate::FailureKind::InternalInvariant
    ) {
        return Err(error);
    }
    Ok(DecodedOutcome {
        decoded: None,
        prefix,
        error: Some(error),
    })
}
