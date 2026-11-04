use super::reference::{CheckedPrototype, CheckedReferenceFact, ReferenceFactEntry};
use super::structure::bad;
use super::{MethodError, MethodProblem};
use crate::input::formats::{OpcodeSpec, RegisterShape};
use crate::input::operands::RawOperands;
use crate::raw::ReferenceSlot;
use crate::{CodeUnitOffset, SlotWidth};

#[derive(Debug)]
pub(super) enum InvokeIssue {
    Reference(ReferenceSlot),
    Constraint(MethodError),
}
pub(super) struct ProducerFact {
    pub result: ResultKind,
    pub invoke: Option<Result<super::InvokeLayout, MethodError>>,
}
#[derive(Clone, Copy, Debug)]
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

/// The already-produced prototype is the sole binding source. An unreadable
/// target declaration need not erase a separate polymorphic prototype.
pub(super) fn invoke_prototype<'f, 'a>(
    facts: &'f [ReferenceFactEntry<'a>],
    pc: CodeUnitOffset,
    opcode: u8,
) -> Result<&'f CheckedPrototype<'a>, InvokeIssue> {
    let slot = if matches!(opcode, 0xfa | 0xfb) {
        ReferenceSlot::Second
    } else {
        ReferenceSlot::First
    };
    let entry = facts
        .binary_search_by(|e| (e.pc, e.slot).cmp(&(pc, slot)))
        .ok()
        .map(|i| &facts[i])
        .ok_or(InvokeIssue::Constraint(MethodError::InternalInvariant(
            "invoke lost its reference site",
        )))?;
    let fact = entry
        .fact
        .as_ref()
        .map_err(|_| InvokeIssue::Reference(slot))?;
    match fact {
        CheckedReferenceFact::Method { prototype, .. }
        | CheckedReferenceFact::CallSite { prototype, .. }
        | CheckedReferenceFact::Proto { prototype } => Ok(prototype),
        _ => Err(InvokeIssue::Constraint(MethodError::InternalInvariant(
            "invoke reference has no prototype",
        ))),
    }
}

/// Only argument grouping can fail here; a readable prototype still supplies a
/// receiver rule and result category, including when the encoded word count is bad.
pub(super) fn check_producer(
    facts: &[ReferenceFactEntry<'_>],
    pc: CodeUnitOffset,
    spec: OpcodeSpec,
    operands: RawOperands,
) -> Result<Option<ProducerFact>, InvokeIssue> {
    if spec.registers == RegisterShape::ArrayWords {
        return Ok(Some(ProducerFact {
            result: ResultKind::Object,
            invoke: None,
        }));
    }
    if !matches!(spec.opcode,0x6e..=0x72|0x74..=0x78|0xfa..=0xfd) {
        return Ok(None);
    }
    let prototype = invoke_prototype(facts, pc, spec.opcode)?;
    let receiver = !matches!(spec.opcode, 0x71 | 0x77 | 0xfc | 0xfd);
    let grouping = (|| {
        let expected = prototype.parameter_words() + u64::from(receiver);
        if operands.registers().len() as u64 != expected {
            return Err(bad(pc, MethodProblem::InvalidInvokeWordCount));
        }
        let mut shorty = prototype.shorty().reader()?;
        shorty.next()?;
        let mut words = operands.registers().iter();
        if receiver {
            words
                .next()
                .ok_or(MethodError::InternalInvariant("missing checked receiver"))?;
        }
        let mut arguments = Vec::new();
        while let Some(tag) = shorty.next()? {
            let first = words
                .next()
                .ok_or(MethodError::InternalInvariant("missing checked argument"))?;
            let width = if matches!(tag, 0x4a | 0x44) {
                let second = words
                    .next()
                    .ok_or(MethodError::InternalInvariant("missing checked wide word"))?;
                if first.0.checked_add(1) != Some(second.0) {
                    return Err(bad(pc, MethodProblem::NonAdjacentWideArgument));
                }
                SlotWidth::Two
            } else {
                SlotWidth::One
            };
            crate::substrate::owned::push(&mut arguments, width)
                .map_err(super::disassembly::scratch)?;
        }
        if words.next().is_some() {
            return Err(MethodError::InternalInvariant(
                "checked argument words remain",
            ));
        }
        Ok(super::InvokeLayout::new(
            receiver,
            arguments,
            prototype.result(),
        ))
    })();
    Ok(Some(ProducerFact {
        result: prototype.result_kind,
        invoke: Some(grouping),
    }))
}
