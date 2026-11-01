use crate::p5_types::{ArrayElement, PrimitiveArray};
use crate::p8_java::fresh_array::FreshArrayBody;
use crate::pipeline::fresh_array::{ConstantId, Step};
use crate::pipeline::scalar::{Expression, ScalarType};
use crate::substrate::owned::TextBuffer;
use crate::{MethodBudget, RequestError};
fn value(
    id: ConstantId,
    output: &mut TextBuffer,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<(), RequestError> {
    output.push("v", budget)?;
    output.unsigned(u64::from(id.0), budget)
}
pub(super) fn element(
    element: &ArrayElement,
    output: &mut TextBuffer,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<(), RequestError> {
    let bits = element.bits();
    match element.kind() {
        PrimitiveArray::Boolean => output.push(if bits == 0 { "false" } else { "true" }, budget),
        PrimitiveArray::Byte => output.arguments(
            format_args!(
                "{}",
                i8::from_le_bytes(
                    u8::try_from(bits)
                        .map_err(|_| RequestError::InternalInvariant(
                            "sealed byte literal overflowed"
                        ))?
                        .to_le_bytes()
                )
            ),
            budget,
        ),
        PrimitiveArray::Short => output.arguments(
            format_args!(
                "{}",
                i16::from_le_bytes(
                    u16::try_from(bits)
                        .map_err(|_| RequestError::InternalInvariant(
                            "sealed short literal overflowed"
                        ))?
                        .to_le_bytes()
                )
            ),
            budget,
        ),
        PrimitiveArray::Char => output.arguments(
            format_args!(
                "{}",
                u16::try_from(bits).map_err(|_| RequestError::InternalInvariant(
                    "sealed char literal overflowed"
                ))?
            ),
            budget,
        ),
        PrimitiveArray::Int | PrimitiveArray::Float => {
            let value = i32::from_le_bytes(
                u32::try_from(bits)
                    .map_err(|_| {
                        RequestError::InternalInvariant("sealed 32-bit literal overflowed")
                    })?
                    .to_le_bytes(),
            );
            super::literals::numeric(
                &Expression::Constant(value),
                if element.kind() == PrimitiveArray::Int {
                    ScalarType::Int
                } else {
                    ScalarType::Float
                },
                output,
                budget,
            )
        }
        PrimitiveArray::Long | PrimitiveArray::Double => super::literals::numeric(
            &Expression::ConstantWide(i64::from_le_bytes(bits.to_le_bytes())),
            if element.kind() == PrimitiveArray::Long {
                ScalarType::Long
            } else {
                ScalarType::Double
            },
            output,
            budget,
        ),
    }
}
pub(super) fn write(
    body: &FreshArrayBody,
    output: &mut TextBuffer,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<(), RequestError> {
    let proof = &body.proof;
    if proof.allocation.id.0.0 >= proof.fill.pc.0 || proof.fill.pc.0 >= proof.returned.0 {
        return Err(RequestError::InternalInvariant(
            "fresh array effect positions changed",
        ));
    }
    for step in &proof.steps {
        budget.charge_work(1)?;
        match step {
            Step::Constant {
                value: id, literal, ..
            } => {
                output.push("int ", budget)?;
                value(*id, output, budget)?;
                output.push(" = ", budget)?;
                output.arguments(format_args!("{literal}"), budget)?;
                output.push(";\n", budget)?;
            }
            Step::Allocate => {
                output.push(proof.allocation.kind.spelling(), budget)?;
                output.push(" array0 = new ", budget)?;
                output.push(proof.allocation.kind.component_spelling(), budget)?;
                output.push("[", budget)?;
                value(proof.allocation.length, output, budget)?;
                output.push("];\n", budget)?;
            }
            Step::Fill => {
                output.push("int unusedLength = array0.length;\n", budget)?;
                for (i, literal) in body.elements.iter().enumerate() {
                    budget.charge_work(1)?;
                    output.push("array0[", budget)?;
                    output.unsigned(
                        u64::try_from(i).map_err(|_| RequestError::PlatformCapacity)?,
                        budget,
                    )?;
                    output.push("] = ", budget)?;
                    element(literal, output, budget)?;
                    output.push(";\n", budget)?;
                }
            }
        }
    }
    output.push("return array0;\n", budget)
}
