use crate::ByteOffset;
use crate::input::catalog::{MemberEntry, MemberSelection};
use crate::input::{DexString, InputError};
use crate::pipeline::control::InputControl;

pub(crate) fn units(
    value: DexString<'_>,
    budget: &mut impl InputControl,
) -> Result<Vec<u16>, InputError> {
    let mut reader = value.reader()?;
    let mut result = Vec::new();
    while let Some(unit) = reader.next(budget)? {
        reserve_one(&mut result, budget)?;
        result.push(unit);
    }
    Ok(result)
}

fn push_unit(
    result: &mut Vec<u16>,
    unit: u16,
    budget: &mut impl InputControl,
) -> Result<(), InputError> {
    budget.charge_work(1)?;
    reserve_one(result, budget)?;
    result.push(unit);
    Ok(())
}

pub(crate) fn materialize(
    bytes: &[u8],
    header_offset: ByteOffset,
    class_def: crate::ClassDefId,
    budget: &mut impl InputControl,
) -> Result<Option<super::declaration_types::InputClassDeclaration>, InputError> {
    let container = crate::input::DexContainer::open_with(bytes, budget)?;
    let index = crate::input::ProgramIndex::from_container(container, header_offset, budget)?;
    let class = index.class_with(class_def, budget)?;
    let descriptor = units(class.descriptor, budget)?;
    let access_flags = class.access_flags;
    let context = index.class_context(class_def, budget)?;
    let parent = context
        .superclass?
        .map(|parent| units(parent, budget))
        .transpose()?;
    let mut interfaces = Vec::new();
    for word in context.interfaces?.as_chunks::<2>().0 {
        budget.charge_work(1)?;
        let id = crate::TypeId(u32::from(u16::from_le_bytes(*word)));
        reserve_one(&mut interfaces, budget)?;
        interfaces.push(units(index.type_name(id, budget)?, budget)?);
    }
    let mut issues = Vec::new();
    let mut methods = Vec::new();
    let method_visit = class.visit_members_with::<_, InputError>(
        budget,
        MemberSelection::AllMethods,
        |entry, budget| {
            let MemberEntry::Method(method) = entry else {
                return Ok(());
            };
            let declaration = match method.declaration {
                Ok(declaration) => declaration,
                Err(error) => {
                    record_issue(&mut issues, error, budget)?;
                    return Ok(());
                }
            };
            let mut parameters = Vec::new();
            declaration.prototype.visit_parameter_type_ids(
                budget,
                |_, name, budget| -> Result<(), InputError> {
                    reserve_one(&mut parameters, budget)?;
                    parameters.push(units(name, budget)?);
                    Ok(())
                },
            )?;
            let mut prototype = Vec::new();
            push_unit(&mut prototype, u16::from(b'('), budget)?;
            for parameter in parameters {
                for unit in parameter {
                    push_unit(&mut prototype, unit, budget)?;
                }
            }
            push_unit(&mut prototype, u16::from(b')'), budget)?;
            for unit in units(declaration.prototype.return_type, budget)? {
                push_unit(&mut prototype, unit, budget)?;
            }
            reserve_one(&mut methods, budget)?;
            methods.push(super::declaration_types::InputMethodDeclaration {
                name: units(declaration.name, budget)?,
                prototype,
                access_flags: method.access_flags,
                throws: None,
                generic: false,
                signature: None,
            });
            Ok(())
        },
    );
    if let Err(error) = method_visit {
        record_issue(&mut issues, error, budget)?;
    }
    let mut fields = Vec::new();
    let mut field_names = Vec::new();
    let field_visit = class.visit_members_with::<_, InputError>(
        budget,
        MemberSelection::Fields,
        |entry, budget| {
            let MemberEntry::Field(field) = entry else {
                return Ok(());
            };
            let declaration = match field.declaration {
                Ok(declaration) => declaration,
                Err(error) => {
                    record_issue(&mut issues, error, budget)?;
                    return Ok(());
                }
            };
            let name = units(declaration.name, budget)?;
            reserve_one(&mut field_names, budget)?;
            field_names.push(units(declaration.name, budget)?);
            reserve_one(&mut fields, budget)?;
            fields.push(super::InputFieldEvidence {
                owner: units(class.descriptor, budget)?,
                name,
                field_type: units(declaration.field_type, budget)?,
                access_flags: field.access_flags,
                constant_kind: if field.access_flags & crate::substrate::flags::FINAL == 0 {
                    super::JavaConstantKind::NonConstant
                } else {
                    super::JavaConstantKind::Unknown
                },
            });
            Ok(())
        },
    );
    if let Err(error) = field_visit {
        record_issue(&mut issues, error, budget)?;
    }
    let coverage = if issues.is_empty() {
        super::DeclarationCoverage::AllDeclarations
    } else {
        super::DeclarationCoverage::Partial
    };
    Ok(Some(super::declaration_types::InputClassDeclaration {
        descriptor,
        access_flags,
        parent,
        interfaces,
        methods,
        field_names,
        fields,
        issues,
        coverage,
        generic: false,
        enclosing_scope: false,
        signature: None,
    }))
}

fn record_issue(
    issues: &mut Vec<InputError>,
    error: InputError,
    budget: &mut impl InputControl,
) -> Result<(), InputError> {
    if !matches!(
        error.kind(),
        crate::FailureKind::Malformed | crate::FailureKind::Unsupported
    ) {
        return Err(error);
    }
    reserve_one(issues, budget)?;
    issues.push(error);
    Ok(())
}

fn reserve_one<T>(values: &mut Vec<T>, budget: &mut impl InputControl) -> Result<(), InputError> {
    if values.len() < values.capacity() {
        return Ok(());
    }
    let growth = values.capacity().max(1);
    let bytes = growth
        .checked_mul(std::mem::size_of::<T>())
        .and_then(|n| u64::try_from(n).ok())
        .ok_or(InputError::PlatformCapacity {
            offset_byte: ByteOffset(0),
        })?;
    budget.charge_allocation(bytes)?;
    values
        .try_reserve_exact(growth)
        .map_err(|_| InputError::Allocation {
            offset_byte: ByteOffset(0),
        })
}
