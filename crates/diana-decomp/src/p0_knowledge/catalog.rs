use std::cell::OnceCell;
use std::collections::HashMap;

use crate::input::catalog::{MemberEntry, MemberSelection};
use crate::input::{DexString, InputError};
use crate::pipeline::control::InputControl;
use crate::{ByteOffset, ClassDefId};

fn units(value: DexString<'_>, budget: &mut impl InputControl) -> Result<Vec<u16>, InputError> {
    let mut reader = value.reader()?;
    let mut result = Vec::new();
    while let Some(unit) = reader.next(budget)? {
        result.try_reserve(1).map_err(|_| InputError::Allocation {
            offset_byte: ByteOffset(0),
        })?;
        result.push(unit);
    }
    Ok(result)
}

fn push_unit(
    result: &mut Vec<u16>,
    unit: u16,
    budget: &mut impl InputControl,
) -> Result<(), InputError> {
    budget.charge_work(1)?;
    result.try_reserve(1).map_err(|_| InputError::Allocation {
        offset_byte: ByteOffset(0),
    })?;
    result.push(unit);
    Ok(())
}

pub(crate) fn same_units(
    a: &[u16],
    b: &[u16],
    budget: &mut impl InputControl,
) -> Result<bool, InputError> {
    budget.charge_work(1)?;
    if a.len() != b.len() {
        return Ok(false);
    }
    for (&a, &b) in a.iter().zip(b) {
        budget.charge_work(1)?;
        if a != b {
            return Ok(false);
        }
    }
    Ok(true)
}

struct Entry<'a> {
    bytes: &'a [u8],
    header_offset: ByteOffset,
    class_def: ClassDefId,
    full: OnceCell<Option<super::ExternalClassDeclaration>>,
}

/// Declaration facts derived from the analysed input. Construction scans the
/// class table only; member tables and annotations are materialized per class
/// on first query and cached.
pub struct DeclarationCatalog<'a> {
    entries: Vec<Entry<'a>>,
    index: HashMap<Vec<u16>, usize>,
    ambiguous: std::collections::HashSet<Vec<u16>>,
    fields: Vec<super::ExternalFieldEvidence>,
}

impl<'a> DeclarationCatalog<'a> {
    /// Appends the entries of another image of the same request.
    /// Appends the entries of another image of the same request.
    pub(crate) fn extend(&mut self, other: Self) {
        let DeclarationCatalog {
            entries,
            index,
            ambiguous,
            fields: _,
        } = other;
        let offset = self.entries.len();
        for (descriptor, position) in index {
            if self
                .index
                .insert(descriptor.clone(), position + offset)
                .is_some()
            {
                self.ambiguous.insert(descriptor.clone());
            }
            let _ = position;
        }
        for descriptor in ambiguous {
            self.ambiguous.insert(descriptor);
        }
        self.entries.extend(entries);
    }

    pub(crate) fn empty() -> Self {
        Self {
            entries: Vec::new(),
            index: HashMap::new(),
            ambiguous: std::collections::HashSet::new(),
            fields: Vec::new(),
        }
    }

    pub(crate) fn push(
        &mut self,
        descriptor: Vec<u16>,
        bytes: &'a [u8],
        header_offset: ByteOffset,
        class_def: ClassDefId,
    ) {
        let position = self.entries.len();
        if let Some(previous) = self.index.insert(descriptor.clone(), position) {
            let _ = previous;
            self.ambiguous.insert(descriptor.clone());
        }
        self.entries.push(Entry {
            bytes,
            header_offset,
            class_def,
            full: OnceCell::new(),
        });
    }

    pub(crate) fn field(
        &self,
        owner: DexString<'_>,
        name: DexString<'_>,
        field_type: DexString<'_>,
        budget: &mut impl InputControl,
    ) -> Result<Option<&super::ExternalFieldEvidence>, InputError> {
        let mut found = None;
        for field in &self.fields {
            budget.charge_work(1)?;
            if owner.matches_with(&field.owner, budget)?
                && name.matches_with(&field.name, budget)?
                && field_type.matches_with(&field.field_type, budget)?
            {
                if found.is_some() {
                    return Ok(None);
                }
                found = Some(field);
            }
        }
        Ok(found)
    }

    /// Resolves one class by descriptor, materializing its declarations on the
    /// first successful query.
    pub fn class(
        &self,
        descriptor: &[u16],
        budget: &mut impl InputControl,
    ) -> Result<Option<&super::ExternalClassDeclaration>, InputError> {
        budget.charge_work(1)?;
        let key: Vec<u16> = descriptor.to_vec();
        if self.ambiguous.contains(&key) {
            return Ok(None);
        }
        let Some(&position) = self.index.get(&key) else {
            return Ok(None);
        };
        let entry = &self.entries[position];
        if entry.full.get().is_none() {
            let materialized = materialize(entry, budget)?;
            let _ = entry.full.set(materialized);
        }
        Ok(entry.full.get().and_then(Option::as_ref))
    }
}

fn materialize(
    entry: &Entry<'_>,
    budget: &mut impl InputControl,
) -> Result<Option<super::ExternalClassDeclaration>, InputError> {
    let container = crate::input::DexContainer::open_with(entry.bytes, budget)?;
    let index = crate::input::ProgramIndex::from_container(container, entry.header_offset, budget)?;
    let class = index.class_with(entry.class_def, budget)?;
    let descriptor = units(class.descriptor, budget)?;
    let access_flags = class.access_flags;
    let context = index.class_context(entry.class_def, budget)?;
    let parent = context
        .superclass?
        .map(|parent| units(parent, budget))
        .transpose()?;
    let mut interfaces = Vec::new();
    for word in context.interfaces?.as_chunks::<2>().0 {
        budget.charge_work(1)?;
        let id = crate::TypeId(u32::from(u16::from_le_bytes(*word)));
        interfaces.push(units(index.type_name(id, budget)?, budget)?);
    }
    let mut methods = Vec::new();
    class.visit_members_with::<_, InputError>(
        budget,
        MemberSelection::AllMethods,
        |entry, budget| {
            let MemberEntry::Method(method) = entry else {
                return Ok(());
            };
            let Ok(declaration) = method.declaration else {
                return Ok(());
            };
            let mut parameters = Vec::new();
            let _ = declaration.prototype.visit_parameter_type_ids(
                budget,
                |_, name, budget| -> Result<(), InputError> {
                    parameters.push(units(name, budget)?);
                    Ok(())
                },
            );
            let mut prototype = Vec::new();
            push_unit(&mut prototype, u16::from(b'('), budget)?;
            for parameter in parameters {
                for unit in parameter {
                    push_unit(&mut prototype, unit, budget)?;
                }
            }
            push_unit(&mut prototype, u16::from(b')'), budget)?;
            for unit in units(declaration.prototype.return_type, budget)? {
                push_unit(&mut prototype, unit, budget)?;
            }
            methods.push(super::ExternalMethodDeclaration {
                name: units(declaration.name, budget)?,
                prototype,
                access_flags: method.access_flags,
                throws: None,
                generic: false,
                signature: None,
            });
            Ok(())
        },
    )?;
    let mut field_names = Vec::new();
    class.visit_members_with::<_, InputError>(
        budget,
        MemberSelection::Fields,
        |entry, budget| {
            let MemberEntry::Field(field) = entry else {
                return Ok(());
            };
            if let Ok(declaration) = field.declaration {
                field_names.push(units(declaration.name, budget)?);
            }
            Ok(())
        },
    )?;
    Ok(Some(super::ExternalClassDeclaration {
        descriptor,
        access_flags,
        parent,
        interfaces,
        methods,
        field_names,
        coverage: super::DeclarationCoverage::AllDeclarations,
        generic: false,
        enclosing_scope: false,
        signature: None,
    }))
}
