use crate::context::Context;
use crate::input::MethodInput;
use crate::{CodeUnitOffset, FieldId, MethodBudget, RequestError};

pub(crate) struct InstanceIntWrite {
    _sealed: (),
    pub reference: FieldId,
    pub name: Vec<u16>,
}
pub(crate) fn bind_own_int_write(
    input: &MethodInput<'_, '_>,
    catalog: &Context,
    reference: FieldId,
    pc: CodeUnitOffset,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<InstanceIntWrite, RequestError> {
    let missing = || RequestError::FieldBindingEvidenceMissing { reference, pc };
    let original = input.field_reference(reference, budget)?;
    if !original
        .declaration
        .field_type
        .matches_with(&[73], budget)?
    {
        return Err(missing());
    }
    let Some(native) = input.own_field_metadata(reference, budget)? else {
        return Err(missing());
    };
    let Some(evidence) = catalog.field(
        original.owner,
        original.declaration.name,
        original.declaration.field_type,
        budget,
    )?
    else {
        return Err(missing());
    };
    if native.same_named_fields != 1
        || native.access_flags != evidence.access_flags
        || native.access_flags & 8 != 0
        || native.access_flags & (16 | 64) == (16 | 64)
    {
        return Err(missing());
    }
    let Some(class) = catalog.class(&evidence.owner, budget)? else {
        return Err(missing());
    };
    let caller = crate::substrate::owned::copy_units(input.owner(), budget)?;
    if !super::contracts::plain(catalog, class, &caller, budget)?
        || !super::nested_scope::plain_owner(input, catalog, class.parent.as_deref(), budget)?
    {
        return Err(missing());
    }
    let mut names = 0usize;
    for name in &class.field_names {
        budget.charge_work(1)?;
        if crate::context::same_units(name, &evidence.name, budget)? {
            names = names.checked_add(1).ok_or(RequestError::PlatformCapacity)?;
        }
    }
    if names != 1 {
        return Err(missing());
    }
    Ok(InstanceIntWrite {
        _sealed: (),
        reference,
        name: crate::substrate::owned::copy_units(original.declaration.name, budget)?,
    })
}
