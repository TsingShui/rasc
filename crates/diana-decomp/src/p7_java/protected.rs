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
    uncertainties: &mut Vec<crate::OutputReason>,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<Option<Vec<CatchName>>, RequestError> {
    crate::p6_regions::protection::verify(storage, region, budget)?;
    let mut result = Vec::new();
    for node in &region.nodes {
        budget.charge_work(1)?;
        let Node::Try { catches, .. } = node else {
            continue;
        };
        for catch in catches {
            budget.charge_work(1)?;
            for clause in &catch.clauses {
                budget.charge_work(1)?;
                let descriptor = match clause.kind {
                    CatchType::All => {
                        let mut descriptor = Vec::new();
                        for &unit in crate::p4_types::declarations::exceptions::THROWABLE {
                            push(&mut descriptor, unit, budget)?;
                        }
                        descriptor
                    }
                    CatchType::Typed(id) => copy_units(input.type_descriptor(id, budget)?, budget)?,
                };
                if matches!(clause.kind, CatchType::Typed(_))
                    && catalog.class(&descriptor, budget)?.is_none()
                {
                    super::method::note(uncertainties, "catch_declaration_unknown", None, budget)?;
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
            }
        }
    }
    Ok(Some(result))
}
