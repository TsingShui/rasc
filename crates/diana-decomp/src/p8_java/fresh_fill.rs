use super::branch::{LocalId, ValueUse};
use crate::p2_flow::normal::{Op, OpId};
use crate::p3_ssa::normal::BoundOp;
use crate::p5_types::normal::{Kind, TypedBody};
use crate::p6_variables::normal::StorageBody;
use crate::{CodeUnitOffset, MethodBudget, RequestError};

pub(crate) struct Allocation {
    pub result: LocalId,
    pub length: ValueUse,
    pub reference: crate::TypeId,
    pub component: crate::p5_types::PrimitiveArray,
}
pub(crate) struct Fill {
    pub array: LocalId,
    pub allocation: OpId,
    pub payload: CodeUnitOffset,
    pub elements: Vec<crate::p5_types::ArrayElement>,
}
fn ix(value: u32) -> Result<usize, RequestError> {
    usize::try_from(value).map_err(|_| RequestError::PlatformCapacity)
}
pub(super) fn validate(
    input: &crate::input::MethodInput<'_, '_>,
    typed: &TypedBody<'_>,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<(), RequestError> {
    for site in &typed.ssa.array_creations {
        budget.charge_work(1)?;
        if !input
            .type_name(site.reference, budget)?
            .matches_with(site.component.descriptor(), budget)?
        {
            return Err(RequestError::InternalInvariant(
                "array allocation changed original type",
            ));
        }
    }
    Ok(())
}
pub(super) fn allocate(typed: &TypedBody<'_>, op: &BoundOp) -> Result<Allocation, RequestError> {
    let Op::NewArray(_, _, id) = op.op else {
        return Err(RequestError::InternalInvariant(
            "non-allocation entered new-array lowering",
        ));
    };
    let site = typed
        .ssa
        .array_creations
        .get(ix(id.0)?)
        .ok_or(RequestError::InternalInvariant(
            "array allocation lost type",
        ))?;
    let length = typed
        .array_lengths
        .get(ix(id.0)?)
        .ok_or(RequestError::InternalInvariant(
            "array allocation lost length use",
        ))?;
    Ok(Allocation {
        result: LocalId(
            op.result
                .ok_or(RequestError::InternalInvariant(
                    "array allocation lost result",
                ))?
                .0,
        ),
        length: super::values::lower(LocalId(length.value.0), length.conversion),
        reference: site.reference,
        component: site.component,
    })
}
pub(super) fn fill(
    storage: &StorageBody<'_>,
    op: &BoundOp,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<Fill, RequestError> {
    let Op::FreshFill(_, id) = op.op else {
        return Err(RequestError::InternalInvariant(
            "non-fill entered FreshArray lowering",
        ));
    };
    let proof = storage
        .fresh_fills
        .get(ix(id.0)?)
        .ok_or(RequestError::InternalInvariant(
            "fill lost FreshArray proof",
        ))?;
    let mut elements = Vec::new();
    for value in &proof.elements {
        budget.charge_work(1)?;
        let element = value
            .kind()
            .element(value.bits())
            .ok_or(RequestError::InternalInvariant(
                "sealed array element changed representation",
            ))?;
        crate::substrate::owned::push(&mut elements, element, budget)?;
    }
    Ok(Fill {
        array: LocalId(proof.array.0),
        allocation: proof.allocation,
        payload: proof.payload,
        elements,
    })
}
pub(super) fn verify_allocation(
    typed: &TypedBody<'_>,
    op: &BoundOp,
    actual: &Allocation,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<bool, RequestError> {
    budget.charge_work(1)?;
    let Op::NewArray(destination, length_register, id) = op.op else {
        return Ok(false);
    };
    let site = typed
        .ssa
        .array_creations
        .get(ix(id.0)?)
        .ok_or(RequestError::InternalInvariant(
            "allocation certificate lost target",
        ))?;
    let layout = crate::raw::instruction_layout(
        typed.ssa.raw.instructions(),
        op.pc,
        typed.ssa.raw.version(),
    )
    .map_err(crate::input::MethodError::Decode)?;
    let operands =
        crate::input::operands::instruction_operands(typed.ssa.raw.instructions(), op.pc, layout)
            .map_err(crate::input::MethodError::Decode)?;
    let mut registers = operands.registers().iter();
    let length = op.input(0)?;
    if !matches!(layout.kind(),crate::raw::LayoutKind::Instruction(spec) if spec.opcode == 0x23)
        || !matches!(layout.references()[0],Some(reference) if reference.target == crate::raw::OperandReference::Type(site.reference))
        || registers.next() != Some(destination)
        || registers.next() != Some(length_register)
        || registers.next().is_some()
        || op.result.map(|value| value.0) != Some(actual.result.0)
        || actual.reference != site.reference
        || actual.component != site.component
        || typed.kinds[ix(actual.result.0)?] != Some(Kind::PrimitiveArray(site.component))
        || actual.length.value().0 != length.0
        || op.inputs.len() != 1
    {
        return Ok(false);
    }
    Ok(matches!(
        (actual.length, typed.kinds[ix(length.0)?]),
        (ValueUse::Direct(_), Some(Kind::Int)) | (ValueUse::BooleanWord(_), Some(Kind::Boolean))
    ))
}
pub(super) fn verify_fill(
    storage: &StorageBody<'_>,
    op: &BoundOp,
    actual: &Fill,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<bool, RequestError> {
    let Op::FreshFill(_, id) = op.op else {
        return Ok(false);
    };
    let proof = storage
        .fresh_fills
        .get(ix(id.0)?)
        .ok_or(RequestError::InternalInvariant(
            "fill certificate lost proof",
        ))?;
    if op.pc != proof.pc
        || actual.allocation != proof.allocation
        || actual.array.0 != proof.array.0
        || op.input(0)?.0 != actual.array.0
        || op.result.is_some()
        || actual.payload != proof.payload
        || actual.elements.len() != proof.elements.len()
    {
        return Ok(false);
    }
    let raw = &storage.typed.ssa.raw;
    let layout = crate::raw::instruction_layout(raw.instructions(), proof.payload, raw.version())
        .map_err(crate::input::MethodError::Decode)?;
    let mut payload =
        crate::input::operands::array_payload(raw.instructions(), proof.payload, layout)
            .map_err(crate::input::MethodError::Decode)?;
    for (value, expected) in actual.elements.iter().zip(&proof.elements) {
        budget.charge_work(1)?;
        let Some(original) = payload.next() else {
            return Ok(false);
        };
        if value.kind() != expected.kind()
            || value.bits() != expected.bits()
            || value.kind().width() != original.width.0
            || value.bits() != original.bits
        {
            return Ok(false);
        }
    }
    Ok(payload.next().is_none())
}
