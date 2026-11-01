use super::StaticContract;
use crate::input::MethodInput;
use crate::p2_flow::normal::CallId;
use crate::p3_ssa::normal::SsaBody;
use crate::substrate::owned::push;
use crate::{DeclarationCatalog, MethodBudget, RequestError};

pub(crate) enum CallEvidence {
    Declaration(StaticContract),
    Missing,
}
pub(crate) struct ResolvedBody<'a> {
    _sealed: (),
    pub ssa: SsaBody<'a>,
    pub calls: Vec<CallEvidence>,
}
pub(crate) fn collect<'a>(
    input: &MethodInput<'_, '_>,
    catalog: &DeclarationCatalog,
    ssa: SsaBody<'a>,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<ResolvedBody<'a>, RequestError> {
    let mut calls = Vec::new();
    for call in &ssa.calls {
        budget.charge_work(1)?;
        let evidence =
            match super::contracts::collect_static(input, catalog, call.reference, budget)? {
                Some(contract) => CallEvidence::Declaration(contract),
                None => CallEvidence::Missing,
            };
        push(&mut calls, evidence, budget)?;
    }
    Ok(ResolvedBody {
        _sealed: (),
        ssa,
        calls,
    })
}
pub(crate) fn binding_available(
    input: &MethodInput<'_, '_>,
    catalog: &DeclarationCatalog,
    contract: &StaticContract,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<bool, RequestError> {
    super::contracts::binding_available(input, catalog, contract, budget)
}
pub(crate) fn contract<'a>(
    ssa: &SsaBody<'_>,
    calls: &'a [CallEvidence],
    id: CallId,
) -> Result<&'a StaticContract, RequestError> {
    let index = usize::try_from(id.0).map_err(|_| RequestError::PlatformCapacity)?;
    match calls.get(index) {
        Some(CallEvidence::Declaration(contract)) => Ok(contract),
        Some(CallEvidence::Missing) => {
            let site = ssa.calls.get(index).ok_or(RequestError::InternalInvariant(
                "missing evidence lost call identity",
            ))?;
            Err(RequestError::BindingEvidenceMissing {
                reference: site.reference,
                pc: site.pc,
            })
        }
        None => Err(RequestError::InternalInvariant(
            "foreign call evidence identity",
        )),
    }
}
