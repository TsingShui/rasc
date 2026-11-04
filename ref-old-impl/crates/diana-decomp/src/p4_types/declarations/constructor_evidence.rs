use super::allocation::{ConstructorContract, ConstructorUse};
use crate::context::Context;
use crate::input::MethodSource;
use crate::substrate::owned::copy_units;
use crate::{CodeUnitOffset, MethodId, RequestError, RunContext};

enum Candidate {
    Declared(ConstructorContract),
    Missing,
    Unavailable,
}
pub(crate) struct ConstructorDeclarations {
    allocation: Candidate,
    chain: Option<(ConstructorUse, Candidate)>,
}
fn candidate(result: Result<ConstructorContract, RequestError>) -> Result<Candidate, RequestError> {
    match result {
        Ok(contract) => Ok(Candidate::Declared(contract)),
        Err(RequestError::BindingEvidenceMissing { .. }) => Ok(Candidate::Missing),
        Err(RequestError::InitializationUnavailable { .. }) => Ok(Candidate::Unavailable),
        Err(error) => Err(error),
    }
}
pub(crate) fn collect(
    input: &MethodSource<'_, '_>,
    catalog: &Context,
    invocation: &super::throws::Invocation,
    budget: &mut RunContext<'_>,
) -> Result<ConstructorDeclarations, RequestError> {
    let reference = input.method_reference(invocation.reference, budget)?;
    let owner = copy_units(reference.owner, budget)?;
    let allocation = candidate(super::allocation::resolve(
        input,
        catalog,
        invocation,
        &owner,
        ConstructorUse::Allocation,
        budget,
    ))?;
    let chain = if crate::substrate::flags::instance_constructor(input.access_flags()) {
        let caller = copy_units(input.owner(), budget)?;
        let kind = if crate::context::same_units(&owner, &caller, budget)? {
            ConstructorUse::This
        } else {
            ConstructorUse::Super
        };
        let result = super::allocation::resolve(input, catalog, invocation, &owner, kind, budget);
        Some((kind, candidate(result)?))
    } else {
        None
    };
    Ok(ConstructorDeclarations { allocation, chain })
}
impl ConstructorDeclarations {
    pub(crate) fn bind(
        &self,
        reference: MethodId,
        pc: CodeUnitOffset,
        expected_owner: &[u16],
        kind: ConstructorUse,
        budget: &mut RunContext<'_>,
    ) -> Result<ConstructorContract, RequestError> {
        let candidate = if kind == ConstructorUse::Allocation {
            &self.allocation
        } else {
            let Some((expected, candidate)) = &self.chain else {
                return Err(RequestError::InitializationUnavailable { pc });
            };
            if *expected != kind {
                return Err(RequestError::InitializationUnavailable { pc });
            }
            candidate
        };
        match candidate {
            Candidate::Missing => Err(RequestError::BindingEvidenceMissing { reference, pc }),
            Candidate::Unavailable => Err(RequestError::InitializationUnavailable { pc }),
            Candidate::Declared(contract) => {
                if contract.reference != reference {
                    return Err(RequestError::InternalInvariant(
                        "constructor declaration changed original reference",
                    ));
                }
                if !crate::context::same_units(&contract.owner, expected_owner, budget)? {
                    return Err(RequestError::InitializationUnavailable { pc });
                }
                contract.retain(budget)
            }
        }
    }
}
