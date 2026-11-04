use crate::context::Context;
use crate::input::MethodSource;
use crate::{CodeUnitOffset, FieldId, RequestError, RunContext};

#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum FieldUse {
    Read,
    Write,
}
#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum FieldStorage {
    Instance,
    Static,
}
#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum AssignmentObligation {
    Ordinary,
    OwnFinalInitialization,
    OuterCapture,
}
/// The original field operation plus available declaration metadata. Missing
/// flags do not invent access, volatility or constant status.
pub(crate) struct FieldAccess {
    pub outer_capture: Option<Vec<u16>>,
    pub assignment: AssignmentObligation,
    pub kind: crate::p2_flow::DeclarationType,
    pub storage: FieldStorage,
    _sealed: (),
    pub reference: FieldId,
    pub owner_type: crate::TypeId,
    pub owner: Vec<u16>,
    pub name: Vec<u16>,
    pub usage: FieldUse,
    pub flags: Option<u32>,
}
pub(crate) fn collect_field(
    input: &MethodSource<'_, '_>,
    catalog: &Context,
    reference: FieldId,
    storage: FieldStorage,
    usage: FieldUse,
    pc: CodeUnitOffset,
    budget: &mut RunContext<'_>,
) -> Result<FieldAccess, RequestError> {
    let original = input.field_reference(reference, budget)?;
    let kind = crate::p2_flow::field_type(
        input,
        original.declaration.field_type,
        original.declaration.field_type_idx,
        budget,
    )?
    .ok_or(RequestError::FieldBindingEvidenceMissing { reference, pc })?;
    let flags = catalog
        .field(
            original.owner,
            original.declaration.name,
            original.declaration.field_type,
            budget,
        )?
        .map(|declaration| declaration.access_flags);
    let own_flags = input.own_field_metadata(reference, budget)?;
    let flags = own_flags.or(flags);
    let own = own_flags.is_some();
    let own_final = own
        && usage == FieldUse::Write
        && flags.is_some_and(|flags| flags & crate::substrate::flags::FINAL != 0)
        && match storage {
            FieldStorage::Instance => {
                crate::substrate::flags::instance_constructor(input.access_flags())
            }
            FieldStorage::Static => {
                crate::substrate::flags::class_initializer(input.access_flags())
            }
        };
    let capture_outer = if storage == FieldStorage::Instance
        && usage == FieldUse::Read
        && flags.is_some_and(crate::substrate::flags::capture_field)
        && let Some(scope) =
            super::nested_scope::collect(input.program(), input.class_id(), catalog, budget)?
        && original.owner.matches_with(scope.owner(), budget)?
        && original
            .declaration
            .field_type
            .matches_with(scope.outer(), budget)?
    {
        let mut outer = Vec::new();
        for &unit in scope.outer() {
            crate::substrate::owned::push(&mut outer, unit, budget)?;
        }
        Some(outer)
    } else {
        None
    };
    Ok(FieldAccess {
        outer_capture: capture_outer,
        assignment: if own_final {
            AssignmentObligation::OwnFinalInitialization
        } else {
            AssignmentObligation::Ordinary
        },
        kind,
        storage,
        _sealed: (),
        reference,
        owner_type: original.owner_type,
        owner: crate::substrate::owned::copy_units(original.owner, budget)?,
        name: crate::substrate::owned::copy_units(original.declaration.name, budget)?,
        usage,
        flags,
    })
}

pub(crate) fn collect_capture(
    input: &MethodSource<'_, '_>,
    catalog: &Context,
    reference: FieldId,
    pc: CodeUnitOffset,
    budget: &mut RunContext<'_>,
) -> Result<FieldAccess, RequestError> {
    let mut access = collect_field(
        input,
        catalog,
        reference,
        FieldStorage::Instance,
        FieldUse::Write,
        pc,
        budget,
    )?;
    if !access
        .flags
        .is_some_and(crate::substrate::flags::capture_field)
    {
        return Err(RequestError::FieldBindingEvidenceMissing { reference, pc });
    }
    access.assignment = AssignmentObligation::OuterCapture;
    Ok(access)
}

pub(crate) fn collect_own_write(
    input: &MethodSource<'_, '_>,
    catalog: &Context,
    reference: FieldId,
    pc: CodeUnitOffset,
    budget: &mut RunContext<'_>,
) -> Result<FieldAccess, RequestError> {
    collect_field(
        input,
        catalog,
        reference,
        FieldStorage::Instance,
        FieldUse::Write,
        pc,
        budget,
    )
}
