use crate::EnvironmentId;
use crate::input::{DexString, InputError, Prototype};
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

/// Caller-supplied exact symbol spelling. Matching a signature is not proof of
/// dispatch, access, throws, hierarchy or a complete overload candidate group.
#[derive(serde::Deserialize)]
pub struct ExternalMethodSignature {
    pub owner: Vec<u16>,
    pub name: Vec<u16>,
    pub prototype: Vec<u16>,
}

/// Immutable declaration evidence for one caller environment. Signatures alone
/// remain partial; class directories carry explicit coverage. An absent entry
/// means evidence is missing, never that a method cannot exist.
pub struct DeclarationCatalog {
    pub(crate) environment: EnvironmentId,
    methods: Vec<ExternalMethodSignature>,
    classes: Vec<super::ExternalClassDeclaration>,
}

impl DeclarationCatalog {
    pub fn empty(environment: EnvironmentId) -> Self {
        Self {
            environment,
            methods: Vec::new(),
            classes: Vec::new(),
        }
    }

    /// Takes caller-owned partial signature facts without reading files or
    /// asserting completeness. Native loops charge the request that consumes
    /// them; this does not admit any executable Java by itself.
    pub fn partial_signatures(
        environment: EnvironmentId,
        methods: Vec<ExternalMethodSignature>,
    ) -> Self {
        Self {
            environment,
            methods,
            classes: Vec::new(),
        }
    }
    /// Takes caller-owned directories with explicit per-class coverage. Native
    /// consumers charge candidate scans and reject unknown/inconsistent facts;
    /// construction does not certify source binding or catalog completeness.
    pub fn with_declarations(
        environment: EnvironmentId,
        methods: Vec<ExternalMethodSignature>,
        classes: Vec<super::ExternalClassDeclaration>,
    ) -> Self {
        Self {
            environment,
            methods,
            classes,
        }
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

    pub(crate) fn contains_signature(
        &self,
        owner: DexString<'_>,
        name: DexString<'_>,
        prototype: &Prototype<'_, '_>,
        control: &mut impl InputControl,
    ) -> Result<bool, InputError> {
        for method in &self.methods {
            control.charge_work(1)?;
            if owner.matches_with(&method.owner, control)?
                && name.matches_with(&method.name, control)?
                && prototype.matches_with(&method.prototype, control)?
            {
                return Ok(true);
            }
        }
        for class in &self.classes {
            control.charge_work(1)?;
            if owner.matches_with(&class.descriptor, control)? {
                for method in &class.methods {
                    control.charge_work(1)?;
                    if name.matches_with(&method.name, control)?
                        && prototype.matches_with(&method.prototype, control)?
                    {
                        return Ok(true);
                    }
                }
            }
        }
        Ok(false)
    }
}
