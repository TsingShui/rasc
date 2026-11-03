use super::contracts::clear_scope;
use crate::input::MethodInput;
use crate::substrate::owned::{copy_proto, copy_units};
use crate::{DeclarationCatalog, MethodBudget, MethodId, RequestError};

#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum ReceiverKind {
    Super,
    Direct,
    Virtual,
    Interface,
}
pub(crate) struct ReceiverDeclaration {
    target: ReceiverContract,
}

pub(crate) struct ReceiverContract {
    _sealed: (),
    pub reference: MethodId,
    pub owner: Vec<u16>,
    pub name: Vec<u16>,
    pub prototype: Vec<u16>,
    pub kind: ReceiverKind,
    pub arity: usize,
    pub declared_throws: Vec<Vec<u16>>,
}
pub(crate) fn collect_receiver(
    input: &MethodInput<'_, '_>,
    catalog: &DeclarationCatalog,
    invocation: &super::throws::Invocation<'_>,
    kind: ReceiverKind,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<Option<ReceiverDeclaration>, RequestError> {
    let declared = collect_receiver_declared(input, catalog, invocation, kind, budget)?;
    if declared.is_some() || catalog.policy() != crate::EvidencePolicy::TrustedReferences {
        return Ok(declared);
    }
    let reference = invocation.reference;
    let method = input.method_reference(reference, budget)?;
    let Some(formals) = super::contracts::original_formals(input, reference, budget)? else {
        return Ok(None);
    };
    let owner = copy_units(method.owner, budget)?;
    if owner.len() < 3 || owner.first() != Some(&76) || owner.last() != Some(&59) {
        return Ok(None);
    }
    Ok(Some(ReceiverDeclaration {
        target: ReceiverContract {
            _sealed: (),
            reference,
            owner,
            name: copy_units(method.declaration.name, budget)?,
            prototype: copy_proto(&method.declaration.prototype, budget)?,
            kind,
            arity: formals.len(),
            declared_throws: Vec::new(),
        },
    }))
}

fn collect_receiver_declared(
    input: &MethodInput<'_, '_>,
    catalog: &DeclarationCatalog,
    invocation: &super::throws::Invocation<'_>,
    kind: ReceiverKind,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<Option<ReceiverDeclaration>, RequestError> {
    let reference = invocation.reference;
    let method = input.method_reference(reference, budget)?;
    let Some(formals) = super::contracts::original_formals(input, reference, budget)? else {
        return Ok(None);
    };
    let arity = formals.len();
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
    if !super::contracts::directory(class, &caller, budget)?
        || class.access_flags & 1 == 0
        || (class.access_flags & 0x200 != 0) != matches!(kind, ReceiverKind::Interface)
        || !super::overloads::unique_target(
            catalog,
            class,
            &caller,
            &name,
            &prototype,
            super::overloads::Scope::Receiver {
                source: class,
                direct: matches!(kind, ReceiverKind::Direct),
            },
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
    if !super::throws::compatible(
        input,
        catalog,
        selected.throws.as_deref(),
        invocation,
        budget,
    )? {
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
    Ok(Some(ReceiverDeclaration {
        target: ReceiverContract {
            _sealed: (),
            reference,
            owner,
            name,
            prototype,
            kind,
            arity,
            declared_throws: super::throws::retain(selected.throws.as_deref(), budget)?,
        },
    }))
}
impl ReceiverDeclaration {
    pub(crate) fn bind_source(
        &self,
        input: &MethodInput<'_, '_>,
        catalog: &DeclarationCatalog,
        parameter: &[u16],
        budget: &mut MethodBudget<'_, '_>,
    ) -> Result<Option<ReceiverContract>, RequestError> {
        let target = &self.target;
        let caller = copy_units(input.owner(), budget)?;
        if target.kind == ReceiverKind::Super {
            if !crate::p0_knowledge::same_units(parameter, &caller, budget)? {
                return Ok(None);
            }
        } else if super::reference_types::source_subtype(
            input,
            catalog,
            parameter,
            &target.owner,
            budget,
        )? == Some(false)
        {
            return Ok(None);
        }
        // External declaration facts are optional. When the receiver's own owner
        // or its static type has no catalog entry, the original reference still
        // fixes the call; the missing hierarchy/hiding facts stay unknown.
        let (Some(class), Some(source)) = (
            catalog.class(&target.owner, budget)?,
            catalog.class(parameter, budget)?,
        ) else {
            if catalog.policy() == crate::EvidencePolicy::TrustedReferences {
                return Ok(Some(ReceiverContract {
                    _sealed: (),
                    reference: target.reference,
                    owner: retain(&target.owner, budget)?,
                    name: retain(&target.name, budget)?,
                    prototype: retain(&target.prototype, budget)?,
                    kind: target.kind,
                    arity: target.arity,
                    declared_throws: super::throws::retain(Some(&target.declared_throws), budget)?,
                }));
            }
            return Ok(None);
        };
        // P4 has checked the referenced declaration group. The final static
        // receiver type can introduce hiding/overloads, so P8 discharges this
        // source-binding obligation only after P5 chooses that type.
        if !super::overloads::unique_target(
            catalog,
            class,
            &caller,
            &target.name,
            &target.prototype,
            super::overloads::Scope::Receiver {
                source: if target.kind == ReceiverKind::Super {
                    class
                } else {
                    source
                },
                direct: target.kind == ReceiverKind::Direct,
            },
            budget,
        )? {
            return Ok(None);
        }
        Ok(Some(ReceiverContract {
            _sealed: (),
            reference: target.reference,
            owner: retain(&target.owner, budget)?,
            name: retain(&target.name, budget)?,
            prototype: retain(&target.prototype, budget)?,
            kind: target.kind,
            arity: target.arity,
            declared_throws: super::throws::retain(Some(&target.declared_throws), budget)?,
        }))
    }
}
pub(super) fn retain(
    units: &[u16],
    budget: &mut MethodBudget<'_, '_>,
) -> Result<Vec<u16>, RequestError> {
    let mut result = Vec::new();
    for &unit in units {
        budget.charge_work(1)?;
        crate::substrate::owned::push(&mut result, unit, budget)?;
    }
    Ok(result)
}
