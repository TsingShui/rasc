use super::DebugError;
use super::debug::write_reference;
use super::json::JsonBuffer;
use crate::RunContext;
use crate::input::ProgramIndex;
use crate::input::values::{EncodedKind, ValueStep};
use crate::raw::OperandReference;

pub(super) fn write(
    index: &ProgramIndex<'_>,
    output: &mut JsonBuffer,
    reference: OperandReference,
    budget: &mut RunContext<'_>,
) -> Result<(), DebugError> {
    match reference {
        OperandReference::MethodHandle(id) => {
            let handle = index.method_handle(id, budget)?;
            write_handle(index, output, handle.kind(), handle.member(), budget)
        }
        OperandReference::CallSite(id) => {
            let site = index.call_site(id, budget).map_err(|error| match error {
                crate::input::MethodError::Input(error) => DebugError::Input(error),
                other => DebugError::Method(other),
            })?;
            output.push("{\"kind\":\"call_site\",\"name\":", budget)?;
            output.string(site.name(), budget)?;
            output.push(",\"handle\":", budget)?;
            write_handle(
                index,
                output,
                site.handle().kind(),
                site.handle().member(),
                budget,
            )?;
            output.push(",\"proto\":{\"kind\":\"proto\",\"parameters\":[", budget)?;
            let mut first = true;
            site.prototype()
                .visit_parameter_types(budget, |name, budget| {
                    if !first {
                        output.push(",", budget)?;
                    }
                    first = false;
                    output.string(name, budget)
                })?;
            output.push("],\"return\":", budget)?;
            output.string(site.prototype().return_type, budget)?;
            output.push("},\"extra\":[", budget)?;
            write_extra(index, output, site.extra(), budget)?;
            output.push("]}", budget)
        }
        _ => Err(DebugError::InternalInvariant(
            "non-dynamic reference in dynamic writer",
        )),
    }
}
fn write_handle(
    index: &ProgramIndex<'_>,
    output: &mut JsonBuffer,
    kind: u16,
    member: OperandReference,
    budget: &mut RunContext<'_>,
) -> Result<(), DebugError> {
    output.push("{\"kind\":\"method_handle\",\"handle_type\":", budget)?;
    output.unsigned(u64::from(kind), budget)?;
    output.push(",\"member\":", budget)?;
    write_reference(index, output, member, budget)?;
    output.push("}", budget)
}
fn write_extra(
    index: &ProgramIndex<'_>,
    output: &mut JsonBuffer,
    events: &[ValueStep<'_>],
    budget: &mut RunContext<'_>,
) -> Result<(), DebugError> {
    let mut containers = Vec::new();
    let mut first = true;
    for &event in events {
        budget.checkpoint()?;
        let value = match event {
            ValueStep::EndContainer => {
                let annotation = containers.pop().ok_or(DebugError::InternalInvariant(
                    "checked extra container lost parent",
                ))?;
                output.push(if annotation { "}}}" } else { "]}" }, budget)?;
                continue;
            }
            ValueStep::Value(value) => value,
        };
        if if containers.is_empty() {
            let comma = !first;
            first = false;
            comma
        } else {
            value.ordinal.0 != 0
        } {
            output.push(",", budget)?;
        }
        if let Some(name) = value.name {
            output.string(index.string_with(name, budget)?, budget)?;
            output.push(":", budget)?;
        }
        output.push("{\"tag\":", budget)?;
        output.unsigned(u64::from(value.tag), budget)?;
        output.push(",\"value\":", budget)?;
        match value.kind {
            EncodedKind::Signed(number) => output.signed(number, budget)?,
            EncodedKind::Unsigned(number) => output.unsigned(number, budget)?,
            EncodedKind::Boolean(value) => output.boolean(value, budget)?,
            EncodedKind::Null => output.push("null", budget)?,
            EncodedKind::Reference(OperandReference::String(id)) => {
                output.units(index.string_with(id, budget)?, budget)?
            }
            EncodedKind::Reference(OperandReference::Type(id)) => {
                output.string(index.type_name(id, budget)?, budget)?
            }
            EncodedKind::Reference(reference) => write_reference(index, output, reference, budget)?,
            EncodedKind::Bits { width, bits } => {
                output.push("\"", budget)?;
                for digit in (0..width.0 * 2).rev() {
                    output.character(
                        char::from(b"0123456789abcdef"[((bits >> (digit * 4)) & 15) as usize]),
                        budget,
                    )?;
                }
                output.push("\"", budget)?;
            }
            EncodedKind::Array { .. } | EncodedKind::Annotation { .. } => {
                let annotation = matches!(value.kind, EncodedKind::Annotation { .. });
                if let EncodedKind::Annotation { type_id, .. } = value.kind {
                    output.push("{\"type\":", budget)?;
                    output.string(index.type_name(type_id, budget)?, budget)?;
                    output.push(",\"elements\":{", budget)?;
                } else {
                    output.push("[", budget)?;
                }
                if containers.len() == containers.capacity() {
                    let capacity = containers
                        .capacity()
                        .checked_mul(2)
                        .unwrap_or(events.len())
                        .max(1)
                        .min(events.len());
                    budget.checkpoint()?;
                    containers
                        .try_reserve_exact(capacity - containers.len())
                        .map_err(DebugError::Allocation)?;
                    if containers.capacity() != capacity {
                        return Err(DebugError::InternalInvariant(
                            "extra writer reservation changed capacity",
                        ));
                    }
                }
                containers.push(annotation);
                continue;
            }
        }
        output.push("}", budget)?;
    }
    if !containers.is_empty() {
        return Err(DebugError::InternalInvariant(
            "checked extra events ended inside container",
        ));
    }
    Ok(())
}
