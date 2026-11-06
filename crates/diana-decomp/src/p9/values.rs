use super::{Origin, RenderError, buffer::Buffer, names::Lookup};
use crate::p8::*;
pub(super) fn values(
    out: &mut Buffer,
    lookup: &Lookup<'_>,
    values: &[ValueExpression],
) -> Result<(), RenderError> {
    for (i, v) in values.iter().enumerate() {
        if i > 0 {
            out.put(", ")?;
        }
        let start = out.position();
        if let Some(name) = &v.name {
            out.name(&name.units, name.form)?;
            out.put(" = ")?;
        }
        match &v.kind {
            ValueKind::Literal(value) => match value {
                Literal::Signed(v) => out.fmt(format_args!("{v}"))?,
                Literal::Unsigned(v) => out.fmt(format_args!("{v}u"))?,
                Literal::Bits { width, bits } => {
                    out.fmt(format_args!("bits<width={width}; 0x{bits:x}>"))?
                }
                Literal::String(v) => out.units(v, true)?,
                Literal::Null => out.put("null")?,
                Literal::Boolean(v) => out.put(if *v { "true" } else { "false" })?,
            },
            ValueKind::Reference {
                original,
                syntax,
                types,
            } => {
                out.fmt(format_args!("reference<{original:?}; "))?;
                match syntax {
                    ReferenceSyntax::Type => out.put("type")?,
                    ReferenceSyntax::Field(name) | ReferenceSyntax::Method(name) => {
                        out.name(&name.units, name.form)?
                    }
                    ReferenceSyntax::Prototype => out.put("prototype")?,
                    ReferenceSyntax::Handle { kind, name } => {
                        out.fmt(format_args!("handle kind={kind} "))?;
                        out.name(&name.units, name.form)?;
                    }
                    ReferenceSyntax::Unavailable(error) => {
                        out.fmt(format_args!("unavailable: {error}"))?
                    }
                }
                for &ty in types {
                    out.put("; ")?;
                    lookup.print(out, ty)?;
                }
                out.put(">")?;
            }
            ValueKind::Array { count } => out.fmt(format_args!("array_begin<count={count}>"))?,
            ValueKind::Annotation { type_use, count } => {
                out.put("annotation_begin<")?;
                lookup.print(out, *type_use)?;
                out.fmt(format_args!("; count={count}>"))?;
            }
            ValueKind::EndContainer => out.put("end_container")?,
        }
        out.span(start, Origin::Value(v.id))?;
    }
    Ok(())
}
pub(super) fn annotation(
    out: &mut Buffer,
    lookup: &Lookup<'_>,
    plan: &ValuePlan,
    a: &AnnotationExpression,
) -> Result<(), RenderError> {
    out.indent()?;
    let start = out.position();
    out.put("@")?;
    lookup.print(out, a.type_use)?;
    out.put("(")?;
    values(
        out,
        lookup,
        plan.expressions()
            .get(a.values.start as usize..a.values.end as usize)
            .ok_or(RenderError::Invariant(
                "annotation expression range is unavailable",
            ))?,
    )?;
    out.fmt(format_args!(
        ") [scope={:?}; visibility={}; extent={:?}]\n",
        a.location, a.visibility, a.extent
    ))?;
    out.span(start, Origin::Annotation(a.id))?;
    Ok(())
}
pub(super) fn metadata(
    out: &mut Buffer,
    lookup: &Lookup<'_>,
    plan: &ValuePlan,
) -> Result<(), RenderError> {
    if plan.annotations().is_empty() {
        return Ok(());
    }
    out.indent()?;
    out.put("metadata")?;
    out.open()?;
    for a in plan.annotations() {
        annotation(out, lookup, plan, a)?;
    }
    out.close()
}
