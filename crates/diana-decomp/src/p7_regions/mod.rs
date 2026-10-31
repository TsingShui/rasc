mod graph;
mod normal;
mod verify;
pub(crate) use verify::NaturalLoop;
pub(crate) fn build(
    body: &crate::p8_java::dispatcher::Dispatcher,
    budget: &mut crate::MethodBudget<'_, '_>,
) -> Result<Option<NaturalLoop>, crate::RequestError> {
    let Some(plan) = normal::plan(body, budget)? else {
        return Ok(None);
    };
    Ok(Some(verify::verify(body, plan, budget)?))
}
