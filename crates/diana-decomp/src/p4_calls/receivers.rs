use super::contracts::{clear_scope, plain};
use crate::input::MethodInput;
use crate::substrate::owned::{copy_proto, copy_units};
use crate::{DeclarationCatalog, MethodBudget, MethodId, RequestError};

#[derive(Clone, Copy)]
pub(crate) enum ReceiverKind {
    Super,
    Direct,
    Virtual,
    Interface,
}
pub(crate) struct ReceiverContract {
    _sealed: (),
    pub reference: MethodId,
    pub owner: Vec<u16>,
    pub name: Vec<u16>,
    pub prototype: Vec<u16>,
    pub kind: ReceiverKind,
    pub int_arity: usize,
    pub declared_throws: Vec<Vec<u16>>,
}
pub(crate) fn resolve_receiver(
    input: &MethodInput<'_, '_>,
    catalog: &DeclarationCatalog,
    reference: MethodId,
    kind: ReceiverKind,
    parameter: &[u16],
    requires_java_namespace: bool,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<Option<ReceiverContract>, RequestError> {
    let method = input.method_reference(reference, budget)?;
    if (!matches!(kind, ReceiverKind::Super) && !method.owner.matches_with(parameter, budget)?)
        || !method
            .declaration
            .prototype
            .return_type
            .matches_with(&[73], budget)?
    {
        return Ok(None);
    }
    let mut int_arity = 0usize;
    let mut supported = true;
    method.declaration.prototype.visit_parameter_types(
        budget,
        |name, budget| -> Result<(), RequestError> {
            if !name.matches_with(&[73], budget)? {
                supported = false;
            }
            int_arity = int_arity
                .checked_add(1)
                .ok_or(RequestError::PlatformCapacity)?;
            Ok(())
        },
    )?;
    if !supported {
        return Ok(None);
    }
    let owner = copy_units(method.owner, budget)?;
    let name = copy_units(method.declaration.name, budget)?;
    let prototype = copy_proto(&method.declaration.prototype, budget)?;
    let caller = copy_units(input.owner(), budget)?;
    if owner.len() < 3 || owner.first() != Some(&76) || owner.last() != Some(&59) {
        return Ok(None);
    }
    let Some(class) = catalog.class(&owner, budget)? else {
        return Ok(None);
    };
    if !plain(class, &caller, budget)?
        || class.access_flags & 1 == 0
        || (class.access_flags & 0x200 != 0) != matches!(kind, ReceiverKind::Interface)
        || !super::overloads::unique_target(
            class,
            &name,
            &prototype,
            false,
            matches!(kind, ReceiverKind::Direct),
            budget,
        )?
    {
        return Ok(None);
    }
    let mut selected = None;
    for candidate in &class.methods {
        budget.charge_work(1)?;
        if crate::p0_knowledge::same_units(&candidate.name, &name, budget)?
            && crate::p0_knowledge::same_units(&candidate.prototype, &prototype, budget)?
        {
            if selected.is_some() {
                return Ok(None);
            }
            selected = Some(candidate);
        }
    }
    let Some(selected) = selected else {
        return Ok(None);
    };
    if matches!(kind, ReceiverKind::Super) {
        let Some(caller_class) = catalog.class(&caller, budget)? else {
            return Ok(None);
        };
        if class.access_flags & 0x10 != 0
            || selected.access_flags & (0x400 | 0x10) != 0
            || caller_class.access_flags & 0x200 != 0
            || !crate::p0_knowledge::same_units(parameter, &caller, budget)?
        {
            return Ok(None);
        }
        let Some(parent) = caller_class.parent.as_deref() else {
            return Ok(None);
        };
        if !crate::p0_knowledge::same_units(parent, &owner, budget)? {
            return Ok(None);
        }
    }
    if matches!(kind, ReceiverKind::Direct)
        && (!crate::p0_knowledge::same_units(&owner, &caller, budget)?
            || selected.access_flags & 2 == 0
            || selected.access_flags & 0x400 != 0)
    {
        return Ok(None);
    }
    if !super::throws::compatible(input, catalog, selected.throws.as_deref(), budget)? {
        return Ok(None);
    }
    if let Some(original) = input.local_method(reference, budget)? {
        let mask = 1 | 2 | 4 | 8 | 0x10 | 0x40 | 0x80 | 0x100 | 0x400;
        let class_mask = 1 | 0x10 | 0x200 | 0x400 | 0x1000 | 0x2000 | 0x4000;
        if original.owner_access_flags & class_mask != class.access_flags & class_mask
            || original.access_flags & mask != selected.access_flags & mask
            || (original.owner_access_flags & 0x200 != 0) != matches!(kind, ReceiverKind::Interface)
            || !original.plain_scope
        {
            return Ok(None);
        }
        let parent = match (original.parent, class.parent.as_deref()) {
            (None, None) => true,
            (Some(original), Some(expected)) => original.matches_with(expected, budget)?,
            _ => false,
        };
        if !parent {
            return Ok(None);
        }
    }
    let Some(parent) = class.parent.as_deref() else {
        return Ok(None);
    };
    if !crate::p0_knowledge::same_units(
        parent,
        crate::p5_types::BuiltinReference::Object.descriptor(),
        budget,
    )? {
        return Ok(None);
    }
    let Some(object) = catalog.class(parent, budget)? else {
        return Ok(None);
    };
    if !plain(object, &caller, budget)? || object.parent.is_some() {
        return Ok(None);
    }
    for candidate in &object.methods {
        budget.charge_work(1)?;
        if crate::p0_knowledge::same_units(&candidate.name, &name, budget)? {
            return Ok(None);
        }
    }
    let Some(caller_class) = catalog.class(&caller, budget)? else {
        return Ok(None);
    };
    if !input.plain_owner_scope(caller_class.parent.as_deref(), budget)? {
        return Ok(None);
    }
    let end = owner[1..owner.len() - 1]
        .iter()
        .position(|&unit| unit == 47)
        .map_or(owner.len() - 1, |index| index + 1);
    if input.qualifier_shadowed(&owner[1..end], budget)?
        || !clear_scope(catalog, &caller, &owner[1..end], budget)?
    {
        return Ok(None);
    }
    if requires_java_namespace
        && (input.qualifier_shadowed(&[106, 97, 118, 97], budget)?
            || !clear_scope(catalog, &caller, &[106, 97, 118, 97], budget)?)
    {
        return Ok(None);
    }
    Ok(Some(ReceiverContract {
        _sealed: (),
        reference,
        owner,
        name,
        prototype,
        kind,
        int_arity,
        declared_throws: super::throws::retain(selected.throws.as_deref(), budget)?,
    }))
}
