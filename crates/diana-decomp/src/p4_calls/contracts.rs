use crate::input::MethodInput;
use crate::substrate::owned::{copy_proto, copy_units, reserve};
use crate::{
    DeclarationCatalog, DeclarationCoverage, ExternalClassDeclaration, MethodBudget, MethodId,
    RequestError,
};

#[derive(Clone, Copy)]
pub(crate) enum CallFormal {
    Numeric(crate::p2_flow::primitives::ScalarType),
    Int,
    Boolean,
    Narrow(crate::substrate::primitive_types::Narrow),
    Reference(crate::substrate::primitive_types::BuiltinReference),
}
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
    pub formals: Vec<CallFormal>,
    pub declared_throws: Vec<Vec<u16>>,
}
fn package(descriptor: &[u16]) -> &[u16] {
    descriptor
        .iter()
        .rposition(|&unit| unit == 47)
        .map_or(&[], |end| &descriptor[..end])
}
pub(super) fn member_directory(
    class: &ExternalClassDeclaration,
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
        && match class.coverage {
            DeclarationCoverage::AllDeclarations => true,
            DeclarationCoverage::ExportedApi => package(&class.descriptor) != package(caller),
        })
}
pub(super) fn directory(
    class: &ExternalClassDeclaration,
    caller: &[u16],
    budget: &mut MethodBudget<'_, '_>,
) -> Result<bool, RequestError> {
    Ok(!class.enclosing_scope && member_directory(class, caller, budget)?)
}
pub(super) fn plain(
    catalog: &DeclarationCatalog,
    class: &ExternalClassDeclaration,
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
    catalog: &DeclarationCatalog,
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
            if crate::p0_knowledge::same_units(old, at, budget)? {
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
            if crate::p0_knowledge::same_units(name, prefix, budget)? {
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
pub(crate) fn qualifier_available(
    input: &MethodInput<'_, '_>,
    catalog: &DeclarationCatalog,
    prefix: &[u16],
    budget: &mut MethodBudget<'_, '_>,
) -> Result<bool, RequestError> {
    let caller = copy_units(input.owner(), budget)?;
    Ok(
        !input.qualifier_shadowed(prefix, budget)?
            && clear_scope(catalog, &caller, prefix, budget)?,
    )
}
pub(super) fn collect_static(
    input: &MethodInput<'_, '_>,
    catalog: &DeclarationCatalog,
    id: MethodId,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<Option<StaticContract>, RequestError> {
    let reference = input.method_reference(id, budget)?;
    let owner = copy_units(reference.owner, budget)?;
    let name = copy_units(reference.declaration.name, budget)?;
    let prototype = copy_proto(&reference.declaration.prototype, budget)?;
    if owner.first() != Some(&76) || owner.last() != Some(&59) {
        return Ok(None);
    }
    let Some(target_class) = catalog.class(&owner, budget)? else {
        return Ok(None);
    };
    let caller = copy_units(input.owner(), budget)?;
    let Some(caller_class) = catalog.class(&caller, budget)? else {
        return Ok(None);
    };
    if !super::nested_scope::plain_owner(input, catalog, caller_class.parent.as_deref(), budget)? {
        return Ok(None);
    }
    let enclosing = super::nested_scope::same_nest(input, &owner, budget)?;
    if !(directory(target_class, &caller, budget)?
        || enclosing && member_directory(target_class, &caller, budget)?)
        || target_class.access_flags & 1 == 0
        || target_class.access_flags & 0x200 != 0
    {
        return Ok(None);
    }
    let mut target = None;
    for method in &target_class.methods {
        budget.charge_work(1)?;
        if crate::p0_knowledge::same_units(&method.name, &name, budget)?
            && crate::p0_knowledge::same_units(&method.prototype, &prototype, budget)?
        {
            if target.is_some() {
                return Ok(None);
            }
            target = Some(method);
        }
    }
    let Some(target) = target else {
        return Ok(None);
    };
    if !crate::p0_knowledge::same_units(&target.prototype, &prototype, budget)?
        || target.access_flags & 8 == 0
        || target.access_flags & 1 == 0
            && !(enclosing && (target.access_flags & 7 == 0 || owner == caller))
        || target.access_flags & (0x40 | 0x80) != 0
        || !super::raw_signature::object_identity(target, budget)?
        || target.throws.is_none()
    {
        return Ok(None);
    }
    if let Some(original) = input.local_method(id, budget)? {
        let method_mask = 1 | 2 | 4 | 8 | 0x40 | 0x80 | 0x100 | 0x400;
        let class_mask = 1 | 0x10 | 0x200 | 0x400 | 0x1000 | 0x2000 | 0x4000;
        if original.access_flags & method_mask != target.access_flags & method_mask
            || original.owner_access_flags & class_mask != target_class.access_flags & class_mask
            || !original.plain_scope && !enclosing
        {
            return Ok(None);
        }
        let parent_matches = match (original.parent, target_class.parent.as_deref()) {
            (None, None) => true,
            (Some(original), Some(expected)) => original.matches_with(expected, budget)?,
            _ => false,
        };
        if !parent_matches {
            return Ok(None);
        }
    }
    let Some(hierarchy) = super::hierarchy::collect(catalog, target_class, budget)? else {
        return Ok(None);
    };
    for class in hierarchy.declarations() {
        budget.charge_work(1)?;
        if !member_directory(class, &caller, budget)? {
            return Ok(None);
        }
    }
    let mut current = &owner[..];
    let mut seen: Vec<&[u16]> = Vec::new();
    loop {
        budget.charge_work(1)?;
        for old in &seen {
            budget.charge_work(1)?;
            if crate::p0_knowledge::same_units(old, current, budget)? {
                return Ok(None);
            }
        }
        push(&mut seen, current, budget)?;
        let Some(class) = catalog.class(current, budget)? else {
            return Ok(None);
        };
        if !(directory(class, &caller, budget)?
            || enclosing && current == owner && member_directory(class, &caller, budget)?)
        {
            return Ok(None);
        }
        if !crate::p0_knowledge::same_units(current, &owner, budget)? {
            for method in &class.methods {
                budget.charge_work(1)?;
                if crate::p0_knowledge::same_units(&method.name, &name, budget)? {
                    return Ok(None);
                }
            }
        }
        match &class.parent {
            Some(parent) => current = parent,
            None => {
                if class.descriptor
                    != [
                        76, 106, 97, 118, 97, 47, 108, 97, 110, 103, 47, 79, 98, 106, 101, 99, 116,
                        59,
                    ]
                {
                    return Ok(None);
                }
                break;
            }
        }
    }
    let prefix_end = owner[1..owner.len() - 1]
        .iter()
        .position(|&unit| unit == 47)
        .map_or(owner.len() - 1, |index| index + 1);
    if input.qualifier_shadowed(&owner[1..prefix_end], budget)?
        || !clear_scope(catalog, &caller, &owner[1..prefix_end], budget)?
    {
        return Ok(None);
    }
    let result_tag = reference
        .declaration
        .prototype
        .return_type
        .reader()?
        .next(budget)?;
    let result = if matches!(result_tag, Some(66 | 67 | 68 | 70 | 74 | 83)) {
        StaticResult::Numeric
    } else if reference
        .declaration
        .prototype
        .return_type
        .matches_with(&[73], budget)?
    {
        StaticResult::Int
    } else if reference
        .declaration
        .prototype
        .return_type
        .matches_with(&[90], budget)?
    {
        StaticResult::Boolean
    } else if reference.declaration.prototype.return_type.matches_with(
        crate::substrate::primitive_types::BuiltinReference::String.descriptor(),
        budget,
    )? {
        StaticResult::Reference(crate::substrate::primitive_types::BuiltinReference::String)
    } else if reference.declaration.prototype.return_type.matches_with(
        crate::substrate::primitive_types::BuiltinReference::Object.descriptor(),
        budget,
    )? {
        StaticResult::Reference(crate::substrate::primitive_types::BuiltinReference::Object)
    } else if reference
        .declaration
        .prototype
        .return_type
        .matches_with(&[86], budget)?
    {
        StaticResult::Void
    } else if reference
        .declaration
        .prototype
        .return_type
        .reader()?
        .next(budget)?
        == Some(91)
    {
        StaticResult::Array
    } else {
        let descriptor = copy_units(reference.declaration.prototype.return_type, budget)?;
        if descriptor.len() < 3 || descriptor.first() != Some(&76) || descriptor.last() != Some(&59)
        {
            return Ok(None);
        }
        let Some(class) = catalog.class(&descriptor, budget)? else {
            return Ok(None);
        };
        if !plain(catalog, class, &caller, budget)?
            || class.access_flags & 1 == 0
            || class.access_flags & 0x2200 != 0
        {
            return Ok(None);
        }
        let end = descriptor[1..descriptor.len() - 1]
            .iter()
            .position(|&unit| unit == 47)
            .map_or(descriptor.len() - 1, |index| index + 1);
        if input.qualifier_shadowed(&descriptor[1..end], budget)?
            || !clear_scope(catalog, &caller, &descriptor[1..end], budget)?
        {
            return Ok(None);
        }
        StaticResult::Nominal
    };
    let Some(formals) = original_formals(input, id, budget)? else {
        return Ok(None);
    };
    if (matches!(result, StaticResult::Reference(_))
        || formals
            .iter()
            .any(|formal| matches!(formal, CallFormal::Reference(_))))
        && (input.qualifier_shadowed(&[106, 97, 118, 97], budget)?
            || !clear_scope(catalog, &caller, &[106, 97, 118, 97], budget)?)
    {
        return Ok(None);
    }
    Ok(Some(StaticContract {
        _sealed: (),
        access: if target.access_flags & 1 != 0 {
            StaticAccess::Public
        } else {
            StaticAccess::Enclosing
        },
        reference: id,
        owner,
        name,
        prototype,
        formals,
        declared_throws: super::throws::retain(target.throws.as_deref(), budget)?,
    }))
}

pub(super) fn original_formals(
    input: &MethodInput<'_, '_>,
    id: MethodId,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<Option<Vec<CallFormal>>, RequestError> {
    let reference = input.method_reference(id, budget)?;
    let mut formals = Vec::new();
    let mut supported = true;
    reference.declaration.prototype.visit_parameter_types(
        budget,
        |name, budget| -> Result<(), RequestError> {
            use crate::substrate::primitive_types::Narrow;
            let formal = if name.matches_with(&[73], budget)? {
                CallFormal::Int
            } else if name.matches_with(&[74], budget)? {
                CallFormal::Numeric(crate::p2_flow::primitives::ScalarType::Long)
            } else if name.matches_with(&[70], budget)? {
                CallFormal::Numeric(crate::p2_flow::primitives::ScalarType::Float)
            } else if name.matches_with(&[68], budget)? {
                CallFormal::Numeric(crate::p2_flow::primitives::ScalarType::Double)
            } else if name.matches_with(&[90], budget)? {
                CallFormal::Boolean
            } else if name.matches_with(&[66], budget)? {
                CallFormal::Narrow(Narrow::Byte)
            } else if name.matches_with(&[83], budget)? {
                CallFormal::Narrow(Narrow::Short)
            } else if name.matches_with(&[67], budget)? {
                CallFormal::Narrow(Narrow::Char)
            } else if name.matches_with(
                crate::substrate::primitive_types::BuiltinReference::Object.descriptor(),
                budget,
            )? {
                CallFormal::Reference(crate::substrate::primitive_types::BuiltinReference::Object)
            } else if name.matches_with(
                crate::substrate::primitive_types::BuiltinReference::String.descriptor(),
                budget,
            )? {
                CallFormal::Reference(crate::substrate::primitive_types::BuiltinReference::String)
            } else {
                supported = false;
                return Ok(());
            };
            push(&mut formals, formal, budget)
        },
    )?;
    if !supported {
        return Ok(None);
    }
    Ok(Some(formals))
}
pub(super) fn binding_available(
    input: &MethodInput<'_, '_>,
    catalog: &DeclarationCatalog,
    contract: &StaticContract,
    invocation: &super::throws::Invocation<'_>,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<bool, RequestError> {
    let Some(class) = catalog.class(&contract.owner, budget)? else {
        return Ok(false);
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
