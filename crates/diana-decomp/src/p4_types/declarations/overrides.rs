use crate::context::Context;
use crate::input::MethodInput;
use crate::substrate::owned::{copy_proto, copy_units};
use crate::{MethodBudget, MethodId, RequestError};

pub(crate) struct CovariantOverride {
    target: MethodId,
    name: Vec<u16>,
    prototype: Vec<u16>,
}
impl CovariantOverride {
    pub(crate) fn target(&self) -> MethodId {
        self.target
    }
    pub(crate) fn name(&self) -> &[u16] {
        &self.name
    }
    pub(crate) fn prototype(&self) -> &[u16] {
        &self.prototype
    }
}

pub(crate) fn collect(
    _permit: &crate::engine::SchedulePermit,
    input: &MethodInput<'_, '_>,
    target: MethodId,
    catalog: &Context,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<Option<CovariantOverride>, RequestError> {
    collect_impl(input, target, catalog, budget)
}

fn collect_impl(
    input: &MethodInput<'_, '_>,
    target: MethodId,
    catalog: &Context,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<Option<CovariantOverride>, RequestError> {
    let original = input.method_reference(target, budget)?;
    let name = copy_units(original.declaration.name, budget)?;
    let prototype = copy_proto(&original.declaration.prototype, budget)?;
    let bridge = copy_proto(&input.declaration().prototype, budget)?;
    let owner = copy_units(input.owner(), budget)?;
    if !original.owner.matches_with(&owner, budget)?
        || !input.declaration().name.matches_with(&name, budget)?
    {
        return Ok(None);
    }
    // This no-argument covariant profile preserves the exact erased Object result.
    let argument_bridge = bridge.as_slice()
        == [
            40, 76, 106, 97, 118, 97, 47, 108, 97, 110, 103, 47, 79, 98, 106, 101, 99, 116, 59, 41,
            76, 106, 97, 118, 97, 47, 108, 97, 110, 103, 47, 79, 98, 106, 101, 99, 116, 59,
        ];
    if !argument_bridge
        && bridge
            != [
                40, 41, 76, 106, 97, 118, 97, 47, 108, 97, 110, 103, 47, 79, 98, 106, 101, 99, 116,
                59,
            ]
        || prototype != super::interface_scope::string_identity()
            && prototype
                != [
                    40, 41, 76, 106, 97, 118, 97, 47, 108, 97, 110, 103, 47, 83, 116, 114, 105,
                    110, 103, 59,
                ]
    {
        return Ok(None);
    }
    let mut native_target = false;
    input
        .program()
        .class_with(input.class_id(), budget)?
        .visit_members_with(
            budget,
            crate::input::catalog::MemberSelection::AllHeaders,
            |entry, control| -> Result<(), RequestError> {
                control.charge_work(1)?;
                if let crate::input::catalog::MemberEntry::MethodHeader {
                    method_idx,
                    access_flags,
                    declaration,
                } = entry
                    && method_idx == target
                {
                    let declaration = declaration?;
                    native_target = access_flags == 1
                        && declaration.name.matches_with(&name, control)?
                        && crate::substrate::owned::copy_proto(&declaration.prototype, control)?
                            == prototype;
                }
                Ok(())
            },
        )?;
    if !native_target {
        return Ok(None);
    }
    let Some(class) = catalog.class(&owner, budget)? else {
        return Ok(None);
    };
    if !matches!(
        class.coverage,
        crate::input::DeclarationCoverage::AllDeclarations
    ) || class.generic
        || class.enclosing_scope
        || !class.interfaces.is_empty()
            && !super::interface_scope::collect(input.program(), input.class_id(), catalog, budget)?
                .is_some_and(|interfaces| interfaces.string_specialization())
    {
        return Ok(None);
    }
    let mut selected = 0usize;
    for method in &class.methods {
        budget.charge_work(1)?;
        if method.name == name && method.prototype == prototype {
            if method.access_flags != 1
                || method.generic
                || !matches!(&method.throws, Some(throws) if throws.is_empty())
            {
                return Ok(None);
            }
            selected = selected
                .checked_add(1)
                .ok_or(RequestError::PlatformCapacity)?;
        }
    }
    if selected != 1 {
        return Ok(None);
    }
    if argument_bridge {
        if prototype != super::interface_scope::string_identity() {
            return Ok(None);
        }
        for interface in &class.interfaces {
            budget.charge_work(1)?;
            let Some(interface) = catalog.class(interface, budget)? else {
                return Ok(None);
            };
            for method in &interface.methods {
                budget.charge_work(1)?;
                if method.name == name
                    && method.prototype == bridge
                    && method.signature.as_deref() == Some(&[40, 84, 84, 59, 41, 84, 84, 59])
                {
                    return Ok(Some(CovariantOverride {
                        target,
                        name,
                        prototype,
                    }));
                }
            }
        }
        return Ok(None);
    }
    let Some(parent) = &class.parent else {
        return Ok(None);
    };
    let Some(parent) = catalog.class(parent, budget)? else {
        return Ok(None);
    };
    let Some(hierarchy) = super::hierarchy::collect(catalog, parent, budget)? else {
        return Ok(None);
    };
    let mut inherited = false;
    for ancestor in hierarchy.declarations() {
        budget.charge_work(1)?;
        if ancestor.generic
            || ancestor.enclosing_scope
            || !matches!(
                ancestor.coverage,
                crate::input::DeclarationCoverage::AllDeclarations
            )
        {
            return Ok(None);
        }
        for method in &ancestor.methods {
            budget.charge_work(1)?;
            if method.name == name && method.prototype == bridge {
                if method.access_flags & (1 | 8 | 16) != 1
                    || method.generic
                    || !matches!(&method.throws, Some(throws) if throws.is_empty())
                {
                    return Ok(None);
                }
                inherited = true;
            }
        }
    }
    if !inherited {
        return Ok(None);
    }
    Ok(Some(CovariantOverride {
        target,
        name,
        prototype,
    }))
}
