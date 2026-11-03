use crate::context::Context;
use crate::input::MethodInput;
use crate::{CodeUnitOffset, FieldId, JavaConstantKind, MethodBudget, RequestError};

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
    pub flags: u32,
}
pub(crate) fn collect_field(
    input: &MethodInput<'_, '_>,
    catalog: &Context,
    reference: FieldId,
    storage: FieldStorage,
    usage: FieldUse,
    pc: CodeUnitOffset,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<FieldAccess, RequestError> {
    let missing = || RequestError::FieldBindingEvidenceMissing { reference, pc };
    let original = input.field_reference(reference, budget)?;
    let Some(kind) = crate::p2_flow::field_type(
        input,
        original.declaration.field_type,
        original.declaration.field_type_idx,
        budget,
    )?
    else {
        return Err(missing());
    };
    let Some(evidence) = catalog.field(
        original.owner,
        original.declaration.name,
        original.declaration.field_type,
        budget,
    )?
    else {
        return Err(missing());
    };
    let final_initialization = usage == FieldUse::Write
        && evidence.access_flags & 16 != 0
        && storage == FieldStorage::Static
        && crate::substrate::flags::class_initializer(input.access_flags())
        && input
            .declaration()
            .name
            .matches_with(&[60, 99, 108, 105, 110, 105, 116, 62], budget)?
        && input.own_field_metadata(reference, budget)?.is_some();
    let nested = super::nested_scope::collect(input.program(), input.class_id(), catalog, budget)?;
    let capture_outer = if let Some(scope) = &nested {
        if evidence.owner == scope.owner()
            && evidence.access_flags == 0x1010
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
        }
    } else {
        None
    };
    let lexical = nested
        .as_ref()
        .is_some_and(|scope| evidence.owner == scope.owner() || evidence.owner == scope.outer());
    let own_private =
        evidence.access_flags & 7 == 2 && input.owner().matches_with(&evidence.owner, budget)?;
    if evidence.access_flags & 7 == 2 && !own_private
        || (evidence.access_flags & 8 != 0) != (storage == FieldStorage::Static)
        || evidence.access_flags & 7 != 1 && !lexical && !own_private
        || evidence.access_flags & !(1 | 2 | 4 | 8 | 16 | 64 | 128 | 0x1000) != 0
        || evidence.access_flags & (16 | 64) == (16 | 64)
        || usage == FieldUse::Write && evidence.access_flags & 16 != 0 && !final_initialization
    {
        return Err(missing());
    }
    match input.referenced_field_metadata(reference, budget)? {
        crate::input::ReferencedFieldMetadata::Declared(native)
            if native.access_flags == evidence.access_flags && native.same_named_fields == 1 => {}
        crate::input::ReferencedFieldMetadata::External => {}
        _ => return Err(missing()),
    }
    if usage == FieldUse::Read
        && evidence.access_flags & 16 != 0
        && (kind.field_offset() != 2 || kind == crate::p2_flow::DeclarationType::String)
    {
        match evidence.constant_kind {
            JavaConstantKind::Constant => {
                return Err(RequestError::JavaConstantFieldRead { reference, pc });
            }
            JavaConstantKind::Unknown => return Err(missing()),
            JavaConstantKind::NonConstant => {}
        }
    }
    let Some(owner) = catalog.class(&evidence.owner, budget)? else {
        return Err(missing());
    };
    let mut names = 0usize;
    for name in &owner.field_names {
        budget.charge_work(1)?;
        if crate::context::same_units(name, &evidence.name, budget)? {
            names = names.checked_add(1).ok_or(RequestError::PlatformCapacity)?;
        }
    }
    if names != 1 {
        return Err(missing());
    }
    Ok(FieldAccess {
        outer_capture: capture_outer,
        assignment: if final_initialization {
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
        flags: evidence.access_flags,
    })
}

pub(crate) fn collect_capture(
    input: &MethodInput<'_, '_>,
    catalog: &Context,
    reference: FieldId,
    pc: CodeUnitOffset,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<FieldAccess, RequestError> {
    let original = input.field_reference(reference, budget)?;
    let Some(kind) = crate::p2_flow::field_type(
        input,
        original.declaration.field_type,
        original.declaration.field_type_idx,
        budget,
    )?
    else {
        return Err(RequestError::FieldBindingEvidenceMissing { reference, pc });
    };
    let Some(evidence) = catalog.field(
        original.owner,
        original.declaration.name,
        original.declaration.field_type,
        budget,
    )?
    else {
        return Err(RequestError::FieldBindingEvidenceMissing { reference, pc });
    };
    if evidence.access_flags != 0x1010 {
        return Err(RequestError::FieldBindingEvidenceMissing { reference, pc });
    }
    Ok(FieldAccess {
        outer_capture: None,
        assignment: AssignmentObligation::OuterCapture,
        kind,
        storage: FieldStorage::Instance,
        _sealed: (),
        reference,
        owner_type: original.owner_type,
        owner: crate::substrate::owned::copy_units(original.owner, budget)?,
        name: crate::substrate::owned::copy_units(original.declaration.name, budget)?,
        usage: FieldUse::Write,
        flags: evidence.access_flags,
    })
}

pub(crate) fn collect_own_write(
    input: &MethodInput<'_, '_>,
    catalog: &Context,
    reference: FieldId,
    pc: CodeUnitOffset,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<FieldAccess, RequestError> {
    let original = input.field_reference(reference, budget)?;
    let write = super::fields::bind_own_int_write(input, catalog, reference, pc, budget)?;
    let native =
        input
            .own_field_metadata(reference, budget)?
            .ok_or(RequestError::InternalInvariant(
                "own field lost native declaration",
            ))?;
    Ok(FieldAccess {
        outer_capture: None,
        assignment: if native.access_flags & 16 != 0 {
            AssignmentObligation::OwnFinalInitialization
        } else {
            AssignmentObligation::Ordinary
        },
        kind: crate::p2_flow::DeclarationType::Int,
        _sealed: (),
        storage: FieldStorage::Instance,
        reference: write.reference,
        owner_type: original.owner_type,
        owner: crate::substrate::owned::copy_units(original.owner, budget)?,
        name: write.name,
        usage: FieldUse::Write,
        flags: native.access_flags,
    })
}
