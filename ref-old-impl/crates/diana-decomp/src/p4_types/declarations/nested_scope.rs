use crate::context::Context;
use crate::input::ProgramIndex;
use crate::pipeline::control::InputControl;
use crate::substrate::owned::{copy_units, push};
use crate::{ClassDefId, RequestError};

pub(crate) struct Scope {
    owner: Vec<u16>,
    outer: Vec<u16>,
    name: Vec<u16>,
    flags: u32,
}
impl Scope {
    pub(crate) fn owner(&self) -> &[u16] {
        &self.owner
    }
    pub(crate) fn outer(&self) -> &[u16] {
        &self.outer
    }
    pub(crate) fn flags(&self) -> u32 {
        self.flags
    }
    pub(crate) fn name(&self) -> &[u16] {
        &self.name
    }
}
pub(crate) fn collect(
    index: &ProgramIndex<'_>,
    id: ClassDefId,
    catalog: &Context,
    control: &mut impl InputControl,
) -> Result<Option<Scope>, RequestError> {
    let ownership = index.ownership_cached(control)?;
    ownership.check_class(id)?;
    for entry in ownership.nested() {
        control.checkpoint()?;
        if entry.class() != id {
            continue;
        }
        let owner = copy_units(index.class_with(id, control)?.descriptor, control)?;
        let outer = copy_units(
            index.class_with(entry.enclosing(), control)?.descriptor,
            control,
        )?;
        let Some(class) = catalog.class(&owner, control)? else {
            return Ok(None);
        };
        let Some(enclosing) = catalog.class(&outer, control)? else {
            return Ok(None);
        };
        if !matches!(
            class.coverage,
            crate::input::DeclarationCoverage::AllDeclarations
        ) || !matches!(
            enclosing.coverage,
            crate::input::DeclarationCoverage::AllDeclarations
        ) || class.generic
            || enclosing.generic
            || !class.interfaces.is_empty()
            || !enclosing.interfaces.is_empty()
            || entry.access_flags()
                & (crate::substrate::flags::PUBLIC
                    | crate::substrate::flags::PRIVATE
                    | crate::substrate::flags::PROTECTED)
                != crate::substrate::flags::PUBLIC
        {
            return Ok(None);
        }
        let mut name = Vec::new();
        for &unit in entry.name() {
            push(&mut name, unit, control)?;
        }
        let mut expected = Vec::new();
        for &unit in &outer[..outer.len() - 1] {
            push(&mut expected, unit, control)?;
        }
        push(&mut expected, 36, control)?;
        for &unit in &name {
            push(&mut expected, unit, control)?;
        }
        push(&mut expected, 59, control)?;
        if !crate::context::same_units(&owner, &expected, control)? {
            return Ok(None);
        }
        return Ok(Some(Scope {
            owner,
            outer,
            name,
            flags: entry.access_flags(),
        }));
    }
    Ok(None)
}
pub(super) fn same_nest(
    input: &crate::input::MethodSource<'_, '_>,
    target: &[u16],
    budget: &mut crate::RunContext<'_>,
) -> Result<bool, RequestError> {
    let ownership = input.program().ownership_cached(budget)?;
    ownership.check_class(input.class_id())?;
    let mut owner = None;
    for entry in ownership.nested() {
        for class in [entry.class(), entry.enclosing()] {
            budget.checkpoint()?;
            if input
                .program()
                .class_with(class, budget)?
                .descriptor
                .matches_with(target, budget)?
            {
                owner = Some(class);
            }
        }
    }
    let Some(target) = owner else {
        return Ok(false);
    };
    ownership.check_class(target)?;
    for entry in ownership.nested() {
        budget.checkpoint()?;
        if entry.class() == target {
            return Ok(false);
        }
    }
    let context = input.program().class_context(target, budget)?;
    if !context.interfaces?.is_empty()
        || !input
            .program()
            .only_ownership_annotations(context.annotations_offset, budget)?
    {
        return Ok(false);
    }
    let mut roots = [input.class_id(), target];
    for root in &mut roots {
        for _ in 0..ownership.nested().len() {
            let mut parent = None;
            for entry in ownership.nested() {
                budget.checkpoint()?;
                if entry.class() == *root {
                    parent = Some(entry.enclosing());
                    break;
                }
            }
            let Some(parent) = parent else { break };
            *root = parent;
        }
    }
    Ok(roots[0] == roots[1])
}

pub(super) fn plain_owner(
    input: &crate::input::MethodSource<'_, '_>,
    catalog: &Context,
    parent: Option<&[u16]>,
    budget: &mut crate::RunContext<'_>,
) -> Result<bool, RequestError> {
    if input.plain_owner_scope(parent, budget)? {
        return Ok(true);
    }
    let ownership = input.program().ownership_cached(budget)?;
    ownership.check_class(input.class_id())?;
    let mut related = false;
    for entry in ownership.nested() {
        budget.checkpoint()?;
        related |= entry.class() == input.class_id() || entry.enclosing() == input.class_id();
    }
    if !related {
        return Ok(false);
    }
    let owner = copy_units(input.owner(), budget)?;
    let Some(class) = catalog.class(&owner, budget)? else {
        return Ok(false);
    };
    if class.generic
        || !matches!(
            class.coverage,
            crate::input::DeclarationCoverage::AllDeclarations
        )
    {
        return Ok(false);
    }
    let context = input.program().class_context(input.class_id(), budget)?;
    if !context.interfaces?.is_empty() {
        return Ok(false);
    }
    Ok(
        matches!((context.superclass?, parent), (Some(actual), Some(expected)) if actual.matches_with(expected, budget)?),
    )
}
