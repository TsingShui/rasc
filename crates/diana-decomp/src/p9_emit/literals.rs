use crate::pipeline::scalar::{Expression, ScalarType};
use crate::substrate::owned::TextBuffer;
use crate::{MethodBudget, RequestError};

pub(super) fn numeric(
    expression: &Expression,
    kind: ScalarType,
    output: &mut TextBuffer,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<(), RequestError> {
    match (expression, kind) {
        (Expression::Constant(value @ (0 | 1)), ScalarType::Boolean) => {
            output.push(if *value == 0 { "false" } else { "true" }, budget)
        }
        (Expression::Constant(value), ScalarType::Int) => {
            output.arguments(format_args!("{value}"), budget)
        }
        (Expression::ConstantWide(value), ScalarType::Long) => {
            output.arguments(format_args!("{value}L"), budget)
        }
        (Expression::Constant(value), ScalarType::Float) => fp(
            u64::from(u32::from_le_bytes(value.to_le_bytes())),
            false,
            output,
            budget,
        ),
        (Expression::ConstantWide(value), ScalarType::Double) => fp(
            u64::from_le_bytes(value.to_le_bytes()),
            true,
            output,
            budget,
        ),
        _ => Err(RequestError::InternalInvariant(
            "sealed literal view changed width or family",
        )),
    }
}
fn fp(
    bits: u64,
    double: bool,
    output: &mut TextBuffer,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<(), RequestError> {
    let (sign, exponent, fraction, bias, maximum, suffix) = if double {
        (
            (bits >> 63) != 0,
            (bits >> 52) & 0x7ff,
            bits & 0x000f_ffff_ffff_ffff,
            1023,
            0x7ff,
            "d",
        )
    } else {
        (
            (bits >> 31) != 0,
            (bits >> 23) & 0xff,
            bits & 0x007f_ffff,
            127,
            0xff,
            "f",
        )
    };
    if exponent == maximum {
        if fraction != 0 {
            return Err(RequestError::InternalInvariant(
                "unexpressible NaN entered a sealed literal view",
            ));
        }
        output.push(if sign { "(-1.0" } else { "(1.0" }, budget)?;
        output.push(suffix, budget)?;
        output.push(" / 0.0", budget)?;
        output.push(suffix, budget)?;
        return output.push(")", budget);
    }
    if sign {
        output.push("-", budget)?;
    }
    if exponent == 0 && fraction == 0 {
        output.push("0.0", budget)?;
        return output.push(suffix, budget);
    }
    output.push(if exponent == 0 { "0x0." } else { "0x1." }, budget)?;
    if double {
        output.arguments(format_args!("{fraction:013x}"), budget)?;
    } else {
        output.arguments(format_args!("{:06x}", fraction << 1), budget)?;
    }
    let exponent = i64::try_from(exponent).map_err(|_| RequestError::PlatformCapacity)?;
    let power = if exponent == 0 {
        1 - bias
    } else {
        exponent - bias
    };
    output.arguments(format_args!("p{power}{suffix}"), budget)
}
