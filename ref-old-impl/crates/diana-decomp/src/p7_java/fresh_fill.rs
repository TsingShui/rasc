use super::branch::{LocalId, ValueUse};
use crate::p2_flow::{Op, OpId};
use crate::p3_ssa::BoundOp;
use crate::p4_types::{Kind, TypedBody};
use crate::p5_variables::StorageBody;
use crate::{CodeUnitOffset, RequestError, RunContext};

pub(crate) enum Contents {
    Length(ValueUse),
    Elements(Vec<ValueUse>),
}
pub(crate) struct Allocation {
    pub result: LocalId,
    pub contents: Contents,
    pub base_type: String,
    pub trailing_dimensions: u32,
    pub reference: crate::TypeId,
    pub component: crate::p2_flow::ArrayKind,
}
pub(crate) struct Fill {
    pub array: LocalId,
    pub allocation: OpId,
    pub payload: CodeUnitOffset,
    pub elements: Vec<super::array_literals::Literal>,
}
fn ix(value: u32) -> Result<usize, RequestError> {
    usize::try_from(value).map_err(|_| RequestError::PlatformCapacity)
}
fn spelling<'a>(
    component: crate::p2_flow::ArrayKind,
    names: &'a super::types::TypeNames,
    budget: &mut RunContext<'_>,
) -> Result<(&'a str, u32), RequestError> {
    let mut base = names.name(component.kind(), budget)?;
    let mut dimensions = 0u32;
    while let Some(shorter) = base.strip_suffix("[]") {
        budget.checkpoint()?;
        base = shorter;
        dimensions = dimensions
            .checked_add(1)
            .ok_or(RequestError::PlatformCapacity)?;
    }
    let trailing = dimensions
        .checked_sub(1)
        .ok_or(RequestError::InternalInvariant(
            "array spelling became scalar",
        ))?;
    Ok((base, trailing))
}
pub(super) fn validate(
    input: &crate::input::MethodSource<'_, '_>,
    _catalog: &crate::context::Context,
    typed: &TypedBody<'_>,
    budget: &mut RunContext<'_>,
) -> Result<bool, RequestError> {
    let mut checked_java = false;
    let sites = typed
        .ssa
        .array_creations
        .iter()
        .map(|site| (site.reference, site.component))
        .chain(
            typed
                .ssa
                .filled_arrays
                .iter()
                .map(|site| (site.reference, site.component)),
        );
    for (reference, component) in sites {
        budget.checkpoint()?;
        if matches!(
            component,
            crate::p2_flow::ArrayKind::Object | crate::p2_flow::ArrayKind::Reference(_)
        ) && !checked_java
        {
            if input.qualifier_shadowed(&[106, 97, 118, 97], budget)? {
                return Ok(false);
            }
            checked_java = true;
        }
        if crate::p2_flow::ArrayKind::parse(input, reference, budget)? != Some(component) {
            return Err(RequestError::InternalInvariant(
                "array allocation changed original type",
            ));
        }
    }
    Ok(true)
}
pub(super) fn allocate(
    storage: &StorageBody<'_>,
    op: &BoundOp,
    names: &super::types::TypeNames,
    budget: &mut RunContext<'_>,
) -> Result<Allocation, RequestError> {
    let typed = &storage.typed;
    let (result, reference, component, contents) =
        match op.op {
            Op::NewArray(_, _, id) => {
                let site = typed.ssa.array_creations.get(ix(id.0)?).ok_or(
                    RequestError::InternalInvariant("array allocation lost type"),
                )?;
                let length =
                    typed
                        .array_lengths
                        .get(ix(id.0)?)
                        .ok_or(RequestError::InternalInvariant(
                            "array allocation lost length use",
                        ))?;
                let result = LocalId(
                    op.result
                        .ok_or(RequestError::InternalInvariant(
                            "array allocation lost result",
                        ))?
                        .0,
                );
                (
                    result,
                    site.reference,
                    site.component,
                    Contents::Length(super::values::lower(
                        LocalId(length.value.0),
                        length.conversion,
                    )),
                )
            }
            Op::FilledArray(id, _) => {
                let site = typed.ssa.filled_arrays.get(ix(id.0)?).ok_or(
                    RequestError::InternalInvariant("filled allocation lost type"),
                )?;
                let usage =
                    typed
                        .filled_arrays
                        .get(ix(id.0)?)
                        .ok_or(RequestError::InternalInvariant(
                            "filled allocation lost typed contents",
                        ))?;
                let result = *storage.filled_targets.get(ix(id.0)?).ok_or(
                    RequestError::InternalInvariant("filled allocation lost storage"),
                )?;
                let mut values = Vec::new();
                for argument in &usage.arguments {
                    budget.checkpoint()?;
                    crate::substrate::owned::push(
                        &mut values,
                        super::values::lower(LocalId(argument.value.0), argument.conversion),
                        budget,
                    )?;
                }
                (
                    result,
                    site.reference,
                    site.component,
                    Contents::Elements(values),
                )
            }
            _ => {
                return Err(RequestError::InternalInvariant(
                    "non-allocation entered array lowering",
                ));
            }
        };
    let (base, trailing_dimensions) = spelling(component, names, budget)?;
    let mut name = crate::substrate::owned::TextBuffer::new();
    name.push(base, budget)?;
    Ok(Allocation {
        result,
        reference,
        component,
        contents,
        base_type: name.finish(),
        trailing_dimensions,
    })
}
pub(super) fn fill(
    storage: &StorageBody<'_>,
    op: &BoundOp,
    budget: &mut RunContext<'_>,
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
        budget.checkpoint()?;
        let element = value
            .kind()
            .element(value.bits())
            .ok_or(RequestError::InternalInvariant(
                "sealed array element changed representation",
            ))?;
        crate::substrate::owned::push(
            &mut elements,
            super::array_literals::lower(&element)?,
            budget,
        )?;
    }
    Ok(Fill {
        array: LocalId(proof.array.0),
        allocation: proof.allocation,
        payload: proof.payload,
        elements,
    })
}
pub(super) fn verify_allocation(
    input: &crate::input::MethodSource<'_, '_>,
    storage: &StorageBody<'_>,
    op: &BoundOp,
    actual: &Allocation,
    names: &super::types::TypeNames,
    budget: &mut RunContext<'_>,
) -> Result<bool, RequestError> {
    budget.checkpoint()?;
    let typed = &storage.typed;
    let (base, dimensions) = spelling(actual.component, names, budget)?;
    if actual.base_type != base || actual.trailing_dimensions != dimensions {
        return Ok(false);
    }
    let raw = &typed.ssa.raw;
    let layout = crate::raw::instruction_layout(raw.instructions(), op.pc, raw.version())
        .map_err(crate::input::MethodError::Decode)?;
    let operands = crate::input::operands::instruction_operands(raw.instructions(), op.pc, layout)
        .map_err(crate::input::MethodError::Decode)?;
    match op.op {
        Op::NewArray(destination, length_register, id) => {
            let site =
                typed
                    .ssa
                    .array_creations
                    .get(ix(id.0)?)
                    .ok_or(RequestError::InternalInvariant(
                        "allocation certificate lost target",
                    ))?;
            let Contents::Length(length) = actual.contents else {
                return Ok(false);
            };
            let mut registers = operands.registers().iter();
            if !matches!(layout.kind(),crate::raw::LayoutKind::Instruction(spec) if spec.opcode == 0x23)
                || !matches!(layout.references()[0],Some(reference) if reference.target == crate::raw::OperandReference::Type(site.reference))
                || registers.next() != Some(destination)
                || registers.next() != Some(length_register)
                || registers.next().is_some()
                || op.result.map(|value| value.0) != Some(actual.result.0)
                || actual.reference != site.reference
                || actual.component != site.component
                || typed.kinds[ix(actual.result.0)?] != Some(site.component.kind())
                || length.value().0 != op.input(0)?.0
                || op.inputs.len() != 1
            {
                return Ok(false);
            }
            Ok(matches!(
                (length, typed.kinds[ix(op.input(0)?.0)?]),
                (ValueUse::Direct(_), Some(Kind::Int))
                    | (ValueUse::BooleanWord(_), Some(Kind::Boolean))
            ))
        }
        Op::FilledArray(id, result) => {
            let site =
                typed
                    .ssa
                    .filled_arrays
                    .get(ix(id.0)?)
                    .ok_or(RequestError::InternalInvariant(
                        "filled certificate lost type",
                    ))?;
            if matches!(site.component, crate::p2_flow::ArrayKind::Primitive(kind) if kind != crate::p4_types::PrimitiveArray::Int)
            {
                return Ok(false);
            }
            let Contents::Elements(values) = &actual.contents else {
                return Ok(false);
            };
            let target = storage
                .locals
                .get(ix(actual.result.0)?)
                .ok_or(RequestError::InternalInvariant("filled result lost local"))?;
            if !matches!(layout.kind(),crate::raw::LayoutKind::Instruction(spec) if matches!(spec.opcode,0x24|0x25))
                || !matches!(layout.references()[0],Some(reference) if reference.target == crate::raw::OperandReference::Type(site.reference))
                || site.pc != op.pc
                || actual.reference != site.reference
                || actual.component != site.component
                || storage.filled_targets.get(ix(id.0)?) != Some(&actual.result)
                || target.id != actual.result
                || target.kind != site.component.kind()
                || site.registers.len() != op.inputs.len()
                || values.len() != op.inputs.len()
            {
                return Ok(false);
            }
            let mut words = operands.registers().iter();
            for &register in &site.registers {
                budget.checkpoint()?;
                if words.next() != Some(register) {
                    return Ok(false);
                }
            }
            if words.next().is_some() {
                return Ok(false);
            }
            let next = op
                .pc
                .checked_advance(layout.width())
                .ok_or(RequestError::PlatformCapacity)?;
            let following = if next.0 < raw.code_units().0 {
                Some(
                    crate::raw::instruction_layout(raw.instructions(), next, raw.version())
                        .map_err(crate::input::MethodError::Decode)?,
                )
            } else {
                None
            };
            match result {
                crate::p2_flow::FilledResult::Stored { register, pc } => {
                    let Some(result_layout) = following else { return Ok(false); };
                    if pc != next || op.result.map(|value| value.0) != Some(actual.result.0)
                        || !matches!(result_layout.kind(),crate::raw::LayoutKind::Instruction(spec) if spec.opcode == 0x0c)
                    { return Ok(false); }
                    let result_words = crate::input::operands::instruction_operands(raw.instructions(), pc, result_layout)
                        .map_err(crate::input::MethodError::Decode)?;
                    let mut registers = result_words.registers().iter();
                    if registers.next() != Some(register) || registers.next().is_some() { return Ok(false); }
                }
                crate::p2_flow::FilledResult::Discarded => {
                    if op.result.is_some() || ix(actual.result.0)? < typed.ssa.values.len()
                        || following.is_some_and(|item| matches!(item.kind(),crate::raw::LayoutKind::Instruction(spec) if matches!(spec.opcode,0x0a..=0x0c))) {
                        return Ok(false);
                    }
                }
            }
            let component = site.component.element_type(input, budget)?;
            for (&source, &view) in op.inputs.iter().zip(values) {
                budget.checkpoint()?;
                let valid = if component.field_offset() == 2 {
                    super::values::verify_reference_store(typed, view, source, component, budget)?
                } else {
                    super::values::verify_primitive_store(typed, view, source, component)?
                };
                if !valid {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        _ => Ok(false),
    }
}
pub(super) fn verify_fill(
    storage: &StorageBody<'_>,
    op: &BoundOp,
    actual: &Fill,
    budget: &mut RunContext<'_>,
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
        budget.checkpoint()?;
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
