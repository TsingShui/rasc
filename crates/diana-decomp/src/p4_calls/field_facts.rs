use crate::input::MethodInput;
use crate::{
    CodeUnitOffset, DeclarationCatalog, FieldId, JavaConstantKind, MethodBudget, RequestError,
};

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
pub(crate) struct IntFieldAccess {
    pub storage: FieldStorage,
    _sealed: (),
    pub reference: FieldId,
    pub owner_type: crate::TypeId,
    pub owner: Vec<u16>,
    pub name: Vec<u16>,
    pub usage: FieldUse,
    pub flags: u32,
}
pub(crate) fn collect_int_field(
    input: &MethodInput<'_, '_>,
    catalog: &DeclarationCatalog,
    reference: FieldId,
    storage: FieldStorage,
    usage: FieldUse,
    pc: CodeUnitOffset,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<IntFieldAccess, RequestError> {
    let missing = || RequestError::FieldBindingEvidenceMissing { reference, pc };
    let original = input.field_reference(reference, budget)?;
    if !original
        .declaration
        .field_type
        .matches_with(&[73], budget)?
    {
        return Err(missing());
    }
    let Some(evidence) = catalog.field(
        original.owner,
        original.declaration.name,
        original.declaration.field_type,
        budget,
    )?
    else {
        return Err(missing());
    };
    if (evidence.access_flags & 8 != 0) != (storage == FieldStorage::Static)
        || evidence.access_flags & 7 != 1
        || evidence.access_flags & !(1 | 8 | 16 | 64 | 128 | 0x1000) != 0
        || evidence.access_flags & (16 | 64) == (16 | 64)
        || usage == FieldUse::Write && evidence.access_flags & 16 != 0
    {
        return Err(missing());
    }
    match input.referenced_field_metadata(reference, budget)? {
        crate::input::ReferencedFieldMetadata::Declared(native)
            if native.access_flags == evidence.access_flags && native.same_named_fields == 1 => {}
        crate::input::ReferencedFieldMetadata::External => {}
        _ => return Err(missing()),
    }
    if !crate::p4_calls::source_type(input, catalog, &evidence.owner, budget)? {
        return Err(missing());
    }
    let Some(owner) = catalog.class(&evidence.owner, budget)? else {
        return Err(missing());
    };
    let mut names = 0usize;
    for name in &owner.field_names {
        budget.charge_work(1)?;
        if crate::p0_knowledge::same_units(name, &evidence.name, budget)? {
            names = names.checked_add(1).ok_or(RequestError::PlatformCapacity)?;
        }
    }
    if names != 1 {
        return Err(missing());
    }
    if usage == FieldUse::Read && evidence.access_flags & 16 != 0 {
        match evidence.constant_kind {
            JavaConstantKind::Constant => {
                return Err(RequestError::JavaConstantFieldRead { reference, pc });
            }
            JavaConstantKind::Unknown => return Err(missing()),
            JavaConstantKind::NonConstant => {}
        }
    }
    Ok(IntFieldAccess {
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

pub(crate) fn collect_own_write(
    input: &MethodInput<'_, '_>,
    catalog: &DeclarationCatalog,
    reference: FieldId,
    pc: CodeUnitOffset,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<IntFieldAccess, RequestError> {
    let original = input.field_reference(reference, budget)?;
    let write = super::fields::bind_own_int_write(input, catalog, reference, pc, budget)?;
    let native =
        input
            .own_field_metadata(reference, budget)?
            .ok_or(RequestError::InternalInvariant(
                "own field lost native declaration",
            ))?;
    Ok(IntFieldAccess {
        _sealed: (),
        storage: FieldStorage::Instance,
        reference: write.reference,
        owner_type: original.owner_type,
        owner: write.owner,
        name: write.name,
        usage: FieldUse::Write,
        flags: native.access_flags,
    })
}
