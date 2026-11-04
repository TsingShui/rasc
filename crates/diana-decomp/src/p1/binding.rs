use super::structure::bad;
use super::{MethodError, MethodProblem};
use crate::CodeUnitOffset;
use crate::input::ProgramIndex;
use crate::input::formats::{InsnLayout, OpcodeSpec, OperandReference, RegisterShape};
use crate::input::operands::RawOperands;
use crate::substrate::slots::SlotWidth;

#[derive(Clone)]
pub(crate) struct ProducerFact {
    pub result: ResultKind,
    pub invoke: Option<super::InvokeLayout>,
}

#[derive(Clone, Copy)]
pub(crate) enum ResultKind {
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

/// Checks ordinary/custom invoke words against original prototypes, without resolving
/// dispatch or assigning types to register values. All reads use the normal
/// method ledger; borrowed iterators require no scratch allocation.
pub(crate) fn check_producer(
    index: &ProgramIndex<'_>,
    pc: CodeUnitOffset,
    spec: OpcodeSpec,
    layout: InsnLayout,
    operands: RawOperands,
) -> Result<Option<ProducerFact>, MethodError> {
    if spec.registers == RegisterShape::ArrayWords {
        return Ok(Some(ProducerFact {
            result: ResultKind::Object,
            invoke: None,
        }));
    }
    if !matches!(
        spec.opcode,
        0x6e..=0x72 | 0x74..=0x78 | 0xfa..=0xfd
    ) {
        return Ok(None);
    }
    let prototype = match (spec.opcode, layout.references()[0].map(|r| r.target)) {
        // A polymorphic call's own declaration describes the receiver only; the
        // second reference supplies the actual argument/result prototype.
        (0xfa | 0xfb, Some(OperandReference::Method(id))) => {
            index.method_reference(id)?;
            let Some(OperandReference::Proto(proto)) = layout.references()[1].map(|r| r.target)
            else {
                return Err(MethodError::InternalInvariant(
                    "polymorphic invoke has no checked prototype reference",
                ));
            };
            index.prototype(proto)?
        }
        (_, Some(OperandReference::Method(id))) => {
            index.method_reference(id)?.declaration.prototype
        }
        (_, Some(OperandReference::CallSite(id))) => index.call_site(id)?.prototype,
        _ => {
            return Err(MethodError::InternalInvariant(
                "invoke has no checked target reference",
            ));
        }
    };
    let receiver = !matches!(spec.opcode, 0x71 | 0x77 | 0xfc | 0xfd);
    // A checked type_list has a u32 count, so its word total plus a receiver
    // is at most 2 * u32::MAX + 1 and fits u64.
    let expected_words = prototype.parameter_words.0 + u64::from(receiver);
    if operands.registers().len() as u64 != expected_words {
        return Err(bad(pc, MethodProblem::InvalidInvokeWordCount));
    }
    let mut shorty = prototype.shorty.reader()?;
    let result = match shorty.next()? {
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
    let result_width = match result {
        ResultKind::Void => None,
        ResultKind::Scalar | ResultKind::Object => Some(SlotWidth::One),
        ResultKind::Wide => Some(SlotWidth::Two),
    };
    Ok(Some(ProducerFact {
        result,
        invoke: Some(super::InvokeLayout::new(receiver, arguments, result_width)),
    }))
}
