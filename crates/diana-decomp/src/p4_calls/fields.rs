use crate::input::MethodInput;
use crate::{
    CodeUnitOffset, DeclarationCatalog, FieldId, JavaConstantKind, MethodBudget, RequestError,
};

pub(crate) fn check_external_static_read(
    input: &MethodInput<'_, '_>,
    catalog: &DeclarationCatalog,
    reference: FieldId,
    pc: CodeUnitOffset,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<(), RequestError> {
    let original = input.field_reference(reference, budget)?;
    let missing = || RequestError::FieldBindingEvidenceMissing { reference, pc };
    let Some(field) = catalog.field(
        original.owner,
        original.declaration.name,
        original.declaration.field_type,
        budget,
    )?
    else {
        return Err(missing());
    };
    let Some(owner) = catalog.class(&field.owner, budget)? else {
        return Err(missing());
    };
    let mut declared = false;
    for name in &owner.field_names {
        budget.charge_work(1)?;
        if crate::p0_knowledge::same_units(name, &field.name, budget)? {
            declared = true;
        }
    }
    if !declared {
        return Err(missing());
    }
    if field.access_flags & 0x18 != 0x18 {
        return Ok(());
    }
    let mut reader = original.declaration.field_type.reader()?;
    let first = reader.next(budget)?;
    let primitive = matches!(first, Some(90 | 66 | 83 | 67 | 73 | 74 | 70 | 68))
        && reader.next(budget)?.is_none();
    let string = original.declaration.field_type.matches_with(
        &[
            76, 106, 97, 118, 97, 47, 108, 97, 110, 103, 47, 83, 116, 114, 105, 110, 103, 59,
        ],
        budget,
    )?;
    if !primitive && !string {
        return Ok(());
    }
    match field.constant_kind {
        JavaConstantKind::Constant => Err(RequestError::JavaConstantFieldRead { reference, pc }),
        JavaConstantKind::Unknown => Err(missing()),
        JavaConstantKind::NonConstant => Ok(()),
    }
}
