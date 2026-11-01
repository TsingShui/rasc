use crate::p5_types::{ArrayElement, PrimitiveArray};
use crate::substrate::owned::TextBuffer;
use crate::{MethodBudget, RequestError};
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
            let word = u32::try_from(bits)
                .map_err(|_| RequestError::InternalInvariant("sealed 32-bit literal overflowed"))?;
            if element.kind() == PrimitiveArray::Int {
                output.arguments(
                    format_args!("{}", i32::from_le_bytes(word.to_le_bytes())),
                    budget,
                )
            } else {
                super::literals::fp(u64::from(word), false, output, budget)
            }
        }
        PrimitiveArray::Long | PrimitiveArray::Double => {
            if element.kind() == PrimitiveArray::Long {
                output.arguments(
                    format_args!("{}L", i64::from_le_bytes(bits.to_le_bytes())),
                    budget,
                )
            } else {
                super::literals::fp(bits, true, output, budget)
            }
        }
    }
}
