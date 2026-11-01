use crate::input::MethodInput;
use crate::{DeclarationCatalog, MethodBudget, RequestError};

pub(crate) use crate::p2_flow::normal::{
    Arithmetic, BlockId, CallId, DeclarationType, EdgeId, Op, ResultType, Terminal,
};
pub(crate) use crate::p3_ssa::normal::{BoundOp, Definition, Sym, ValueId};
pub(crate) use crate::p5_types::normal::{Kind, TypedBody};

pub(crate) fn build<'a>(
    input: &MethodInput<'_, 'a>,
    catalog: &DeclarationCatalog,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<Option<crate::p6_variables::normal::StorageBody<'a>>, RequestError> {
    let Some(flow) = crate::p2_flow::normal::decode(input, catalog, budget)? else {
        return Ok(None);
    };
    let ssa = crate::p3_ssa::normal::build(flow, budget)?;
    let Some(typed) = crate::p5_types::normal::solve(ssa, budget)? else {
        return Ok(None);
    };
    crate::p6_variables::normal::allocate(typed, budget)
}
