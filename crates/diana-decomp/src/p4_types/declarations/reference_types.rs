use super::contracts::{clear_scope, directory};
use crate::context::Context;
use crate::input::MethodInput;
use crate::substrate::owned::copy_units;
use crate::{MethodBudget, RequestError};

pub(crate) fn source_type(
    input: &MethodInput<'_, '_>,
    catalog: &Context,
    descriptor: &[u16],
    budget: &mut MethodBudget<'_, '_>,
) -> Result<Option<bool>, RequestError> {
    named_type(input, catalog, descriptor, false, budget)
}
pub(crate) fn source_reference_type(
    input: &MethodInput<'_, '_>,
    catalog: &Context,
    descriptor: &[u16],
    budget: &mut MethodBudget<'_, '_>,
) -> Result<Option<bool>, RequestError> {
    named_type(input, catalog, descriptor, true, budget)
}
fn named_type(
    input: &MethodInput<'_, '_>,
    catalog: &Context,
    descriptor: &[u16],
    allow_interface: bool,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<Option<bool>, RequestError> {
    if descriptor.len() < 3 || descriptor.first() != Some(&76) || descriptor.last() != Some(&59) {
        return Ok(Some(false));
    }
    let caller = copy_units(input.owner(), budget)?;
    let Some(class) = catalog.class(descriptor, budget)? else {
        return Ok(None);
    };
    // Lexical scope is only relevant for nested types; ordinary descriptors
    // skip the ownership scan entirely.
    let mut lexical = false;
    if descriptor.contains(&u16::from(b'$')) {
        let nested =
            super::nested_scope::collect(input.program(), input.class_id(), catalog, budget)?;
        lexical = nested
            .as_ref()
            .is_some_and(|scope| scope.owner() == descriptor || scope.outer() == descriptor);
    }
    if descriptor == caller && class.enclosing_scope {
        let ownership = crate::input::ownership::collect(input.program(), budget)?;
        for entry in ownership.nested() {
            budget.charge_work(1)?;
            lexical |= entry.enclosing() == input.class_id();
        }
    }
    if !lexical && !directory(class, &caller, budget)?
        || class.access_flags & 1 == 0
        || class.access_flags & 0x2000 != 0
        || !allow_interface && class.access_flags & 0x200 != 0
    {
        return Ok(Some(false));
    }
    let mut end = descriptor.len() - 1;
    for (i, &unit) in descriptor[1..descriptor.len() - 1].iter().enumerate() {
        budget.charge_work(1)?;
        if unit == 47 {
            end = i + 1;
            break;
        }
    }
    Ok(Some(
        !input.qualifier_shadowed(&descriptor[1..end], budget)?
            && clear_scope(catalog, &caller, &descriptor[1..end], budget)?,
    ))
}
pub(crate) fn source_subtype(
    input: &MethodInput<'_, '_>,
    catalog: &Context,
    from: &[u16],
    to: &[u16],
    budget: &mut MethodBudget<'_, '_>,
) -> Result<Option<bool>, RequestError> {
    if source_reference_type(input, catalog, from, budget)? != Some(true) {
        return Ok(None);
    }
    super::hierarchy::subtype(catalog, from, to, budget)
}
