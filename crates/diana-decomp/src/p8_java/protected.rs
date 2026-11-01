use crate::input::{MethodInput, protection::CatchType};
use crate::p6_variables::normal::StorageBody;
use crate::p7_regions::protection::{Node, Protection};
use crate::substrate::owned::{copy_units, push};
use crate::{DeclarationCatalog, MethodBudget, RequestError};

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
    catalog: &DeclarationCatalog,
    storage: &StorageBody<'_>,
    region: &Protection,
    calls: &super::calls::SourceCalls,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<Option<Vec<CatchName>>, RequestError> {
    crate::p7_regions::protection::verify(storage, region, budget)?;
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
                        for &unit in crate::p4_calls::exceptions::THROWABLE {
                            budget.charge_work(1)?;
                            push(&mut descriptor, unit, budget)?;
                        }
                        descriptor
                    }
                    CatchType::Typed(id) => copy_units(input.type_descriptor(id, budget)?, budget)?,
                };
                if !crate::p4_calls::exceptions::catch_type(input, catalog, &descriptor, budget)? {
                    return Ok(None);
                }
                if !crate::p4_calls::throws::unrestricted_catch(catalog, &descriptor, budget)? {
                    let super::protected_throws::Bounds::Known(bounds) =
                        super::protected_throws::handler_bounds(
                            input,
                            catalog,
                            &storage.typed,
                            calls,
                            clause.handler,
                            budget,
                        )?
                    else {
                        return Ok(None);
                    };
                    if bounds.is_empty() {
                        return Ok(None);
                    }
                }
                for earlier in &previous {
                    budget.charge_work(1)?;
                    if crate::p4_calls::throws::subtype(catalog, &descriptor, earlier, budget)?
                        != Some(false)
                    {
                        return Ok(None);
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
                        if crate::p4_calls::throws::subtype(catalog, &descriptor, &name, budget)?
                            != Some(false)
                        {
                            return Ok(None);
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
