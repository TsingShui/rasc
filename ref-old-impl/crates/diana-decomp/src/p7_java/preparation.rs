use crate::RequestError;
use crate::p0::ScalarValue;
use crate::pipeline::control::InputControl;
use crate::substrate::owned::TextBuffer;

pub(super) fn expression(
    descriptor: &[u16],
    value: Option<&ScalarValue>,
    default: bool,
    control: &mut impl InputControl,
) -> Result<Option<String>, RequestError> {
    let mut output = TextBuffer::new();
    if default {
        output.push(
            match descriptor {
                [90] => "false",
                [74] => "0L",
                [70] => "0.0f",
                [68] => "0.0d",
                [66 | 67 | 83 | 73] => "0",
                _ => "null",
            },
            control,
        )?;
        return Ok(Some(output.finish()));
    }
    match (descriptor, value) {
        ([90], Some(ScalarValue::Boolean(value))) => {
            output.push(if *value { "true" } else { "false" }, control)?
        }
        ([66 | 83 | 73], Some(ScalarValue::Signed(value))) => {
            if descriptor == [66] && i8::try_from(*value).is_err()
                || descriptor == [83] && i16::try_from(*value).is_err()
                || i32::try_from(*value).is_err()
            {
                return Ok(None);
            }
            output.arguments(format_args!("{value}"), control)?;
        }
        ([74], Some(ScalarValue::Signed(value))) => {
            output.arguments(format_args!("{value}L"), control)?
        }
        ([67], Some(ScalarValue::Unsigned(value))) if u16::try_from(*value).is_ok() => {
            output.arguments(format_args!("(char){value}"), control)?
        }
        ([70], Some(ScalarValue::FloatBits(bits))) => {
            if !fp(u64::from(*bits), false, &mut output, control)? {
                return Ok(None);
            }
        }
        ([68], Some(ScalarValue::DoubleBits(bits))) => {
            if !fp(*bits, true, &mut output, control)? {
                return Ok(None);
            }
        }
        (_, Some(ScalarValue::Null))
            if descriptor.first() == Some(&76) || descriptor.first() == Some(&91) =>
        {
            output.push("null", control)?
        }
        (_, Some(ScalarValue::String(units)))
            if descriptor == crate::p4_types::BuiltinReference::String.descriptor() =>
        {
            output.push("\"", control)?;
            for &unit in units.units() {
                control.checkpoint()?;
                match unit {
                    0 => output.push("\\000", control)?,
                    0x22 => output.push("\\\"", control)?,
                    0x5c => output.push("\\\\", control)?,
                    10 => output.push("\\n", control)?,
                    13 => output.push("\\r", control)?,
                    1..=0x1f | 0x7f => output.arguments(format_args!("\\{unit:03o}"), control)?,
                    0x20..=0x7e => output.character(
                        char::from(u8::try_from(unit).map_err(|_| RequestError::PlatformCapacity)?),
                        control,
                    )?,
                    _ => output.arguments(format_args!("\\u{unit:04x}"), control)?,
                }
            }
            output.push("\"", control)?;
        }
        _ => return Ok(None),
    }
    Ok(Some(output.finish()))
}

fn fp(
    bits: u64,
    double: bool,
    output: &mut TextBuffer,
    control: &mut impl InputControl,
) -> Result<bool, RequestError> {
    let (sign, exponent, fraction, bias, maximum, suffix) = if double {
        (
            bits >> 63 != 0,
            (bits >> 52) & 0x7ff,
            bits & 0x000f_ffff_ffff_ffff,
            1023,
            0x7ff,
            "d",
        )
    } else {
        (
            bits >> 31 != 0,
            (bits >> 23) & 0xff,
            bits & 0x007f_ffff,
            127,
            0xff,
            "f",
        )
    };
    if exponent == maximum {
        if fraction != 0 {
            return Ok(false);
        }
        output.arguments(
            format_args!("({}1.0{suffix} / 0.0{suffix})", if sign { "-" } else { "" }),
            control,
        )?;
    } else if exponent == 0 && fraction == 0 {
        output.arguments(
            format_args!("{}0.0{suffix}", if sign { "-" } else { "" }),
            control,
        )?;
    } else {
        output.push(if sign { "-" } else { "" }, control)?;
        output.push(if exponent == 0 { "0x0." } else { "0x1." }, control)?;
        if double {
            output.arguments(format_args!("{fraction:013x}"), control)?;
        } else {
            output.arguments(format_args!("{:06x}", fraction << 1), control)?;
        }
        let power = if exponent == 0 {
            1 - bias
        } else {
            i64::try_from(exponent).map_err(|_| RequestError::PlatformCapacity)? - bias
        };
        output.arguments(format_args!("p{power}{suffix}"), control)?;
    }
    Ok(true)
}
