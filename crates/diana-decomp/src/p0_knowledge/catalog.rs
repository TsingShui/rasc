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

/// How a consumer treats a reference that has no matching declaration entry.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum EvidencePolicy {
    /// Missing declaration evidence declines the affected Java capability.
    #[default]
    Declared,
    /// The original DEX reference carries the call's owner, name and prototype,
    /// so it binds the call on its own. Access flags, hierarchy, generic
    /// signatures and throws remain unknown and are never asserted.
    TrustedReferences,
}

/// Immutable declaration evidence for one caller environment. Signatures alone
/// remain partial; class directories carry explicit coverage. An absent entry
/// means evidence is missing, never that a method cannot exist.
pub struct DeclarationCatalog {
    pub(crate) environment: EnvironmentId,
    policy: EvidencePolicy,
    methods: Vec<ExternalMethodSignature>,
    classes: Vec<super::ExternalClassDeclaration>,
    fields: Vec<super::ExternalFieldEvidence>,
}

impl DeclarationCatalog {
    pub fn empty(environment: EnvironmentId) -> Self {
        Self {
            environment,
            policy: EvidencePolicy::default(),
            methods: Vec::new(),
            classes: Vec::new(),
            fields: Vec::new(),
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
            policy: EvidencePolicy::default(),
            methods,
            classes: Vec::new(),
            fields: Vec::new(),
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
            policy: EvidencePolicy::default(),
            methods,
            classes,
            fields: Vec::new(),
        }
    }
    /// Selects how an absent declaration entry is treated. The default keeps
    /// every Java capability gated on declared evidence.
    pub fn with_policy(mut self, policy: EvidencePolicy) -> Self {
        self.policy = policy;
        self
    }

    pub(crate) fn policy(&self) -> EvidencePolicy {
        self.policy
    }

    /// Takes caller-owned exact field facts. Missing/duplicate entries remain unproven and
    /// consumers still validate the original identity and declaration scope.
    pub fn with_field_evidence(mut self, fields: Vec<super::ExternalFieldEvidence>) -> Self {
        self.fields = fields;
        self
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
