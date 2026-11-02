use super::normal::{CallSite, Op, OpId, Operation};
use crate::input::{ProgramIndex, RawMethodView};
use crate::substrate::owned::push;
use crate::{ClassDefId, CodeUnitOffset, MethodBudget, MethodId, RequestError, TypeId};

#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum ParameterFamily {
    Word,
    Wide,
    Reference,
}
impl ParameterFamily {
    pub(crate) fn width(self) -> super::normal::ValueWidth {
        if self == Self::Wide {
            super::normal::ValueWidth::Two
        } else {
            super::normal::ValueWidth::One
        }
    }
}
crate::substrate::checked::checked_product!(EntryWindow, EntryWindowData);
pub(crate) struct EntryWindowData<'a> {
    _sealed: (),
    pub method: MethodId,
    pub owner: TypeId,
    pub raw: RawMethodView<'a>,
    pub parameters: Vec<ParameterFamily>,
    pub operations: Vec<Operation>,
    pub calls: Vec<CallSite>,
}
pub(crate) fn decode<'a>(
    index: &ProgramIndex<'a>,
    class: ClassDefId,
    method: MethodId,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<Option<EntryWindow<'a>>, RequestError> {
    let Some(envelope) = index.method_envelope(class, method, budget)? else {
        return Ok(None);
    };
    if envelope.access_flags & 0x10008 != 0x10000
        || !envelope
            .declaration
            .name
            .matches_with(&[60, 105, 110, 105, 116, 62], budget)?
        || !envelope
            .declaration
            .prototype
            .return_type
            .matches_with(&[86], budget)?
    {
        return Ok(None);
    }
    let Some(raw) = envelope.code else {
        return Ok(None);
    };
    if !raw.try_byte_range().as_range().is_empty() {
        return Ok(None);
    }
    let bad = || {
        RequestError::Input(crate::input::InputError::Malformed {
            code: crate::input::InputProblem::InvalidCode,
            offset_byte: raw.code_byte_range().start,
        })
    };
    if envelope
        .declaration
        .prototype
        .parameter_words
        .0
        .checked_add(1)
        != Some(u64::from(raw.ins_size()))
        || raw.ins_size() > raw.registers_size()
    {
        return Err(bad());
    }
    let mut parameters = Vec::new();
    push(&mut parameters, ParameterFamily::Reference, budget)?;
    envelope
        .declaration
        .prototype
        .visit_parameter_types(budget, |name, budget| {
            let family = match name.reader()?.next(budget)? {
                Some(74 | 68) => ParameterFamily::Wide,
                Some(76 | 91) => ParameterFamily::Reference,
                _ => ParameterFamily::Word,
            };
            push(&mut parameters, family, budget)
        })?;
    let mut operations = Vec::new();
    let mut calls = Vec::new();
    let mut pc = CodeUnitOffset(0);
    while pc.0 < raw.code_units().0 {
        budget.charge_work(1)?;
        let layout = crate::raw::instruction_layout(raw.instructions(), pc, raw.version())
            .map_err(crate::input::MethodError::Decode)?;
        let crate::raw::LayoutKind::Instruction(spec) = layout.kind() else {
            return Ok(None);
        };
        let operands = crate::input::operands::instruction_operands(raw.instructions(), pc, layout)
            .map_err(crate::input::MethodError::Decode)?;
        for register in operands.registers().iter() {
            budget.charge_work(1)?;
            if register.0 >= u32::from(raw.registers_size()) {
                return Err(bad());
            }
        }
        let op = if let Some(op) = super::local_ops::local(spec, operands)? {
            op
        } else {
            if matches!(spec.opcode, 0x70 | 0x76)
                && operands.registers().len() > usize::from(raw.outs_size())
            {
                return Err(bad());
            }
            let Some(op) = super::local_ops::initialization(
                index, pc, spec, layout, operands, &mut calls, budget,
            )?
            else {
                return Ok(None);
            };
            op
        };
        let id = OpId(u32::try_from(operations.len()).map_err(|_| RequestError::PlatformCapacity)?);
        push(&mut operations, Operation { id, pc, kind: op }, budget)?;
        if matches!(op, Op::Initialize(_)) {
            return Ok(Some(EntryWindow(EntryWindowData {
                _sealed: (),
                method: envelope.method,
                owner: envelope.owner,
                raw,
                parameters,
                operations,
                calls,
            })));
        }
        pc = pc
            .checked_advance(layout.width())
            .ok_or(RequestError::PlatformCapacity)?;
    }
    Ok(None)
}
