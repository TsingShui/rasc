use crate::context::Context;
use crate::input::MethodInput;
use crate::input::{DeclarationCoverage, InputClassDeclaration};
use crate::substrate::owned::{copy_proto, copy_units, reserve};
use crate::{MethodBudget, MethodId, RequestError};

#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum StaticResult {
    /// The exact non-builtin class result is retained in the sealed prototype.
    Nominal,
    Array,
    Reference(crate::substrate::primitive_types::BuiltinReference),
    Int,
    Boolean,
    Numeric,
    Void,
}
/// An exact non-generic static declaration contract. Source expression binding
/// still requires typed argument views and a checked source qualifier.
#[derive(Clone, Copy)]
pub(crate) enum StaticAccess {
    Public,
    Enclosing,
}
pub(crate) struct StaticContract {
    _sealed: (),
    access: StaticAccess,
    pub reference: MethodId,
    pub owner: Vec<u16>,
    pub name: Vec<u16>,
    pub prototype: Vec<u16>,
    pub declared_throws: Vec<Vec<u16>>,
}
pub(super) fn member_directory(
    class: &InputClassDeclaration,
    caller: &[u16],
    budget: &mut MethodBudget<'_, '_>,
) -> Result<bool, RequestError> {
    budget.charge_work(
        u64::try_from(
            class
                .descriptor
                .len()
                .checked_add(caller.len())
                .ok_or(RequestError::PlatformCapacity)?,
        )
        .map_err(|_| RequestError::PlatformCapacity)?,
    )?;
    Ok(super::raw_signature::raw_class(class, budget)?
        && matches!(class.coverage, DeclarationCoverage::AllDeclarations))
}
pub(super) fn directory(
    class: &InputClassDeclaration,
    caller: &[u16],
    budget: &mut MethodBudget<'_, '_>,
) -> Result<bool, RequestError> {
    Ok(!class.enclosing_scope && member_directory(class, caller, budget)?)
}
pub(super) fn plain(
    catalog: &Context,
    class: &InputClassDeclaration,
    caller: &[u16],
    budget: &mut MethodBudget<'_, '_>,
) -> Result<bool, RequestError> {
    Ok(
        super::interface_scope::catalog_plain(catalog, class, budget)?
            && (directory(class, caller, budget)?
                || class.descriptor == caller
                    && class.enclosing_scope
                    && matches!(class.coverage, DeclarationCoverage::AllDeclarations)
                    && !class.generic),
    )
}
fn push<T>(
    values: &mut Vec<T>,
    value: T,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<(), RequestError> {
    reserve(values, 1, budget)?;
    values.push(value);
    Ok(())
}
fn generated_name(prefix: &[u16]) -> bool {
    if prefix == [112, 99] {
        return true;
    }
    matches!(prefix.first(), Some(112 | 118 | 99))
        && prefix.len() > 1
        && prefix[1..].iter().all(|unit| matches!(*unit, 48..=57 | 95))
}
pub(super) fn clear_scope(
    catalog: &Context,
    caller: &[u16],
    prefix: &[u16],
    budget: &mut MethodBudget<'_, '_>,
) -> Result<bool, RequestError> {
    budget.charge_work(u64::try_from(prefix.len()).map_err(|_| RequestError::PlatformCapacity)?)?;
    if generated_name(prefix) {
        return Ok(false);
    }
    let mut at = caller;
    let mut seen: Vec<&[u16]> = Vec::new();
    loop {
        budget.charge_work(1)?;
        for old in &seen {
            budget.charge_work(1)?;
            if crate::context::same_units(old, at, budget)? {
                return Ok(false);
            }
        }
        push(&mut seen, at, budget)?;
        let Some(class) = catalog.class(at, budget)? else {
            return Ok(false);
        };
        if !plain(catalog, class, caller, budget)? {
            return Ok(false);
        }
        for name in &class.field_names {
            budget.charge_work(1)?;
            if crate::context::same_units(name, prefix, budget)? {
                return Ok(false);
            }
        }
        match &class.parent {
            Some(parent) => at = parent,
            None => {
                return Ok(class.descriptor
                    == [
                        76, 106, 97, 118, 97, 47, 108, 97, 110, 103, 47, 79, 98, 106, 101, 99, 116,
                        59,
                    ]);
            }
        }
    }
}
pub(crate) fn collect_static(
    input: &MethodInput<'_, '_>,
    _catalog: &Context,
    id: MethodId,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<Option<StaticContract>, RequestError> {
    let _span = crate::pipeline::phase_span("p4_static");
    let reference = input.method_reference(id, budget)?;
    let owner = copy_units(reference.owner, budget)?;
    if owner.first() != Some(&76) || owner.last() != Some(&59) {
        return Ok(None);
    }
    Ok(Some(StaticContract {
        _sealed: (),
        access: StaticAccess::Public,
        reference: id,
        owner,
        name: copy_units(reference.declaration.name, budget)?,
        prototype: copy_proto(&reference.declaration.prototype, budget)?,
        declared_throws: Vec::new(),
    }))
}

pub(crate) fn binding_available(
    input: &MethodInput<'_, '_>,
    catalog: &Context,
    contract: &StaticContract,
    invocation: &super::throws::Invocation<'_>,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<bool, RequestError> {
    let Some(class) = catalog.class(&contract.owner, budget)? else {
        // An unknown external owner does not invalidate the original reference.
        // Without hierarchy/overload facts there is no rival target to reject.
        return Ok(true);
    };
    let caller = copy_units(input.owner(), budget)?;
    Ok(super::overloads::unique_target(
        catalog,
        class,
        &caller,
        &contract.name,
        &contract.prototype,
        match contract.access {
            StaticAccess::Public => super::overloads::Scope::Static,
            StaticAccess::Enclosing => super::overloads::Scope::EnclosingStatic {
                same_class: caller == contract.owner,
            },
        },
        budget,
    )? && super::throws::compatible(
        input,
        catalog,
        Some(&contract.declared_throws),
        invocation,
        budget,
    )?)
}

pub(crate) fn qualifier_available(
    input: &MethodInput<'_, '_>,
    catalog: &Context,
    prefix: &[u16],
    budget: &mut MethodBudget<'_, '_>,
) -> Result<bool, RequestError> {
    let caller = copy_units(input.owner(), budget)?;
    Ok(
        !input.qualifier_shadowed(prefix, budget)?
            && clear_scope(catalog, &caller, prefix, budget)?,
    )
}
