use crate::context::Context;
use crate::input::MethodSource;
use crate::substrate::owned::{copy_proto, copy_units};
use crate::{MethodId, RequestError, RunContext};

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
    input: &MethodSource<'_, '_>,
    target: MethodId,
    catalog: &Context,
    budget: &mut RunContext<'_>,
) -> Result<Option<CovariantOverride>, RequestError> {
    collect_impl(input, target, catalog, budget)
}

fn collect_impl(
    input: &MethodSource<'_, '_>,
    target: MethodId,
    catalog: &Context,
    budget: &mut RunContext<'_>,
) -> Result<Option<CovariantOverride>, RequestError> {
    let original = input.method_reference(target, budget)?;
    let name = copy_units(original.declaration.name, budget)?;
    let prototype = copy_proto(&original.declaration.prototype, budget)?;
    let bridge =
        crate::substrate::owned::copy_slice_units(input.declaration().prototype.units(), budget)?;
    let owner = copy_units(input.owner(), budget)?;
    if !original.owner.matches_with(&owner, budget)?
        || !input.declaration().name.matches_with(&name, budget)?
    {
        return Ok(None);
    }
    // This no-argument covariant profile preserves the exact erased Object result.
    let argument_bridge = bridge.as_slice()
        == const { crate::substrate::owned::ascii_units(*b"(Ljava/lang/Object;)Ljava/lang/Object;") };
    if !argument_bridge
        && bridge != const { crate::substrate::owned::ascii_units(*b"()Ljava/lang/Object;") }
        || prototype != super::interface_scope::string_identity()
            && prototype != const { crate::substrate::owned::ascii_units(*b"()Ljava/lang/String;") }
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
                control.checkpoint()?;
                if let crate::input::catalog::MemberEntry::MethodHeader {
                    method_idx,
                    access_flags,
                    declaration,
                    ..
                } = entry
                    && method_idx == target
                {
                    let declaration = declaration?;
                    native_target = access_flags == crate::substrate::flags::PUBLIC
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
        budget.checkpoint()?;
        if method.name == name && method.prototype == prototype {
            if method.access_flags != crate::substrate::flags::PUBLIC
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
            budget.checkpoint()?;
            let Some(interface) = catalog.class(interface, budget)? else {
                return Ok(None);
            };
            for method in &interface.methods {
                budget.checkpoint()?;
                if method.name == name
                    && method.prototype == bridge
                    && method.signature.as_deref()
                        == Some(&const { crate::substrate::owned::ascii_units(*b"(TT;)TT;") })
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
        budget.checkpoint()?;
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
            budget.checkpoint()?;
            if method.name == name && method.prototype == bridge {
                if method.access_flags
                    & (1 | crate::substrate::flags::STATIC | crate::substrate::flags::FINAL)
                    != crate::substrate::flags::PUBLIC
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
