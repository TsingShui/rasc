use crate::substrate::owned::TextBuffer;
use crate::{RequestError, RunContext};

pub(super) fn string(
    units: &[u16],
    output: &mut TextBuffer,
    budget: &mut RunContext<'_>,
) -> Result<(), RequestError> {
    output.push("\"", budget)?;
    for &unit in units {
        budget.checkpoint()?;
        match unit {
            0 => output.push("\\000", budget)?,
            0x22 => output.push("\\\"", budget)?,
            0x5c => output.push("\\\\", budget)?,
            8 => output.push("\\b", budget)?,
            9 => output.push("\\t", budget)?,
            10 => output.push("\\n", budget)?,
            12 => output.push("\\f", budget)?,
            13 => output.push("\\r", budget)?,
            1..=0x1f | 0x7f => output.arguments(format_args!("\\{unit:03o}"), budget)?,
            0x20..=0x7e => output.character(
                char::from(u8::try_from(unit).map_err(|_| {
                    RequestError::InternalInvariant("ASCII literal unit exceeded one byte")
                })?),
                budget,
            )?,
            _ => output.arguments(format_args!("\\u{unit:04x}"), budget)?,
        }
    }
    output.push("\"", budget)
}
pub(super) fn fp(
    bits: u64,
    double: bool,
    output: &mut TextBuffer,
    budget: &mut RunContext<'_>,
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
