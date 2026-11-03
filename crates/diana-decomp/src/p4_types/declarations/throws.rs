use crate::context::Context;
use crate::input::MethodInput;
use crate::substrate::owned::{copy_units, reserve};
use crate::{MethodBudget, RequestError};

const OBJECT: &[u16] = &const { crate::substrate::owned::ascii_units(*b"Ljava/lang/Object;") };
const THROWABLE: &[u16] =
    &const { crate::substrate::owned::ascii_units(*b"Ljava/lang/Throwable;") };
const RUNTIME: &[u16] =
    &const { crate::substrate::owned::ascii_units(*b"Ljava/lang/RuntimeException;") };
const ERROR: &[u16] = &const { crate::substrate::owned::ascii_units(*b"Ljava/lang/Error;") };
fn same(a: &[u16], b: &[u16], budget: &mut MethodBudget<'_, '_>) -> Result<bool, RequestError> {
    Ok(crate::context::same_units(a, b, budget)?)
}
pub(crate) fn subtype(
    catalog: &Context,
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
        if class.access_flags & crate::substrate::flags::INTERFACE != 0 {
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
    catalog: &Context,
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
    // Unknown throws stay unknown; they do not suppress the readable call.
    let Some(declared) = declared else {
        return Ok(Vec::new());
    };
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
    catalog: &Context,
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
    _catalog: &Context,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<Option<SourceThrows>, RequestError> {
    let original = input.throws_metadata(budget)?;
    let mut descriptors = Vec::new();
    for exception in original.exceptions {
        budget.charge_work(1)?;
        reserve(&mut descriptors, 1, budget)?;
        descriptors.push(copy_units(exception, budget)?);
    }
    Ok(Some(SourceThrows {
        _sealed: (),
        descriptors,
    }))
}
#[allow(dead_code)]
pub(crate) struct Invocation<'a> {
    pub reference: crate::MethodId,
    pub pc: crate::CodeUnitOffset,
    pub protection: Option<&'a crate::input::protection::CheckedProtection>,
}

#[allow(dead_code)]
pub(crate) fn compatible(
    input: &MethodInput<'_, '_>,
    catalog: &Context,
    declared: Option<&[Vec<u16>]>,
    invocation: &Invocation<'_>,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<bool, RequestError> {
    // An unknown external throws clause is not a known violation.
    let Some(declared) = declared else {
        return Ok(true);
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
    pub(crate) fn retained(&self, budget: &mut MethodBudget<'_, '_>) -> Result<Self, RequestError> {
        Ok(Self {
            _sealed: (),
            descriptors: retain(Some(&self.descriptors), budget)?,
        })
    }
    pub(crate) fn covers_at(
        &self,
        input: &MethodInput<'_, '_>,
        catalog: &Context,
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
        catalog: &Context,
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
        catalog: &Context,
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
