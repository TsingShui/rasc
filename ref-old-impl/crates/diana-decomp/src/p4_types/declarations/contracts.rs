use crate::context::Context;
use crate::input::MethodSource;
use crate::input::{DeclarationCoverage, InputClassDeclaration};
use crate::substrate::owned::{copy_proto, copy_units, reserve};
use crate::{MethodId, RequestError, RunContext};

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
}
pub(super) fn member_directory(
    class: &InputClassDeclaration,
    _caller: &[u16],
    budget: &mut RunContext<'_>,
) -> Result<bool, RequestError> {
    budget.checkpoint()?;
    Ok(super::raw_signature::raw_class(class, budget)?
        && matches!(class.coverage, DeclarationCoverage::AllDeclarations))
}
pub(super) fn directory(
    class: &InputClassDeclaration,
    caller: &[u16],
    budget: &mut RunContext<'_>,
) -> Result<bool, RequestError> {
    Ok(!class.enclosing_scope && member_directory(class, caller, budget)?)
}
pub(super) fn plain(
    catalog: &Context,
    class: &InputClassDeclaration,
    caller: &[u16],
    budget: &mut RunContext<'_>,
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
fn push<T>(values: &mut Vec<T>, value: T, budget: &mut RunContext<'_>) -> Result<(), RequestError> {
    reserve(values, 1, budget)?;
    values.push(value);
    Ok(())
}
fn generated_name(prefix: &[u16]) -> bool {
    if prefix == const { crate::substrate::owned::ascii_units(*b"pc") } {
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
    budget: &mut RunContext<'_>,
) -> Result<bool, RequestError> {
    budget.checkpoint()?;
    if generated_name(prefix) {
        return Ok(false);
    }
    let mut at = caller;
    let mut seen: Vec<&[u16]> = Vec::new();
    loop {
        budget.checkpoint()?;
        for old in &seen {
            budget.checkpoint()?;
            if crate::context::same_units(old, at, budget)? {
                return Ok(false);
            }
        }
        push(&mut seen, at, budget)?;
        let Some(class) = catalog.class(at, budget)? else {
            return Ok(false);
        };
        if !matches!(
            class.coverage,
            crate::input::DeclarationCoverage::AllDeclarations
        ) {
            return Ok(false);
        }
        for name in &class.field_names {
            budget.checkpoint()?;
            if crate::context::same_units(name, prefix, budget)? {
                return Ok(false);
            }
        }
        match &class.parent {
            Some(parent) => at = parent,
            None => return Ok(true),
        }
    }
}
pub(crate) fn collect_static(
    input: &MethodSource<'_, '_>,
    _catalog: &Context,
    id: MethodId,
    budget: &mut RunContext<'_>,
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
    }))
}

pub(crate) fn binding_available(
    input: &MethodSource<'_, '_>,
    catalog: &Context,
    contract: &StaticContract,
    _invocation: &super::throws::Invocation,
    budget: &mut RunContext<'_>,
) -> Result<bool, RequestError> {
    // The original reference fixes the target. Overload-resolution uniqueness
    // only decides whether javac would pick the same method, so it is recorded
    // as source uncertainty instead of removing the call and the whole body.
    let Some(class) = catalog.class(&contract.owner, budget)? else {
        return Ok(true);
    };
    let caller = copy_units(input.owner(), budget)?;
    super::contracts::member_directory(class, &caller, budget)
}

pub(crate) fn qualifier_available(
    input: &MethodSource<'_, '_>,
    catalog: &Context,
    prefix: &[u16],
    budget: &mut RunContext<'_>,
) -> Result<bool, RequestError> {
    let caller = copy_units(input.owner(), budget)?;
    Ok(
        !input.qualifier_shadowed(prefix, budget)?
            && clear_scope(catalog, &caller, prefix, budget)?,
    )
}
