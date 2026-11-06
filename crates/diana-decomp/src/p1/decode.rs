use crate::input::{MethodSource, ProgramIndex, RawPrintable, strings::TypeShape};
use crate::raw::OperandReference;
use crate::substrate::flags::STATIC;
use crate::substrate::owned::push;
use crate::substrate::slots::{RegisterSlot, SlotWidth};
use crate::{CodeUnitOffset, RegisterId, RequestError, TypeId};

use super::disassembly::DecodedFacts;
use super::reference::{
    BootstrapDeclaration, BootstrapReferenceId, BootstrapToken, BootstrapValue,
    CheckedBootstrapFact, CheckedMemberFact, CheckedMethodHandleFact, CheckedPrototype,
    CheckedReferenceFact, ReferenceFactEntry, type_form,
};
use super::{DecodedOutcome, EntryLayout, EntryParameter, MethodError, MethodProblem};

struct ReferenceReads<'a> {
    facts: Vec<ReferenceFactEntry<'a>>,
    issues: Vec<(CodeUnitOffset, MethodError)>,
}

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
            Err((prefix, error)) => return outcome(code, prefix, error),
        };
        let references = match self.read_reference_facts(&items, return_shape) {
            Ok(facts) => facts,
            Err(error) => return outcome(code, items, error),
        };
        let layout = match self.check_method_layout(code, &items, &references.facts) {
            Ok(layout) => layout,
            Err(error) => return outcome(code, items, error),
        };
        #[cfg(test)]
        if let Err(error) = check_reference_coverage(&items, &references.facts) {
            return outcome(code, items, error);
        }
        let protection = match crate::input::protection::read(&layout).map_err(method_error) {
            Ok(protection) => protection,
            Err(error) => return outcome(code, items, error),
        };
        let entry = match build_entry_layout(source, code) {
            Ok(entry) => entry,
            Err(error) => return outcome(code, items, error),
        };
        let declaration = match source.declaration().try_copy().map_err(method_error) {
            Ok(declaration) => declaration,
            Err(error) => return outcome(code, items, error),
        };
        Ok(DecodedOutcome {
            decoded: Some(crate::input::ProgramIndex::assemble_decoded(
                code,
                layout,
                protection,
                items,
                DecodedFacts {
                    owner: source.owner_context().clone(),
                    declaration,
                    access_flags: source.access_flags(),
                    entry,
                    annotations: source.annotations(),
                    metadata_issue: source.metadata().is_err(),
                    original_metadata: source.original_metadata().clone(),
                    metadata_error: source.metadata_error(),
                    reference_facts: references.facts,
                    issues: references.issues,
                },
            )),
            prefix: Vec::new(),
            error: None,
            source: None,
        })
    }

    fn read_reference_facts(
        &self,
        items: &[super::DecodedItem],
        return_shape: TypeShape,
    ) -> Result<ReferenceReads<'a>, MethodError> {
        let mut facts = Vec::new();
        let mut issues = Vec::new();
        for item in items {
            let super::DecodedItem::Instruction(instruction) = item else {
                continue;
            };
            let pc = instruction.pc();
            for reference in instruction.references() {
                let fact = if self.reference_in_bounds(reference.target) {
                    self.resolve_instruction_reference(reference.target)
                } else {
                    Err(MethodError::ReferenceOutOfBounds {
                        pc,
                        reference: reference.target,
                    })
                };
                let use_issue = match &fact {
                    Ok(fact) => self
                        .check_reference_use(pc, instruction.opcode(), reference.target, fact)
                        .err(),
                    Err(_) => None,
                };
                push(
                    &mut facts,
                    ReferenceFactEntry {
                        pc,
                        slot: reference.slot,
                        fact,
                        use_issue,
                    },
                )
                .map_err(method_error)?;
            }
            if matches!(
                instruction.spec().flow,
                crate::input::formats::FlowKind::Return
            ) {
                let expected = match return_shape {
                    TypeShape::Void => 0x0e,
                    TypeShape::Class | TypeShape::Array => 0x11,
                    other if other.words() == 2 => 0x10,
                    _ => 0x0f,
                };
                if instruction.opcode() != expected {
                    push(
                        &mut issues,
                        (
                            pc,
                            MethodError::Malformed {
                                pc,
                                problem: MethodProblem::InvalidResultKind,
                            },
                        ),
                    )
                    .map_err(method_error)?;
                }
            }
        }
        Ok(ReferenceReads { facts, issues })
    }

    fn resolve_instruction_reference(
        &self,
        target: OperandReference,
    ) -> Result<CheckedReferenceFact<'a>, MethodError> {
        Ok(match target {
            OperandReference::String(id) => CheckedReferenceFact::String {
                value: self.string(id)?,
            },
            OperandReference::Type(id) => {
                let (descriptor, shape) = self.type_descriptor(id)?;
                CheckedReferenceFact::Type {
                    descriptor,
                    form: type_form(shape),
                    words: shape.words(),
                }
            }
            OperandReference::Field(id) => {
                let field = self.field_reference(id)?;
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
                CheckedReferenceFact::Method {
                    owner: method.owner,
                    owner_type: method.owner_type,
                    name: method.declaration.name,
                    prototype: CheckedPrototype::from_prototype(&method.declaration.prototype)?,
                }
            }
            OperandReference::Proto(id) => CheckedReferenceFact::Proto {
                prototype: CheckedPrototype::from_prototype(&self.prototype(id)?)?,
            },
            OperandReference::CallSite(id) => {
                let site = self.call_site(id)?;
                let mut declarations = Vec::new();
                push(
                    &mut declarations,
                    BootstrapDeclaration {
                        reference: site.handle.member,
                        range: site.range,
                        fact: CheckedBootstrapFact::Member(
                            self.checked_member(site.handle.member)?,
                        ),
                    },
                )
                .map_err(method_error)?;
                let mut bootstrap = Vec::new();
                for token in site.bootstrap {
                    push(
                        &mut bootstrap,
                        self.bootstrap_token(token, &mut declarations)?,
                    )
                    .map_err(method_error)?;
                }
                CheckedReferenceFact::CallSite {
                    name: site.name,
                    name_id: site.name_id,
                    handle_id: site.handle_id,
                    prototype_id: site.proto_id,
                    member: BootstrapReferenceId(0),
                    declarations,
                    bootstrap,
                    argument_count: site.count,
                    encoded: site.encoded,
                    range: site.range,
                    handle: CheckedMethodHandleFact::new(site.handle.kind, site.handle.member),
                    prototype: CheckedPrototype::from_prototype(&site.prototype)?,
                }
            }
            OperandReference::MethodHandle(id) => {
                let handle = self.method_handle(id)?;
                CheckedReferenceFact::MethodHandle {
                    handle: CheckedMethodHandleFact::new(handle.kind, handle.member),
                    member: self.checked_member(handle.member)?,
                }
            }
        })
    }

    fn checked_member(
        &self,
        target: OperandReference,
    ) -> Result<CheckedMemberFact<'a>, MethodError> {
        if !matches!(
            target,
            OperandReference::Field(_) | OperandReference::Method(_)
        ) {
            return Err(MethodError::InternalInvariant(
                "checked handle has no leaf member",
            ));
        }
        match self.resolve_instruction_reference(target)? {
            CheckedReferenceFact::Field {
                owner,
                owner_type,
                name,
                field_type,
                field_type_id,
            } => Ok(CheckedMemberFact::Field {
                owner,
                owner_type,
                name,
                field_type,
                field_type_id,
            }),
            CheckedReferenceFact::Method {
                owner,
                owner_type,
                name,
                prototype,
            } => Ok(CheckedMemberFact::Method {
                owner,
                owner_type,
                name,
                prototype,
            }),
            _ => Err(MethodError::InternalInvariant(
                "handle declaration changed kind",
            )),
        }
    }
    fn bootstrap_declaration(
        &self,
        reference: OperandReference,
    ) -> Result<CheckedBootstrapFact<'a>, MethodError> {
        if matches!(reference, OperandReference::CallSite(_)) {
            return Err(MethodError::InternalInvariant(
                "encoded bootstrap value names a call site",
            ));
        }
        Ok(match self.resolve_instruction_reference(reference)? {
            CheckedReferenceFact::String { value } => CheckedBootstrapFact::String { value },
            CheckedReferenceFact::Type {
                descriptor,
                form,
                words,
            } => CheckedBootstrapFact::Type {
                descriptor,
                form,
                words,
            },
            CheckedReferenceFact::Field {
                owner,
                owner_type,
                name,
                field_type,
                field_type_id,
            } => CheckedBootstrapFact::Member(CheckedMemberFact::Field {
                owner,
                owner_type,
                name,
                field_type,
                field_type_id,
            }),
            CheckedReferenceFact::Method {
                owner,
                owner_type,
                name,
                prototype,
            } => CheckedBootstrapFact::Member(CheckedMemberFact::Method {
                owner,
                owner_type,
                name,
                prototype,
            }),
            CheckedReferenceFact::Proto { prototype } => CheckedBootstrapFact::Proto { prototype },
            CheckedReferenceFact::MethodHandle { handle, member } => {
                CheckedBootstrapFact::MethodHandle { handle, member }
            }
            CheckedReferenceFact::CallSite { .. } => {
                return Err(MethodError::InternalInvariant(
                    "bootstrap leaf declaration changed kind",
                ));
            }
        })
    }
    fn bootstrap_token(
        &self,
        token: crate::input::values::ValueStep<'a>,
        declarations: &mut Vec<BootstrapDeclaration<'a>>,
    ) -> Result<BootstrapToken<'a>, MethodError> {
        use crate::input::values::{EncodedKind, ValueStep};
        let ValueStep::Value(item) = token else {
            return Ok(BootstrapToken::EndContainer);
        };
        let value = match item.kind {
            EncodedKind::Signed(v) => BootstrapValue::Signed(v),
            EncodedKind::Unsigned(v) => BootstrapValue::Unsigned(v),
            EncodedKind::Bits { width, bits } => BootstrapValue::Bits { width, bits },
            EncodedKind::Null => BootstrapValue::Null,
            EncodedKind::Boolean(v) => BootstrapValue::Boolean(v),
            EncodedKind::Reference(reference) => {
                let at = BootstrapReferenceId(
                    u32::try_from(declarations.len()).map_err(|_| MethodError::PlatformCapacity)?,
                );
                push(
                    declarations,
                    BootstrapDeclaration {
                        reference,
                        range: item.range,
                        fact: self.bootstrap_declaration(reference)?,
                    },
                )
                .map_err(method_error)?;
                BootstrapValue::Reference {
                    reference,
                    fact: at,
                }
            }
            EncodedKind::Array { count } => BootstrapValue::Array { count },
            EncodedKind::Annotation { type_id, count } => BootstrapValue::Annotation {
                type_id,
                descriptor: self.type_name(type_id)?,
                count,
            },
        };
        Ok(BootstrapToken::Value {
            range: item.range,
            name_id: item.name,
            name: item.name.map(|id| self.string(id)).transpose()?,
            tag: item.tag,
            value_arg: item.arg,
            value,
        })
    }

    fn check_reference_use(
        &self,
        pc: CodeUnitOffset,
        opcode: u8,
        target: OperandReference,
        fact: &CheckedReferenceFact<'_>,
    ) -> Result<(), MethodError> {
        match fact {
            CheckedReferenceFact::Type {
                descriptor, form, ..
            } => {
                use super::TypeForm;
                let valid = match opcode {
                    0x1f | 0x20 => matches!(form, TypeForm::Class | TypeForm::Array),
                    0x22 => matches!(form, TypeForm::Class),
                    0x23..=0x25 => matches!(form, TypeForm::Array),
                    _ => true,
                };
                if !valid {
                    return Err(bad_type(pc));
                }
                if matches!(opcode, 0x24 | 0x25) {
                    let mut units = descriptor.reader()?;
                    units.next()?;
                    if matches!(units.next()?, Some(0x4a | 0x44)) {
                        return Err(bad_type(pc));
                    }
                }
            }
            CheckedReferenceFact::Field { field_type, .. } if matches!(opcode, 0x52..=0x6d) => {
                let first = field_type.reader()?.next()?;
                let valid = match (opcode - 0x52) % 7 {
                    0 => matches!(first, Some(73 | 70)),
                    1 => matches!(first, Some(74 | 68)),
                    2 => matches!(first, Some(76 | 91)),
                    3 => first == Some(90),
                    4 => first == Some(66),
                    5 => first == Some(67),
                    6 => first == Some(83),
                    _ => false,
                };
                let OperandReference::Field(id) = target else {
                    return Err(MethodError::InternalInvariant(
                        "field fact has another reference domain",
                    ));
                };
                if !valid
                    || self
                        .local_field_flags(id)?
                        .is_some_and(|flags| (flags & 8 != 0) != matches!(opcode, 0x60..=0x6d))
                {
                    return Err(MethodError::Malformed {
                        pc,
                        problem: MethodProblem::InvalidFieldKind,
                    });
                }
            }
            CheckedReferenceFact::Method { .. } if matches!(opcode, 0x6e..=0x72 | 0x74..=0x78) => {
                let OperandReference::Method(id) = target else {
                    return Err(MethodError::InternalInvariant(
                        "method fact has another reference domain",
                    ));
                };
                if self.local_method(id)?.is_some_and(|method| {
                    (method.access_flags & STATIC != 0) != matches!(opcode, 0x71 | 0x77)
                }) {
                    return Err(MethodError::Malformed {
                        pc,
                        problem: MethodProblem::InvalidInvokeKind,
                    });
                }
            }
            _ => {}
        }
        Ok(())
    }
}

#[cfg(test)]
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
    source: &crate::input::RawMethodView<'a>,
    prefix: Vec<crate::p1::DecodedItem>,
    error: MethodError,
) -> Result<DecodedOutcome<'a>, MethodError> {
    Ok(DecodedOutcome {
        decoded: None,
        prefix,
        error: Some(error),
        source: Some(source.clone()),
    })
}
