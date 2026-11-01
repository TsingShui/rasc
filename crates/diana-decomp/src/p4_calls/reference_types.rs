use super::contracts::{clear_scope, plain};
use crate::input::MethodInput;
use crate::substrate::owned::{copy_units, reserve};
use crate::{DeclarationCatalog, MethodBudget, RequestError};

pub(crate) fn source_type(
    input: &MethodInput<'_, '_>,
    catalog: &DeclarationCatalog,
    descriptor: &[u16],
    budget: &mut MethodBudget<'_, '_>,
) -> Result<bool, RequestError> {
    named_type(input, catalog, descriptor, false, budget)
}
pub(crate) fn source_reference_type(
    input: &MethodInput<'_, '_>,
    catalog: &DeclarationCatalog,
    descriptor: &[u16],
    budget: &mut MethodBudget<'_, '_>,
) -> Result<bool, RequestError> {
    named_type(input, catalog, descriptor, true, budget)
}
fn named_type(
    input: &MethodInput<'_, '_>,
    catalog: &DeclarationCatalog,
    descriptor: &[u16],
    allow_interface: bool,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<bool, RequestError> {
    if descriptor.len() < 3 || descriptor.first() != Some(&76) || descriptor.last() != Some(&59) {
        return Ok(false);
    }
    let caller = copy_units(input.owner(), budget)?;
    let Some(class) = catalog.class(descriptor, budget)? else {
        return Ok(false);
    };
    if !plain(class, &caller, budget)?
        || class.access_flags & 1 == 0
        || class.access_flags & 0x2000 != 0
        || !allow_interface && class.access_flags & 0x200 != 0
    {
        return Ok(false);
    }
    let mut end = descriptor.len() - 1;
    for (i, &unit) in descriptor[1..descriptor.len() - 1].iter().enumerate() {
        budget.charge_work(1)?;
        if unit == 47 {
            end = i + 1;
            break;
        }
    }
    Ok(!input.qualifier_shadowed(&descriptor[1..end], budget)?
        && clear_scope(catalog, &caller, &descriptor[1..end], budget)?)
}
pub(crate) fn source_subtype(
    input: &MethodInput<'_, '_>,
    catalog: &DeclarationCatalog,
    from: &[u16],
    to: &[u16],
    budget: &mut MethodBudget<'_, '_>,
) -> Result<bool, RequestError> {
    let caller = copy_units(input.owner(), budget)?;
    let mut current = from;
    let mut seen: Vec<&[u16]> = Vec::new();
    loop {
        budget.charge_work(1)?;
        for old in &seen {
            budget.charge_work(1)?;
            if crate::p0_knowledge::same_units(old, current, budget)? {
                return Ok(false);
            }
        }
        let Some(class) = catalog.class(current, budget)? else {
            return Ok(false);
        };
        if !plain(class, &caller, budget)? || class.access_flags & 1 == 0 {
            return Ok(false);
        }
        if crate::p0_knowledge::same_units(current, to, budget)? {
            return Ok(true);
        }
        if class.access_flags & 0x200 != 0 {
            return Ok(false);
        }
        reserve(&mut seen, 1, budget)?;
        seen.push(current);
        let Some(parent) = class.parent.as_deref() else {
            return Ok(false);
        };
        current = parent;
    }
}
