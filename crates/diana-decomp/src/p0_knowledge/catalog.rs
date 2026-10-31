use crate::EnvironmentId;
use crate::input::{DexString, InputError, Prototype};
use crate::pipeline::control::InputControl;

/// Caller-supplied exact symbol spelling. Matching a signature is not proof of
/// dispatch, access, throws, hierarchy or a complete overload candidate group.
#[derive(serde::Deserialize)]
pub struct ExternalMethodSignature {
    pub owner: Vec<u16>,
    pub name: Vec<u16>,
    pub prototype: Vec<u16>,
}

/// Immutable partial external signature evidence for one caller environment.
/// An absent entry means evidence is missing, never that a method cannot exist.
pub struct DeclarationCatalog {
    pub(crate) environment: EnvironmentId,
    methods: Vec<ExternalMethodSignature>,
}

impl DeclarationCatalog {
    pub fn empty(environment: EnvironmentId) -> Self {
        Self {
            environment,
            methods: Vec::new(),
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
        }
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
        Ok(false)
    }
}
