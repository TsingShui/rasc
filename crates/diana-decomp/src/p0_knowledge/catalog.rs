use crate::EnvironmentId;
use crate::input::{DexString, InputError};
use crate::pipeline::control::InputControl;

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

/// Immutable declaration evidence for one caller environment. Signatures alone
/// remain partial; class directories carry explicit coverage. An absent entry
/// means evidence is missing, never that a method cannot exist.
pub struct DeclarationCatalog {
    pub(crate) environment: EnvironmentId,
    classes: Vec<super::ExternalClassDeclaration>,
    fields: Vec<super::ExternalFieldEvidence>,
}

impl DeclarationCatalog {
    pub fn empty(environment: EnvironmentId) -> Self {
        Self {
            environment,
            classes: Vec::new(),
            fields: Vec::new(),
        }
    }

    /// Takes caller-owned directories with explicit per-class coverage. Native
    /// consumers charge candidate scans and reject unknown/inconsistent facts;
    /// construction does not certify source binding or catalog completeness.
    pub(crate) fn with_declarations(
        environment: EnvironmentId,
        classes: Vec<super::ExternalClassDeclaration>,
    ) -> Self {
        Self {
            environment,
            classes,
            fields: Vec::new(),
        }
    }
    /// Appends facts derived from another input image of the same request.
    pub(crate) fn extend(&mut self, other: Self) {
        let Self {
            classes, fields, ..
        } = other;
        self.classes.extend(classes);
        self.fields.extend(fields);
    }

    pub(crate) fn field<'c>(
        &'c self,
        owner: DexString<'_>,
        name: DexString<'_>,
        field_type: DexString<'_>,
        budget: &mut impl InputControl,
    ) -> Result<Option<&'c super::ExternalFieldEvidence>, InputError> {
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
    pub(crate) fn class<'c>(
        &'c self,
        descriptor: &[u16],
        budget: &mut impl InputControl,
    ) -> Result<Option<&'c super::ExternalClassDeclaration>, InputError> {
        let mut found = None;
        for class in &self.classes {
            budget.charge_work(1)?;
            if same_units(&class.descriptor, descriptor, budget)? {
                if found.is_some() {
                    return Ok(None);
                }
                found = Some(class);
            }
        }
        Ok(found)
    }
}
