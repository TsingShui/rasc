mod branch;
pub(crate) use branch::{Arm, BodyBranch};
mod graph;
mod integer_switch;
pub(crate) use integer_switch::{SwitchForm, choose as switch_form};
mod normal;
#[cfg(test)]
mod tests;
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
