use super::contracts::plain;
use crate::input::{MethodInput, RawMethodView};
use crate::raw::{LayoutKind, OperandReference, instruction_layout};
use crate::substrate::owned::{copy_proto, copy_units, reserve};
use crate::{CodeUnitOffset, DeclarationCatalog, MethodBudget, MethodId, RegisterId, RequestError};

#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) struct AllocationId(pub CodeUnitOffset);
pub(crate) struct ConstructorContract {
    _sealed: (),
    pub reference: MethodId,
    pub owner: Vec<u16>,
    pub prototype: Vec<u16>,
    pub int_arity: usize,
    pub declared_throws: Vec<Vec<u16>>,
}
pub(crate) struct AdjacentAllocation {
    _sealed: (),
    pub id: AllocationId,
    pub init: CodeUnitOffset,
    pub resume: CodeUnitOffset,
    pub aliases: Vec<RegisterId>,
    pub arguments: Vec<RegisterId>,
    pub receiver: RegisterId,
    pub contract: ConstructorContract,
}
#[derive(Clone, Copy)]
pub(crate) enum ConstructorUse {
    Allocation,
    Super,
}
pub(crate) fn resolve(
    input: &MethodInput<'_, '_>,
    catalog: &DeclarationCatalog,
    reference: MethodId,
    expected_owner: &[u16],
    use_kind: ConstructorUse,
    pc: CodeUnitOffset,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<ConstructorContract, RequestError> {
    let method = input.method_reference(reference, budget)?;
    if !method.owner.matches_with(expected_owner, budget)?
        || !method
            .declaration
            .name
            .matches_with(&[60, 105, 110, 105, 116, 62], budget)?
        || !method
            .declaration
            .prototype
            .return_type
            .matches_with(&[86], budget)?
    {
        return unavailable(pc);
    }
    let prototype = copy_proto(&method.declaration.prototype, budget)?;
    let caller = copy_units(input.owner(), budget)?;
    if matches!(use_kind, ConstructorUse::Super) {
        let Some(caller_class) = catalog.class(&caller, budget)? else {
            return Err(RequestError::BindingEvidenceMissing { reference, pc });
        };
        let mask = 1 | 0x10 | 0x200 | 0x400 | 0x1000 | 0x2000 | 0x4000;
        if caller_class.access_flags & mask != input.owner_access_flags() & mask
            || !plain(caller_class, &caller, budget)?
            || !input.plain_owner_scope(caller_class.parent.as_deref(), budget)?
            || !matches!(caller_class.parent.as_deref(),Some(parent) if crate::p0_knowledge::same_units(parent,expected_owner,budget)?)
        {
            return Err(RequestError::BindingEvidenceMissing { reference, pc });
        }
    }
    let Some(class) = catalog.class(expected_owner, budget)? else {
        return Err(RequestError::BindingEvidenceMissing { reference, pc });
    };
    if !plain(class, &caller, budget)?
        || class.access_flags & 0x6200 != 0
        || (matches!(use_kind, ConstructorUse::Allocation) && class.access_flags & 0x400 != 0)
        || !super::reference_types::source_type(input, catalog, expected_owner, budget)?
        || !super::overloads::unique_target(
            class,
            &[60, 105, 110, 105, 116, 62],
            &prototype,
            false,
            false,
            budget,
        )?
    {
        return Err(RequestError::BindingEvidenceMissing { reference, pc });
    }
    let mut target = None;
    for candidate in &class.methods {
        budget.charge_work(1)?;
        if crate::p0_knowledge::same_units(&candidate.name, &[60, 105, 110, 105, 116, 62], budget)?
            && crate::p0_knowledge::same_units(&candidate.prototype, &prototype, budget)?
        {
            if target.is_some() {
                return Err(RequestError::BindingEvidenceMissing { reference, pc });
            }
            target = Some(candidate);
        }
    }
    let Some(target) = target else {
        return Err(RequestError::BindingEvidenceMissing { reference, pc });
    };
    if target.access_flags & 9 != 1
        || target.access_flags & (0x100 | 0x400) != 0
        || target.generic
        || !super::throws::compatible(input, catalog, target.throws.as_deref(), budget)?
    {
        return Err(RequestError::BindingEvidenceMissing { reference, pc });
    }
    if let Some(original) = input.local_method(reference, budget)? {
        let mask = 1 | 2 | 4 | 8 | 0x10 | 0x40 | 0x80 | 0x100 | 0x400;
        let class_mask = 1 | 0x10 | 0x200 | 0x400 | 0x1000 | 0x2000 | 0x4000;
        if original.access_flags & mask != target.access_flags & mask
            || original.owner_access_flags & class_mask != class.access_flags & class_mask
            || !original.plain_scope
        {
            return Err(RequestError::BindingEvidenceMissing { reference, pc });
        }
        let parent = match (original.parent, class.parent.as_deref()) {
            (None, None) => true,
            (Some(a), Some(b)) => a.matches_with(b, budget)?,
            _ => false,
        };
        if !parent {
            return Err(RequestError::BindingEvidenceMissing { reference, pc });
        }
    }
    let mut int_arity = 0usize;
    let mut supported = true;
    method.declaration.prototype.visit_parameter_types(
        budget,
        |name, budget| -> Result<(), RequestError> {
            if !name.matches_with(&[73], budget)? {
                supported = false;
            }
            int_arity = int_arity
                .checked_add(1)
                .ok_or(RequestError::PlatformCapacity)?;
            Ok(())
        },
    )?;
    if !supported {
        return unavailable(pc);
    }

    if matches!(use_kind, ConstructorUse::Super) && class.access_flags & 0x10 != 0 {
        return Err(RequestError::BindingEvidenceMissing { reference, pc });
    }
    Ok(ConstructorContract {
        _sealed: (),
        reference,
        owner: copy_units(method.owner, budget)?,
        prototype,
        int_arity,
        declared_throws: super::throws::retain(target.throws.as_deref(), budget)?,
    })
}

fn unavailable<T>(pc: CodeUnitOffset) -> Result<T, RequestError> {
    Err(RequestError::InitializationUnavailable { pc })
}
pub(crate) fn pair(
    input: &MethodInput<'_, '_>,
    raw: &RawMethodView<'_>,
    start: CodeUnitOffset,
    catalog: &DeclarationCatalog,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<AdjacentAllocation, RequestError> {
    let first = instruction_layout(raw.instructions(), start, raw.version())
        .map_err(crate::input::MethodError::Decode)?;
    if !matches!(first.kind(),LayoutKind::Instruction(spec) if spec.opcode==0x22) {
        return Err(RequestError::InternalInvariant(
            "allocation pair lost new-instance",
        ));
    }
    let Some(reference) = first.references()[0] else {
        return Err(RequestError::InternalInvariant(
            "allocation lost type reference",
        ));
    };
    let OperandReference::Type(ty) = reference.target else {
        return Err(RequestError::InternalInvariant(
            "allocation changed reference kind",
        ));
    };
    let descriptor = copy_units(input.type_name(ty, budget)?, budget)?;
    let operands = crate::input::operands::instruction_operands(raw.instructions(), start, first)
        .map_err(crate::input::MethodError::Decode)?;
    let Some(destination) = operands.registers().iter().next() else {
        return Err(RequestError::InternalInvariant(
            "allocation lost destination",
        ));
    };
    let mut aliases = Vec::new();
    reserve(&mut aliases, 1, budget)?;
    aliases.push(destination);
    let mut pc = start
        .checked_advance(first.width())
        .ok_or(RequestError::PlatformCapacity)?;
    while pc.0 < raw.code_units().0 {
        budget.charge_work(1)?;
        let layout = instruction_layout(raw.instructions(), pc, raw.version())
            .map_err(crate::input::MethodError::Decode)?;
        let LayoutKind::Instruction(spec) = layout.kind() else {
            return unavailable(pc);
        };
        let operands = crate::input::operands::instruction_operands(raw.instructions(), pc, layout)
            .map_err(crate::input::MethodError::Decode)?;
        let mut words = operands.registers().iter();
        match spec.opcode {
            7..=9 => {
                let destination = words.next().ok_or(RequestError::InternalInvariant(
                    "uninit move lost destination",
                ))?;
                let source = words
                    .next()
                    .ok_or(RequestError::InternalInvariant("uninit move lost source"))?;
                let mut source_alias = false;
                let mut destination_alias = false;
                for &alias in &aliases {
                    budget.charge_work(1)?;
                    source_alias |= alias == source;
                    destination_alias |= alias == destination;
                }
                if !source_alias {
                    return unavailable(pc);
                }
                if !destination_alias {
                    reserve(&mut aliases, 1, budget)?;
                    aliases.push(destination);
                }
            }
            0x70 | 0x76 => {
                let receiver = words
                    .next()
                    .ok_or(RequestError::InternalInvariant("init lost receiver"))?;
                let mut receiver_alias = false;
                for &alias in &aliases {
                    budget.charge_work(1)?;
                    receiver_alias |= alias == receiver;
                }
                if !receiver_alias {
                    return unavailable(pc);
                }
                let Some(reference) = layout.references()[0] else {
                    return Err(RequestError::InternalInvariant(
                        "init lost method reference",
                    ));
                };
                let OperandReference::Method(reference) = reference.target else {
                    return Err(RequestError::InternalInvariant(
                        "init changed reference kind",
                    ));
                };
                let contract = resolve(
                    input,
                    catalog,
                    reference,
                    &descriptor,
                    ConstructorUse::Allocation,
                    pc,
                    budget,
                )?;
                let mut arguments = Vec::new();
                for register in words {
                    budget.charge_work(1)?;
                    for &alias in &aliases {
                        budget.charge_work(1)?;
                        if register == alias {
                            return unavailable(pc);
                        }
                    }
                    reserve(&mut arguments, 1, budget)?;
                    arguments.push(register);
                }
                if arguments.len() != contract.int_arity {
                    return Err(RequestError::InternalInvariant(
                        "checked constructor changed word arity",
                    ));
                }
                return Ok(AdjacentAllocation {
                    _sealed: (),
                    id: AllocationId(start),
                    init: pc,
                    resume: pc
                        .checked_advance(layout.width())
                        .ok_or(RequestError::PlatformCapacity)?,
                    aliases,
                    arguments,
                    receiver,
                    contract,
                });
            }
            _ => return unavailable(pc),
        }
        pc = pc
            .checked_advance(layout.width())
            .ok_or(RequestError::PlatformCapacity)?;
    }
    unavailable(start)
}
