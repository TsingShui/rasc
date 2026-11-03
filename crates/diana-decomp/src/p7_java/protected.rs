use crate::context::Context;
use crate::input::{MethodInput, protection::CatchType};
use crate::p5_variables::StorageBody;
use crate::p6_regions::protection::{Node, Protection};
use crate::substrate::owned::{copy_units, push};
use crate::{MethodBudget, RequestError};

pub(crate) struct CatchName {
    pub kind: CatchType,
    pub source: String,
}
pub(crate) struct ProtectedBody {
    pub storage: super::dispatcher::Dispatcher,
    pub source: super::source_control::Plan,
}
pub(super) fn names(
    input: &MethodInput<'_, '_>,
    catalog: &Context,
    storage: &StorageBody<'_>,
    region: &Protection,
    calls: &super::calls::SourceCalls,
    uncertainties: &mut Vec<crate::OutputReason>,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<Option<Vec<CatchName>>, RequestError> {
    crate::p6_regions::protection::verify(storage, region, budget)?;
    let mut result: Vec<CatchName> = Vec::new();
    for node in &region.nodes {
        budget.charge_work(1)?;
        let Node::Try { catches, .. } = node else {
            continue;
        };
        let mut previous: Vec<Vec<u16>> = Vec::new();
        for catch in catches {
            budget.charge_work(1)?;
            for clause in &catch.clauses {
                budget.charge_work(1)?;
                let descriptor = match clause.kind {
                    CatchType::All => {
                        let mut descriptor = Vec::new();
                        for &unit in crate::p4_types::declarations::exceptions::THROWABLE {
                            budget.charge_work(1)?;
                            push(&mut descriptor, unit, budget)?;
                        }
                        descriptor
                    }
                    CatchType::Typed(id) => copy_units(input.type_descriptor(id, budget)?, budget)?,
                };
                if !crate::p4_types::declarations::exceptions::catch_type(
                    input,
                    catalog,
                    &descriptor,
                    budget,
                )? {
                    super::method::note(uncertainties, "catch_type_evidence", None, budget)?;
                }
                if !crate::p4_types::declarations::throws::unrestricted_catch(
                    catalog,
                    &descriptor,
                    budget,
                )? {
                    match super::protected_throws::handler_bounds(
                        input,
                        catalog,
                        &storage.typed,
                        calls,
                        clause.handler,
                        budget,
                    )? {
                        super::protected_throws::Bounds::Known(bounds) if !bounds.is_empty() => {}
                        _ => {
                            super::method::note(uncertainties, "catch_reachability", None, budget)?
                        }
                    }
                }
                for earlier in &previous {
                    budget.charge_work(1)?;
                    if crate::p4_types::declarations::throws::subtype(
                        catalog,
                        &descriptor,
                        earlier,
                        budget,
                    )? != Some(false)
                    {
                        super::method::note(uncertainties, "catch_order_evidence", None, budget)?;
                    }
                }
                if catch.clauses.len() > 1 {
                    for other in &catch.clauses {
                        budget.charge_work(1)?;
                        if std::ptr::eq(other, clause) {
                            continue;
                        }
                        let CatchType::Typed(id) = other.kind else {
                            return Ok(None);
                        };
                        let name = copy_units(input.type_descriptor(id, budget)?, budget)?;
                        if crate::p4_types::declarations::throws::subtype(
                            catalog,
                            &descriptor,
                            &name,
                            budget,
                        )? != Some(false)
                        {
                            super::method::note(
                                uncertainties,
                                "multi_catch_type_evidence",
                                None,
                                budget,
                            )?;
                        }
                    }
                }
                let Some(source) = super::calls::type_name(&descriptor, budget)? else {
                    return Ok(None);
                };
                push(
                    &mut result,
                    CatchName {
                        kind: clause.kind,
                        source,
                    },
                    budget,
                )?;
                push(&mut previous, descriptor, budget)?;
            }
        }
    }
    Ok(Some(result))
}
