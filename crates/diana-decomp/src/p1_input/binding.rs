use super::formats::{InsnLayout, OpcodeSpec, OperandReference, RegisterShape};
use super::operands::RawOperands;
use super::structure::bad;
use super::{MethodError, MethodProblem, ProgramIndex};
use crate::{CodeUnitOffset, MethodBudget};

#[derive(Clone, Copy)]
pub(super) enum ResultKind {
    Void,
    Scalar,
    Wide,
    Object,
}

impl ResultKind {
    pub(super) fn accepts(self, opcode: u8) -> bool {
        matches!(
            (self, opcode),
            (Self::Scalar, 0x0a) | (Self::Wide, 0x0b) | (Self::Object, 0x0c)
        )
    }
}

/// Checks the 035 invoke families against original prototypes, without resolving
/// dispatch or assigning types to register values. All reads use the normal
/// method ledger; borrowed iterators require no scratch allocation.
pub(super) fn check_producer(
    index: &ProgramIndex<'_>,
    pc: CodeUnitOffset,
    spec: OpcodeSpec,
    layout: InsnLayout,
    operands: RawOperands,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<Option<ResultKind>, MethodError> {
    if spec.registers == RegisterShape::ArrayWords {
        return Ok(Some(ResultKind::Object));
    }
    if !matches!(spec.opcode, 0x6e..=0x72 | 0x74..=0x78) {
        return Ok(None);
    }
    let Some(OperandReference::Method(id)) = layout.references()[0].map(|r| r.target) else {
        return Err(MethodError::InternalInvariant(
            "invoke has no method reference",
        ));
    };
    let method = index.method_reference(id, budget)?;
    let prototype = method.declaration.prototype;
    let receiver = !matches!(spec.opcode, 0x71 | 0x77);
    // A checked type_list has a u32 count, so its word total plus a receiver
    // is at most 2 * u32::MAX + 1 and fits u64.
    let expected_words = prototype.parameter_words.0 + u64::from(receiver);
    if operands.registers().len() as u64 != expected_words {
        return Err(bad(pc, MethodProblem::InvalidInvokeWordCount));
    }
    let mut shorty = prototype.shorty.reader()?;
    let result = match shorty.next(budget)? {
        Some(0x56) => ResultKind::Void,
        Some(0x4a | 0x44) => ResultKind::Wide,
        Some(0x4c) => ResultKind::Object,
        Some(0x5a | 0x42 | 0x53 | 0x43 | 0x49 | 0x46) => ResultKind::Scalar,
        _ => {
            return Err(MethodError::InternalInvariant(
                "checked shorty has no return kind",
            ));
        }
    };
    let mut words = operands.registers().iter();
    if receiver {
        budget.charge_work(1)?;
        words
            .next()
            .ok_or(MethodError::InternalInvariant("missing checked receiver"))?;
    }
    while let Some(tag) = shorty.next(budget)? {
        budget.charge_work(1)?;
        let first = words
            .next()
            .ok_or(MethodError::InternalInvariant("missing checked argument"))?;
        if matches!(tag, 0x4a | 0x44) {
            budget.charge_work(1)?;
            let second = words
                .next()
                .ok_or(MethodError::InternalInvariant("missing checked wide word"))?;
            if first.0.checked_add(1) != Some(second.0) {
                return Err(bad(pc, MethodProblem::NonAdjacentWideArgument));
            }
        }
    }
    if words.next().is_some() {
        return Err(MethodError::InternalInvariant(
            "checked argument words remain",
        ));
    }
    Ok(Some(result))
}
