//! Retained facts of stopped construction, not another reconstruction path.
use super::{Origin, RenderError, buffer::Buffer};
use crate::{
    AnnotationLocation,
    p0::{Fact, InitialValue, ScalarValue},
    p8::AssemblyStop,
};

pub(super) fn assembly(
    out: &mut Buffer,
    stop: &AssemblyStop,
    class: bool,
) -> Result<(), RenderError> {
    let facts = stop.facts();
    if class {
        for field in facts.fields() {
            out.indent()?;
            let start = out.position();
            out.fmt(format_args!(
                "original_preparation field#{} {:?} ordinal {}: ",
                field.field().0,
                field.storage(),
                field.group_ordinal()
            ))?;
            match field.initial_value() {
                Fact::Present(InitialValue::ImplicitDefault) => out.put("implicit_default")?,
                Fact::Present(InitialValue::Encoded {
                    tag,
                    value_arg,
                    range_bytes,
                    bytes,
                    scalar,
                    tokens,
                    references,
                    extent,
                }) => {
                    out.fmt(format_args!("encoded<tag=0x{tag:x}; arg={value_arg}; file_bytes={range_bytes:?}; extent={extent:?}> bytes["))?;
                    for b in bytes {
                        out.fmt(format_args!("{b:02x} "))?;
                    }
                    out.put("]")?;
                    if let Some(scalar) = scalar {
                        out.put(" scalar=")?;
                        match scalar {
                            ScalarValue::Signed(v) => out.fmt(format_args!("{v}"))?,
                            ScalarValue::Unsigned(v) => out.fmt(format_args!("{v}u"))?,
                            ScalarValue::FloatBits(b) => {
                                out.fmt(format_args!("float_bits<0x{b:08x}>"))?
                            }
                            ScalarValue::DoubleBits(b) => {
                                out.fmt(format_args!("double_bits<0x{b:016x}>"))?
                            }
                            ScalarValue::Boolean(v) => {
                                out.put(if *v { "true" } else { "false" })?
                            }
                            ScalarValue::Null => out.put("null")?,
                            ScalarValue::String(s) => out.units(s.units(), true)?,
                        }
                    }
                    out.fmt(format_args!(
                        " tokens={tokens:?}; references={references:?}"
                    ))?;
                }
                Fact::Absent => out.put("absent")?,
                Fact::NotRead => out.put("not_read")?,
                Fact::Unavailable(e) => out.fmt(format_args!("unavailable<{e}>"))?,
            }
            out.put(";\n")?;
            out.span(start, Origin::Preparation(field.field()))?;
        }
    }
    for record in facts.annotations().records() {
        let included = class
            || match record.location() {
                AnnotationLocation::Method(id)
                | AnnotationLocation::Parameter { method: id, .. } => {
                    stop.methods().iter().any(|m| m.id == id)
                }
                _ => false,
            };
        if !included {
            continue;
        }
        out.indent()?;
        out.put("original_metadata ")?;
        out.units(record.descriptor(), true)?;
        out.fmt(format_args!(
            " scope={:?}; visibility={}; range={:?}; extent={:?}; tokens={:?};\n",
            record.location(),
            record.visibility(),
            record.range(),
            record.extent(),
            record.tokens()
        ))?;
        for token in record.tokens() {
            if let crate::AnnotationToken::Value(value) = token {
                match value.kind() {
                    crate::AnnotationValueKind::String { units, .. } => {
                        out.indent()?;
                        out.put("original_metadata_string ")?;
                        out.units(units, true)?;
                        out.put(";\n")?;
                    }
                    crate::AnnotationValueKind::Reference { declaration, .. } => {
                        // Published P0 tokens name completed entries in this owned pool.
                        let reference = &facts.annotations().references()[declaration.0 as usize];
                        out.indent()?;
                        out.fmt(format_args!("original_metadata_reference {reference:?};\n"))?;
                    }
                    _ => {}
                }
            }
        }
    }
    if let Some(error) = facts.annotations_issue() {
        out.indent()?;
        out.fmt(format_args!("metadata_unavailable<{error}>;\n"))?;
    }
    Ok(())
}
