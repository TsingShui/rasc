use super::contracts::{clear_scope, plain};
use crate::input::MethodInput;
use crate::substrate::owned::{copy_proto, copy_units, reserve};
use crate::{DeclarationCatalog, MethodBudget, RequestError};

const OBJECT: &[u16] = &[
    76, 106, 97, 118, 97, 47, 108, 97, 110, 103, 47, 79, 98, 106, 101, 99, 116, 59,
];
const THROWABLE: &[u16] = &[
    76, 106, 97, 118, 97, 47, 108, 97, 110, 103, 47, 84, 104, 114, 111, 119, 97, 98, 108, 101, 59,
];
const RUNTIME: &[u16] = &[
    76, 106, 97, 118, 97, 47, 108, 97, 110, 103, 47, 82, 117, 110, 116, 105, 109, 101, 69, 120, 99,
    101, 112, 116, 105, 111, 110, 59,
];
const ERROR: &[u16] = &[
    76, 106, 97, 118, 97, 47, 108, 97, 110, 103, 47, 69, 114, 114, 111, 114, 59,
];
fn same(a: &[u16], b: &[u16], budget: &mut MethodBudget<'_, '_>) -> Result<bool, RequestError> {
    Ok(crate::p0_knowledge::same_units(a, b, budget)?)
}
pub(crate) fn subtype(
    catalog: &DeclarationCatalog,
    from: &[u16],
    target: &[u16],
    budget: &mut MethodBudget<'_, '_>,
) -> Result<Option<bool>, RequestError> {
    let mut current = from;
    let mut seen: Vec<&[u16]> = Vec::new();
    loop {
        budget.charge_work(1)?;
        for old in &seen {
            budget.charge_work(1)?;
            if same(old, current, budget)? {
                return Ok(None);
            }
        }
        let Some(class) = catalog.class(current, budget)? else {
            return Ok(None);
        };
        if class.access_flags & 0x200 != 0 {
            return Ok(None);
        }
        if same(current, target, budget)? {
            return Ok(Some(true));
        }
        reserve(&mut seen, 1, budget)?;
        seen.push(current);
        let Some(parent) = class.parent.as_deref() else {
            return Ok(if same(current, OBJECT, budget)? {
                Some(false)
            } else {
                None
            });
        };
        current = parent;
    }
}
fn contains(
    catalog: &DeclarationCatalog,
    from: &[u16],
    to: &[u16],
    budget: &mut MethodBudget<'_, '_>,
) -> Result<bool, RequestError> {
    Ok(subtype(catalog, from, to, budget)? == Some(true))
}
pub(super) fn retain(
    declared: Option<&[Vec<u16>]>,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<Vec<Vec<u16>>, RequestError> {
    let declared = declared.ok_or(RequestError::InternalInvariant(
        "resolved call lost known throws",
    ))?;
    let mut result = Vec::new();
    for descriptor in declared {
        budget.charge_work(1)?;
        let mut copy = Vec::new();
        reserve(&mut copy, descriptor.len(), budget)?;
        for &unit in descriptor {
            budget.charge_work(1)?;
            copy.push(unit);
        }
        reserve(&mut result, 1, budget)?;
        result.push(copy);
    }
    Ok(result)
}
pub(crate) fn unrestricted_catch(
    catalog: &DeclarationCatalog,
    name: &[u16],
    budget: &mut MethodBudget<'_, '_>,
) -> Result<bool, RequestError> {
    Ok(contains(catalog, name, RUNTIME, budget)?
        || contains(catalog, name, ERROR, budget)?
        || contains(catalog, RUNTIME, name, budget)?
        || contains(catalog, ERROR, name, budget)?)
}

pub(crate) struct SourceThrows {
    _sealed: (),
    pub descriptors: Vec<Vec<u16>>,
}
pub(crate) fn source_throws(
    input: &MethodInput<'_, '_>,
    catalog: &DeclarationCatalog,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<Option<SourceThrows>, RequestError> {
    let original = input.throws_metadata(budget)?;
    if original.other_annotations {
        return Ok(None);
    }
    let mut descriptors = Vec::new();
    for exception in original.exceptions {
        budget.charge_work(1)?;
        reserve(&mut descriptors, 1, budget)?;
        descriptors.push(copy_units(exception, budget)?);
    }
    if descriptors.is_empty() {
        return Ok(Some(SourceThrows {
            _sealed: (),
            descriptors,
        }));
    }
    if input.access_flags() & 8 == 0 {
        return Ok(None);
    }
    let caller = copy_units(input.owner(), budget)?;
    let Some(class) = catalog.class(&caller, budget)? else {
        return Ok(None);
    };
    if !plain(class, &caller, budget)?
        || !matches!(class.parent.as_deref(),Some(parent) if same(parent,OBJECT,budget)?)
        || !input.plain_owner_scope(class.parent.as_deref(), budget)?
    {
        return Ok(None);
    }
    let name = copy_units(input.declaration().name, budget)?;
    let prototype = copy_proto(&input.declaration().prototype, budget)?;
    let mut declaration = None;
    for method in &class.methods {
        budget.charge_work(1)?;
        if same(&method.name, &name, budget)? && same(&method.prototype, &prototype, budget)? {
            if declaration.is_some() {
                return Ok(None);
            }
            declaration = Some(method);
        }
    }
    let Some(declaration) = declaration else {
        return Ok(None);
    };
    let Some(declared) = &declaration.throws else {
        return Ok(None);
    };
    if declaration.generic
        || declaration.access_flags & 0x1f != input.access_flags() & 0x1f
        || declared.len() != descriptors.len()
    {
        return Ok(None);
    }
    for (actual, expected) in descriptors.iter().zip(declared) {
        budget.charge_work(1)?;
        if !same(actual, expected, budget)? {
            return Ok(None);
        }
    }
    let Some(object) = catalog.class(OBJECT, budget)? else {
        return Ok(None);
    };
    if !plain(object, &caller, budget)? || object.parent.is_some() {
        return Ok(None);
    }
    for method in &object.methods {
        budget.charge_work(1)?;
        if same(&method.name, &name, budget)? {
            return Ok(None);
        }
    }
    for descriptor in &descriptors {
        budget.charge_work(1)?;
        let Some(class) = catalog.class(descriptor, budget)? else {
            return Ok(None);
        };
        if descriptor.len() < 3
            || descriptor.first() != Some(&76)
            || descriptor.last() != Some(&59)
            || class.access_flags & 1 == 0
            || class.generic
            || class.enclosing_scope
            || !contains(catalog, descriptor, THROWABLE, budget)?
        {
            return Ok(None);
        }
        let mut end = descriptor.len() - 1;
        for (i, &unit) in descriptor[1..descriptor.len() - 1].iter().enumerate() {
            budget.charge_work(1)?;
            if unit == 47 {
                end = i + 1;
                break;
            }
        }
        if input.qualifier_shadowed(&descriptor[1..end], budget)?
            || !clear_scope(catalog, &caller, &descriptor[1..end], budget)?
        {
            return Ok(None);
        }
    }
    Ok(Some(SourceThrows {
        _sealed: (),
        descriptors,
    }))
}
pub(crate) struct Invocation<'a> {
    pub reference: crate::MethodId,
    pub pc: crate::CodeUnitOffset,
    pub protection: Option<&'a crate::input::protection::CheckedProtection>,
}

pub(crate) fn compatible(
    input: &MethodInput<'_, '_>,
    catalog: &DeclarationCatalog,
    declared: Option<&[Vec<u16>]>,
    invocation: &Invocation<'_>,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<bool, RequestError> {
    let Some(declared) = declared else {
        return Ok(false);
    };
    if declared.is_empty() {
        return Ok(true);
    }
    let Some(allowed) = source_throws(input, catalog, budget)? else {
        return Ok(false);
    };
    match invocation.protection {
        Some(protection) => {
            allowed.covers_at(input, catalog, declared, protection, invocation.pc, budget)
        }
        None => allowed.covers(catalog, declared, budget),
    }
}
impl SourceThrows {
    pub(crate) fn covers_at(
        &self,
        input: &MethodInput<'_, '_>,
        catalog: &DeclarationCatalog,
        declared: &[Vec<u16>],
        protection: &crate::input::protection::CheckedProtection,
        pc: crate::CodeUnitOffset,
        budget: &mut MethodBudget<'_, '_>,
    ) -> Result<bool, RequestError> {
        for exception in declared {
            budget.charge_work(1)?;
            if self.covers_exception(catalog, exception, budget)? {
                continue;
            }
            let mut caught = false;
            if let Some(chain) = protection.at(pc, budget)? {
                for clause in chain {
                    budget.charge_work(1)?;
                    match clause.kind {
                        crate::input::protection::CatchType::All => {
                            caught = contains(catalog, exception, THROWABLE, budget)?;
                        }
                        crate::input::protection::CatchType::Typed(id) => {
                            let target = copy_units(input.type_descriptor(id, budget)?, budget)?;
                            caught = contains(catalog, exception, &target, budget)?;
                        }
                    }
                    if caught {
                        break;
                    }
                }
            }
            if !caught {
                return Ok(false);
            }
        }
        Ok(true)
    }
    pub(crate) fn covers(
        &self,
        catalog: &DeclarationCatalog,
        declared: &[Vec<u16>],
        budget: &mut MethodBudget<'_, '_>,
    ) -> Result<bool, RequestError> {
        for exception in declared {
            budget.charge_work(1)?;
            if !self.covers_exception(catalog, exception, budget)? {
                return Ok(false);
            }
        }
        Ok(true)
    }
    pub(crate) fn covers_exception(
        &self,
        catalog: &DeclarationCatalog,
        exception: &[u16],
        budget: &mut MethodBudget<'_, '_>,
    ) -> Result<bool, RequestError> {
        if !contains(catalog, exception, THROWABLE, budget)? {
            return Ok(false);
        }
        if contains(catalog, exception, RUNTIME, budget)?
            || contains(catalog, exception, ERROR, budget)?
        {
            return Ok(true);
        }
        for target in &self.descriptors {
            budget.charge_work(1)?;
            if contains(catalog, exception, target, budget)? {
                return Ok(true);
            }
        }
        Ok(false)
    }
}
