use super::contracts::plain;
use crate::input::MethodInput;
use crate::substrate::owned::{copy_proto, copy_units, reserve};
use crate::{CodeUnitOffset, DeclarationCatalog, MethodBudget, MethodId, RequestError};

pub(crate) struct ConstructorContract {
    _sealed: (),
    pub reference: MethodId,
    pub owner: Vec<u16>,
    pub prototype: Vec<u16>,
    pub arity: usize,
    pub declared_throws: Vec<Vec<u16>>,
}
#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum ConstructorUse {
    Allocation,
    Super,
    This,
}
pub(crate) fn resolve(
    input: &MethodInput<'_, '_>,
    catalog: &DeclarationCatalog,
    invocation: &super::throws::Invocation<'_>,
    expected_owner: &[u16],
    use_kind: ConstructorUse,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<ConstructorContract, RequestError> {
    let reference = invocation.reference;
    let pc = invocation.pc;
    let method = input.method_reference(reference, budget)?;
    if !method.owner.matches_with(expected_owner, budget)?
        || !method
            .declaration
            .name
            .matches_with(&[60, 105, 110, 105, 116, 62], budget)?
        || !method
            .declaration
            .prototype
            .return_type
            .matches_with(&[86], budget)?
    {
        return unavailable(pc);
    }
    let prototype = copy_proto(&method.declaration.prototype, budget)?;
    let caller = copy_units(input.owner(), budget)?;
    if matches!(use_kind, ConstructorUse::This)
        && !crate::p0_knowledge::same_units(expected_owner, &caller, budget)?
    {
        return Err(RequestError::BindingEvidenceMissing { reference, pc });
    }
    if matches!(use_kind, ConstructorUse::Super) {
        let Some(caller_class) = catalog.class(&caller, budget)? else {
            return Err(RequestError::BindingEvidenceMissing { reference, pc });
        };
        let mask = 1 | 0x10 | 0x200 | 0x400 | 0x1000 | 0x2000 | 0x4000;
        if caller_class.access_flags & mask != input.owner_access_flags() & mask
            || !plain(caller_class, &caller, budget)?
            || !input.plain_owner_scope(caller_class.parent.as_deref(), budget)?
            || !matches!(caller_class.parent.as_deref(),Some(parent) if crate::p0_knowledge::same_units(parent,expected_owner,budget)?)
        {
            return Err(RequestError::BindingEvidenceMissing { reference, pc });
        }
    }
    let Some(class) = catalog.class(expected_owner, budget)? else {
        return Err(RequestError::BindingEvidenceMissing { reference, pc });
    };
    if !super::contracts::directory(class, &caller, budget)?
        || class.access_flags & 0x6200 != 0
        || (matches!(use_kind, ConstructorUse::Allocation) && class.access_flags & 0x400 != 0)
        || !super::reference_types::source_type(input, catalog, expected_owner, budget)?
        || !super::overloads::unique_target(
            catalog,
            class,
            &caller,
            &[60, 105, 110, 105, 116, 62],
            &prototype,
            super::overloads::Scope::Constructor {
                same_class: matches!(use_kind, ConstructorUse::This),
            },
            budget,
        )?
    {
        return Err(RequestError::BindingEvidenceMissing { reference, pc });
    }
    let mut target = None;
    for candidate in &class.methods {
        budget.charge_work(1)?;
        if crate::p0_knowledge::same_units(&candidate.name, &[60, 105, 110, 105, 116, 62], budget)?
            && crate::p0_knowledge::same_units(&candidate.prototype, &prototype, budget)?
        {
            if target.is_some() {
                return Err(RequestError::BindingEvidenceMissing { reference, pc });
            }
            target = Some(candidate);
        }
    }
    let Some(target) = target else {
        return Err(RequestError::BindingEvidenceMissing { reference, pc });
    };
    if target.access_flags & 8 != 0
        || (!matches!(use_kind, ConstructorUse::This) && target.access_flags & 1 == 0)
        || target.access_flags & (0x100 | 0x400) != 0
        || target.generic
        || !super::throws::compatible(input, catalog, target.throws.as_deref(), invocation, budget)?
    {
        return Err(RequestError::BindingEvidenceMissing { reference, pc });
    }
    if let Some(original) = input.local_method(reference, budget)? {
        let mask = 1 | 2 | 4 | 8 | 0x10 | 0x40 | 0x80 | 0x100 | 0x400;
        let class_mask = 1 | 0x10 | 0x200 | 0x400 | 0x1000 | 0x2000 | 0x4000;
        if original.access_flags & mask != target.access_flags & mask
            || original.owner_access_flags & class_mask != class.access_flags & class_mask
            || !original.plain_scope
        {
            return Err(RequestError::BindingEvidenceMissing { reference, pc });
        }
        let parent = match (original.parent, class.parent.as_deref()) {
            (None, None) => true,
            (Some(a), Some(b)) => a.matches_with(b, budget)?,
            _ => false,
        };
        if !parent {
            return Err(RequestError::BindingEvidenceMissing { reference, pc });
        }
    }
    let Some(formals) = super::contracts::original_formals(input, reference, budget)? else {
        return unavailable(pc);
    };
    let arity = formals.len();

    if matches!(use_kind, ConstructorUse::Super) && class.access_flags & 0x10 != 0 {
        return Err(RequestError::BindingEvidenceMissing { reference, pc });
    }
    Ok(ConstructorContract {
        _sealed: (),
        reference,
        owner: copy_units(method.owner, budget)?,
        prototype,
        arity,
        declared_throws: super::throws::retain(target.throws.as_deref(), budget)?,
    })
}

impl ConstructorContract {
    pub(super) fn retain(&self, budget: &mut MethodBudget<'_, '_>) -> Result<Self, RequestError> {
        Ok(Self {
            _sealed: (),
            reference: self.reference,
            owner: super::receivers::retain(&self.owner, budget)?,
            prototype: super::receivers::retain(&self.prototype, budget)?,
            arity: self.arity,
            declared_throws: super::throws::retain(Some(&self.declared_throws), budget)?,
        })
    }
}

fn unavailable<T>(pc: CodeUnitOffset) -> Result<T, RequestError> {
    Err(RequestError::InitializationUnavailable { pc })
}
pub(crate) fn prove_this_chain(
    input: &MethodInput<'_, '_>,
    catalog: &DeclarationCatalog,
    target: MethodId,
    origin: CodeUnitOffset,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<(), RequestError> {
    let mut seen = Vec::new();
    reserve(&mut seen, 1, budget)?;
    seen.push(input.method_id());
    let mut current = target;
    let owner = copy_units(input.owner(), budget)?;
    loop {
        budget.charge_work(1)?;
        for &method in &seen {
            budget.charge_work(1)?;
            if method == current {
                return unavailable(origin);
            }
        }
        reserve(&mut seen, 1, budget)?;
        seen.push(current);
        let Some(link) = input.constructor_link(current, budget)? else {
            return unavailable(origin);
        };
        match link.kind {
            crate::input::ConstructorLinkKind::This => {
                let _contract = resolve(
                    input,
                    catalog,
                    &super::throws::Invocation {
                        reference: link.target,
                        pc: link.pc,
                        protection: None,
                    },
                    &owner,
                    ConstructorUse::This,
                    budget,
                )?;
                current = link.target;
            }
            crate::input::ConstructorLinkKind::Super => {
                let parent =
                    copy_units(input.method_reference(link.target, budget)?.owner, budget)?;
                let _contract = resolve(
                    input,
                    catalog,
                    &super::throws::Invocation {
                        reference: link.target,
                        pc: link.pc,
                        protection: None,
                    },
                    &parent,
                    ConstructorUse::Super,
                    budget,
                )?;
                return Ok(());
            }
        }
    }
}
