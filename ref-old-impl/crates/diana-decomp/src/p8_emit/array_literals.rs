use crate::p7_java::array_literals::Literal;
use crate::substrate::owned::TextBuffer;
use crate::{RequestError, RunContext};
pub(super) fn element(
    element: &Literal,
    output: &mut TextBuffer,
    budget: &mut RunContext<'_>,
) -> Result<(), RequestError> {
    match *element {
        Literal::Boolean(value) => output.push(if value { "true" } else { "false" }, budget),
        Literal::Byte(value) => output.arguments(format_args!("{value}"), budget),
        Literal::Short(value) => output.arguments(format_args!("{value}"), budget),
        Literal::Char(value) => output.arguments(format_args!("{value}"), budget),
        Literal::Int(value) => output.arguments(format_args!("{value}"), budget),
        Literal::Long(value) => output.arguments(format_args!("{value}L"), budget),
        Literal::Float(bits) => super::literals::fp(u64::from(bits), false, output, budget),
        Literal::Double(bits) => super::literals::fp(bits, true, output, budget),
    }
}
