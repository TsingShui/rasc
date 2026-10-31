use super::catalog::{MemberEntry, MemberSelection};
use super::{InputProblem, MethodBody, ProgramIndex, malformed};
use crate::ClassDefId;
use crate::raw::{LayoutKind, OperandReference, instruction_layout};
use crate::substrate::owned::reserve;
use crate::{CodeUnitOffset, MethodBudget, MethodId, RequestError};

#[derive(Clone, Copy)]
pub(crate) enum ConstructorLinkKind {
    This,
    Super,
}
/// A link from only the bounded pure prologue, not validation of the later body.
pub(crate) struct ConstructorLink {
    _sealed: (),
    pub target: MethodId,
    pub pc: CodeUnitOffset,
    pub kind: ConstructorLinkKind,
}
#[derive(Clone, Copy)]
enum Root {
    Undefined,
    This,
    Int,
}
impl ProgramIndex<'_> {
    pub(crate) fn constructor_link(
        &self,
        class_id: ClassDefId,
        method_id: MethodId,
        budget: &mut MethodBudget<'_, '_>,
    ) -> Result<Option<ConstructorLink>, RequestError> {
        let class = self.class_with(class_id, budget)?;
        let mut selected = None;
        class.visit_members_with(
            budget,
            MemberSelection::Method(method_id),
            |entry, _| -> Result<(), RequestError> {
                if let MemberEntry::Method(method) = entry {
                    if selected.is_some() {
                        return Err(malformed(
                            InputProblem::InvalidClassData,
                            self.header.class_defs.offset_byte,
                        )
                        .into());
                    }
                    selected = Some(method);
                }
                Ok(())
            },
        )?;
        let Some(method) = selected else {
            return Ok(None);
        };
        let declaration = method.declaration?;
        if method.access_flags & 0x10008 != 0x10000
            || !declaration
                .name
                .matches_with(&[60, 105, 110, 105, 116, 62], budget)?
            || !declaration
                .prototype
                .return_type
                .matches_with(&[86], budget)?
        {
            return Ok(None);
        }
        let MethodBody::Code(code) = method.body? else {
            return Ok(None);
        };
        if !code.try_byte_range().as_range().is_empty() {
            return Ok(None);
        }
        let mut parameters = 0u32;
        let mut all_int = true;
        declaration.prototype.visit_parameter_types(
            budget,
            |name, budget| -> Result<(), RequestError> {
                all_int &= name.matches_with(&[73], budget)?;
                parameters = parameters
                    .checked_add(1)
                    .ok_or(RequestError::PlatformCapacity)?;
                Ok(())
            },
        )?;
        if !all_int {
            return Ok(None);
        }
        if parameters.checked_add(1) != Some(u32::from(code.ins_size()))
            || code.ins_size() > code.registers_size()
        {
            return Err(malformed(InputProblem::InvalidCode, code.code_byte_range().start).into());
        }
        let mut slots = Vec::new();
        reserve(&mut slots, usize::from(code.registers_size()), budget)?;
        for _ in 0..code.registers_size() {
            budget.charge_work(1)?;
            slots.push(Root::Undefined);
        }
        let entry = usize::from(code.registers_size() - code.ins_size());
        slots[entry] = Root::This;
        for parameter in 0..parameters {
            budget.charge_work(1)?;
            let at = entry
                .checked_add(1)
                .and_then(|v| v.checked_add(usize::try_from(parameter).ok()?))
                .ok_or(RequestError::PlatformCapacity)?;
            slots[at] = Root::Int;
        }
        let mut pc = CodeUnitOffset(0);
        while pc.0 < code.code_units().0 {
            budget.charge_work(1)?;
            let layout = instruction_layout(code.instructions(), pc, code.version())
                .map_err(super::MethodError::Decode)?;
            let LayoutKind::Instruction(spec) = layout.kind() else {
                return Ok(None);
            };
            let operands = super::operands::instruction_operands(code.instructions(), pc, layout)
                .map_err(super::MethodError::Decode)?;
            for register in operands.registers().iter() {
                budget.charge_work(1)?;
                if register.0 >= u32::from(code.registers_size()) {
                    return Err(
                        malformed(InputProblem::InvalidCode, code.code_byte_range().start).into(),
                    );
                }
            }
            let reg = |i| -> Result<usize, RequestError> {
                usize::try_from(
                    operands
                        .registers()
                        .iter()
                        .nth(i)
                        .ok_or(RequestError::InternalInvariant(
                            "constructor prefix lost checked operand",
                        ))?
                        .0,
                )
                .map_err(|_| RequestError::PlatformCapacity)
            };
            match spec.opcode {
                0 => {}
                1..=3 => {
                    if !matches!(slots[reg(1)?], Root::Int) {
                        return Ok(None);
                    }
                    slots[reg(0)?] = Root::Int;
                }
                7..=9 => {
                    if !matches!(slots[reg(1)?], Root::This) {
                        return Ok(None);
                    }
                    slots[reg(0)?] = Root::This;
                }
                0x12..=0x15 => slots[reg(0)?] = Root::Int,
                0x70 | 0x76 => {
                    if !matches!(slots[reg(0)?], Root::This) {
                        return Ok(None);
                    }
                    let Some(reference) = layout.references()[0] else {
                        return Err(RequestError::InternalInvariant(
                            "constructor prefix lost reference",
                        ));
                    };
                    let OperandReference::Method(target) = reference.target else {
                        return Err(RequestError::InternalInvariant(
                            "constructor prefix changed reference kind",
                        ));
                    };
                    let method = self.method_reference(target, budget)?;
                    if !method
                        .declaration
                        .name
                        .matches_with(&[60, 105, 110, 105, 116, 62], budget)?
                        || !method
                            .declaration
                            .prototype
                            .return_type
                            .matches_with(&[86], budget)?
                    {
                        return Ok(None);
                    }
                    let mut expected = 1u32;
                    let mut compatible = true;
                    method.declaration.prototype.visit_parameter_types(
                        budget,
                        |name, budget| -> Result<(), RequestError> {
                            compatible &= name.matches_with(&[73], budget)?;
                            expected = expected
                                .checked_add(1)
                                .ok_or(RequestError::PlatformCapacity)?;
                            Ok(())
                        },
                    )?;
                    if !compatible {
                        return Ok(None);
                    }
                    let mut actual = 0u32;
                    for register in operands.registers().iter() {
                        budget.charge_work(1)?;
                        if actual != 0
                            && !matches!(
                                slots[usize::try_from(register.0)
                                    .map_err(|_| RequestError::PlatformCapacity)?],
                                Root::Int
                            )
                        {
                            return Ok(None);
                        }
                        actual = actual
                            .checked_add(1)
                            .ok_or(RequestError::PlatformCapacity)?;
                    }
                    if expected != actual || actual > u32::from(code.outs_size()) {
                        return Err(malformed(
                            InputProblem::InvalidCode,
                            code.code_byte_range().start,
                        )
                        .into());
                    }
                    let kind = if method.owner.matches_with(
                        &crate::substrate::owned::copy_units(class.descriptor, budget)?,
                        budget,
                    )? {
                        ConstructorLinkKind::This
                    } else {
                        let context = self.class_context(class_id, budget)?;
                        let Some(parent) = context.superclass? else {
                            return Ok(None);
                        };
                        let parent = crate::substrate::owned::copy_units(parent, budget)?;
                        if !method.owner.matches_with(&parent, budget)? {
                            return Ok(None);
                        }
                        ConstructorLinkKind::Super
                    };
                    return Ok(Some(ConstructorLink {
                        _sealed: (),
                        target,
                        pc,
                        kind,
                    }));
                }
                _ => return Ok(None),
            }
            pc = pc
                .checked_advance(layout.width())
                .ok_or(RequestError::PlatformCapacity)?;
        }
        Ok(None)
    }
}
