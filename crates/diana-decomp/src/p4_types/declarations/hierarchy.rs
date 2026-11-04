use crate::RequestError;
use crate::context::Context;
use crate::input::InputClassDeclaration;
use crate::pipeline::control::InputControl;
use crate::substrate::owned::push;

fn lookup<'a>(
    catalog: &'a Context,
    name: &[u16],
    budget: &mut impl InputControl,
) -> Result<Option<&'a InputClassDeclaration>, RequestError> {
    if name.first() == Some(&76) && name.last() == Some(&59) {
        return Ok(catalog.class(name, budget)?);
    }
    // Classfile interface entries retain JVM internal names; original DEX and
    // prototype references use descriptors. Normalize only the lookup key.
    if name.is_empty() {
        return Ok(None);
    }
    let mut descriptor = Vec::new();
    push(&mut descriptor, 76, budget)?;
    for &unit in name {
        budget.checkpoint()?;
        if matches!(unit, 0 | 46 | 59 | 91) {
            return Ok(None);
        }
        push(&mut descriptor, unit, budget)?;
    }
    push(&mut descriptor, 59, budget)?;
    Ok(catalog.class(&descriptor, budget)?)
}
fn seen(
    classes: &[&InputClassDeclaration],
    class: &InputClassDeclaration,
    budget: &mut impl InputControl,
) -> Result<bool, RequestError> {
    for old in classes {
        budget.checkpoint()?;
        if crate::context::same_units(&old.descriptor, &class.descriptor, budget)? {
            return Ok(true);
        }
    }
    Ok(false)
}

pub(super) struct Hierarchy<'a> {
    declarations: Vec<&'a InputClassDeclaration>,
}
impl<'a> Hierarchy<'a> {
    pub(super) fn declarations(&self) -> &[&'a InputClassDeclaration] {
        &self.declarations
    }
}

pub(super) fn collect<'a>(
    catalog: &'a Context,
    root: &'a InputClassDeclaration,
    budget: &mut impl InputControl,
) -> Result<Option<Hierarchy<'a>>, RequestError> {
    let _span = crate::pipeline::phase_span("p4_hierarchy");
    let mut pending = Vec::new();
    let mut active = Vec::new();
    let mut complete = Vec::new();
    push(&mut pending, (root, false), budget)?;
    while let Some((class, exiting)) = pending.pop() {
        budget.checkpoint()?;
        if exiting {
            if active.pop().is_none() {
                return Err(RequestError::InternalInvariant(
                    "hierarchy lost active ancestor",
                ));
            }
            push(&mut complete, class, budget)?;
            continue;
        }
        if seen(&complete, class, budget)? {
            continue;
        }
        if seen(&active, class, budget)? {
            return Ok(None);
        }
        if !matches!(
            class.coverage,
            crate::input::DeclarationCoverage::AllDeclarations
        ) {
            return Ok(None);
        }
        push(&mut active, class, budget)?;
        push(&mut pending, (class, true), budget)?;
        let object = crate::substrate::primitive_types::BuiltinReference::Object.descriptor();
        if crate::context::same_units(&class.descriptor, object, budget)? {
            if class.parent.is_some() || !class.interfaces.is_empty() {
                return Ok(None);
            }
            continue;
        }
        let Some(parent) = class.parent.as_deref() else {
            return Ok(None);
        };
        let Some(parent) = lookup(catalog, parent, budget)? else {
            return Ok(None);
        };
        if parent.access_flags & crate::substrate::flags::INTERFACE != 0 {
            return Ok(None);
        }
        push(&mut pending, (parent, false), budget)?;
        for interface in &class.interfaces {
            budget.checkpoint()?;
            let Some(interface) = lookup(catalog, interface, budget)? else {
                return Ok(None);
            };
            if interface.access_flags & crate::substrate::flags::INTERFACE == 0 {
                return Ok(None);
            }
            push(&mut pending, (interface, false), budget)?;
        }
    }
    Ok(Some(Hierarchy {
        declarations: complete,
    }))
}

pub(super) fn subtype(
    catalog: &Context,
    from: &[u16],
    to: &[u16],
    budget: &mut impl InputControl,
) -> Result<Option<bool>, RequestError> {
    let Some(root) = lookup(catalog, from, budget)? else {
        return Ok(None);
    };
    let Some(classes) = collect(catalog, root, budget)? else {
        return Ok(None);
    };
    for class in classes.declarations() {
        budget.checkpoint()?;
        if crate::context::same_units(&class.descriptor, to, budget)? {
            return Ok(Some(true));
        }
    }
    Ok(Some(false))
}
