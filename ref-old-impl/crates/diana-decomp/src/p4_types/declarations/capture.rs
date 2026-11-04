use crate::context::Context;
use crate::input::MethodSource;
use crate::input::catalog::{MemberEntry, MemberSelection};
use crate::substrate::owned::{copy_units, push};
use crate::{FieldId, RequestError, RunContext};

pub(crate) struct CaptureDeclaration {
    field: FieldId,
    outer: Vec<u16>,
    name: Vec<u16>,
}
impl CaptureDeclaration {
    pub(crate) fn field(&self) -> FieldId {
        self.field
    }
    pub(crate) fn outer(&self) -> &[u16] {
        &self.outer
    }
    pub(crate) fn name(&self) -> &[u16] {
        &self.name
    }
}

pub(crate) fn collect(
    _permit: &crate::engine::SchedulePermit,
    input: &MethodSource<'_, '_>,
    ownership: &crate::Ownership,
    catalog: &Context,
    budget: &mut RunContext<'_>,
) -> Result<Option<CaptureDeclaration>, RequestError> {
    collect_impl(input, ownership, catalog, budget)
}
fn collect_impl(
    input: &MethodSource<'_, '_>,
    ownership: &crate::Ownership,
    catalog: &Context,
    budget: &mut RunContext<'_>,
) -> Result<Option<CaptureDeclaration>, RequestError> {
    let mut nested = None;
    for entry in ownership.nested() {
        budget.checkpoint()?;
        if entry.class() == input.class_id() && nested.replace(entry).is_some() {
            return Ok(None);
        }
    }
    let Some(nested) = nested else {
        return Ok(None);
    };
    if nested.access_flags() & crate::substrate::flags::STATIC != 0
        || !crate::substrate::flags::instance_constructor(input.access_flags())
    {
        return Ok(None);
    }
    let outer = copy_units(
        input
            .program()
            .class_with(nested.enclosing(), budget)?
            .descriptor,
        budget,
    )?;
    let owner = copy_units(input.owner(), budget)?;
    let Some(class) = catalog.class(&owner, budget)? else {
        return Ok(None);
    };
    let Some(enclosing) = catalog.class(&outer, budget)? else {
        return Ok(None);
    };
    let Some(object) = catalog.class(
        crate::p4_types::BuiltinReference::Object.descriptor(),
        budget,
    )?
    else {
        return Ok(None);
    };
    let mut object_constructor = false;
    for method in &object.methods {
        budget.checkpoint()?;
        if method.name == const { crate::substrate::owned::ascii_units(*b"<init>") }
            && method.prototype == const { crate::substrate::owned::ascii_units(*b"()V") }
        {
            if object_constructor
                || method.access_flags != crate::substrate::flags::PUBLIC
                || method.generic
                || !matches!(&method.throws, Some(throws) if throws.is_empty())
            {
                return Ok(None);
            }
            object_constructor = true;
        }
    }
    if !object_constructor || object.parent.is_some() {
        return Ok(None);
    }
    if !matches!(
        class.coverage,
        crate::input::DeclarationCoverage::AllDeclarations
    ) || !matches!(
        enclosing.coverage,
        crate::input::DeclarationCoverage::AllDeclarations
    ) || class.generic
        || enclosing.generic
        || !class.interfaces.is_empty()
        || class.access_flags
            & (1 | crate::substrate::flags::FINAL | crate::substrate::flags::ABSTRACT)
            != input.owner_access_flags()
        || !matches!(class.parent.as_deref(), Some(parent) if parent == crate::p4_types::BuiltinReference::Object.descriptor())
    {
        return Ok(None);
    }
    let mut prototype = Vec::new();
    push(&mut prototype, 40, budget)?;
    for &unit in &outer {
        push(&mut prototype, unit, budget)?;
    }
    push(&mut prototype, 41, budget)?;
    push(&mut prototype, 86, budget)?;
    if !input.declaration().name.matches_with(
        &const { crate::substrate::owned::ascii_units(*b"<init>") },
        budget,
    )? || crate::substrate::owned::copy_slice_units(
        input.declaration().prototype.units(),
        budget,
    )? != prototype
    {
        return Ok(None);
    }
    let mut constructors = 0usize;
    for method in &class.methods {
        budget.checkpoint()?;
        if method.name == const { crate::substrate::owned::ascii_units(*b"<init>") }
            && method.prototype == prototype
        {
            if method.generic
                || method.access_flags
                    & (crate::substrate::flags::PUBLIC
                        | crate::substrate::flags::PRIVATE
                        | crate::substrate::flags::PROTECTED)
                    != input.access_flags()
                        & (crate::substrate::flags::PUBLIC
                            | crate::substrate::flags::PRIVATE
                            | crate::substrate::flags::PROTECTED)
                || !matches!(&method.throws, Some(throws) if throws.is_empty())
            {
                return Ok(None);
            }
            constructors = constructors
                .checked_add(1)
                .ok_or(RequestError::PlatformCapacity)?;
        }
    }
    if constructors != 1 {
        return Ok(None);
    }
    let mut field = None;
    let mut valid = true;
    input
        .program()
        .class_with(input.class_id(), budget)?
        .visit_members_with(
            budget,
            MemberSelection::Fields,
            |entry, budget| -> Result<(), RequestError> {
                budget.checkpoint()?;
                let MemberEntry::Field(original) = entry else {
                    return Err(RequestError::InternalInvariant(
                        "capture field visitor returned method",
                    ));
                };
                if original.access_flags & crate::substrate::flags::SYNTHETIC == 0 {
                    return Ok(());
                }
                let declaration = original.declaration?;
                if !crate::substrate::flags::capture_field(original.access_flags)
                    || !declaration.field_type.matches_with(&outer, budget)?
                    || field.is_some()
                {
                    valid = false;
                    return Ok(());
                }
                let Some(evidence) = catalog.field(
                    input.owner(),
                    declaration.name,
                    declaration.field_type,
                    budget,
                )?
                else {
                    valid = false;
                    return Ok(());
                };
                if evidence.access_flags != original.access_flags {
                    valid = false;
                    return Ok(());
                }
                let name = copy_units(declaration.name, budget)?;
                let mut same_names = 0usize;
                for candidate in &class.field_names {
                    budget.checkpoint()?;
                    if crate::context::same_units(candidate, &name, budget)? {
                        same_names = same_names
                            .checked_add(1)
                            .ok_or(RequestError::PlatformCapacity)?;
                    }
                }
                if same_names != 1 {
                    valid = false;
                    return Ok(());
                }
                field = Some((original.field_idx, name));
                Ok(())
            },
        )?;
    if !valid {
        return Ok(None);
    }
    let Some((field, name)) = field else {
        return Ok(None);
    };
    Ok(Some(CaptureDeclaration { field, outer, name }))
}
