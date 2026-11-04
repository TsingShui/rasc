use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use crate::input::{DexString, InputError};
use crate::pipeline::control::InputControl;
use crate::{ByteOffset, ClassDefId};

pub(crate) fn same_units(
    a: &[u16],
    b: &[u16],
    budget: &mut impl InputControl,
) -> Result<bool, InputError> {
    budget.checkpoint()?;
    if a.len() != b.len() {
        return Ok(false);
    }
    for (&a, &b) in a.iter().zip(b) {
        budget.checkpoint()?;
        if a != b {
            return Ok(false);
        }
    }
    Ok(true)
}

struct DeclarationLocation<'a> {
    bytes: &'a [u8],
    header_offset: ByteOffset,
    class_def: ClassDefId,
    build: Mutex<()>,
    full: OnceLock<Option<crate::input::InputClassDeclaration>>,
}

/// Declaration facts derived from the analysed input. Construction scans the
/// class table only; member tables and annotations are materialized per class
/// on first query and cached.
pub struct Context<'a> {
    entries: Vec<DeclarationLocation<'a>>,
    index: HashMap<Vec<u16>, usize>,
    ambiguous: std::collections::HashSet<Vec<u16>>,
}

impl<'a> Context<'a> {
    /// Appends the entries of another image of the same request.
    pub(crate) fn extend(&mut self, other: Self) {
        let Context {
            entries,
            index,
            ambiguous,
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
        self.entries.push(DeclarationLocation {
            bytes,
            header_offset,
            class_def,
            full: OnceLock::new(),
            build: Mutex::new(()),
        });
    }

    pub(crate) fn field(
        &self,
        owner: DexString<'_>,
        name: DexString<'_>,
        field_type: DexString<'_>,
        budget: &mut impl InputControl,
    ) -> Result<Option<&crate::input::InputFieldEvidence>, InputError> {
        let owner_units = crate::input::declaration_facts::units(owner, budget)?;
        let Some(class) = self.class(&owner_units, budget)? else {
            return Ok(None);
        };
        if !class.issues.is_empty() {
            return Ok(None);
        }
        let mut found = None;
        for field in &class.fields {
            budget.checkpoint()?;
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
    ) -> Result<Option<&crate::input::InputClassDeclaration>, InputError> {
        budget.checkpoint()?;
        let key: Vec<u16> = descriptor.to_vec();
        if self.ambiguous.contains(&key) {
            return Ok(None);
        }
        let Some(&position) = self.index.get(&key) else {
            return Ok(None);
        };
        let entry = &self.entries[position];
        if entry.full.get().is_none() {
            let _lock = entry.build.lock().map_err(|_| {
                InputError::InternalInvariant("declaration cache construction poisoned")
            })?;
            if let Some(full) = entry.full.get() {
                return Ok(full.as_ref());
            }
            let materialized = crate::input::declaration_facts::materialize(
                entry.bytes,
                entry.header_offset,
                entry.class_def,
                budget,
            )?;
            let _ = entry.full.set(materialized);
        }
        Ok(entry.full.get().and_then(Option::as_ref))
    }
}
