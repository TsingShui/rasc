use crate::input::MethodInput;
use crate::{DeclarationCatalog, MethodBudget, RequestError};

pub(crate) use crate::p2_flow::normal::{
    Arithmetic, BlockId, CallId, DeclarationType, EdgeId, Op, ResultType, Terminal,
};
pub(crate) use crate::p3_ssa::normal::{BoundOp, Definition, Sym};
pub(crate) use crate::p5_types::normal::{Kind, TypedBody};

pub(crate) struct Analysis<'a> {
    pub storage: Option<crate::p6_variables::normal::StorageBody<'a>>,
    pub declaration_failure: Option<crate::OutputReason>,
}

pub(crate) fn build<'a>(
    input: &MethodInput<'_, 'a>,
    catalog: &DeclarationCatalog,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<Analysis<'a>, RequestError> {
    let Some(flow) = crate::p2_flow::normal::decode(input, budget)? else {
        return Ok(Analysis {
            storage: None,
            declaration_failure: None,
        });
    };
    let ssa = crate::p3_ssa::normal::build(flow, budget)?;
    let resolved = crate::p4_calls::normal::collect(input, catalog, ssa, budget)?;
    let declaration_failure = resolved.decline_reason;
    let Some(typed) = crate::p5_types::normal::solve(input, resolved, budget)? else {
        return Ok(Analysis {
            storage: None,
            declaration_failure,
        });
    };
    Ok(Analysis {
        storage: crate::p6_variables::normal::allocate(typed, budget)?,
        declaration_failure,
    })
}
