use super::build::*;
use super::constraints::Work;
use super::*;
use crate::p2::{ArrayAccess, OpKind};
use crate::p3::SsaMethod;
use crate::substrate::owned;

pub(super) fn declaration(
    f: &mut TypeFacts,
    array: TypeNameId,
) -> Result<Option<TypeNameId>, TypeError> {
    let units = &f.names[array.0 as usize].units;
    if units.first() != Some(&u16::from(b'[')) || units.len() < 2 {
        return Ok(None);
    }
    if let Some(i) = f
        .names
        .iter()
        .position(|n| n.origin == TypeOrigin::Component { array })
    {
        return Ok(Some(TypeNameId(id(i)?)));
    }
    let source = &units[1..];
    let mut component = Vec::new();
    owned::reserve(&mut component, source.len())?;
    component.extend_from_slice(source);
    add_name(f, component, TypeOrigin::Component { array }).map(Some)
}

pub(super) fn finish(ssa: &SsaMethod<'_>, f: &mut TypeFacts, work: &Work) -> Result<(), TypeError> {
    for (index, array) in work.arrays.iter().enumerate() {
        let access = f
            .array_accesses
            .get(index)
            .ok_or(TypeError::InternalInvariant(
                "scoped component producer lost an array access",
            ))?;
        if !access.component.is_complete() {
            continue;
        }
        if let Some(output) = array.output {
            for &name in &access.component.types {
                if let Some(family) = name_family(f, name) {
                    let value = &mut f.values[output.0 as usize];
                    value.families = value.families.intersect(mask(family));
                }
            }
        } else if matches!(
            ssa.execution().operations()[array.operation.0 as usize].kind(),
            OpKind::Array {
                access: ArrayAccess::Put,
                ..
            }
        ) {
            let site =
                ssa.facts()
                    .operation_site(array.operation)
                    .ok_or(TypeError::InternalInvariant(
                        "array operation lost its SSA use site",
                    ))?;
            let input = &mut f.uses[site.use_start() as usize];
            for &name in &access.component.types {
                push(&mut input.requirement.types, name)?;
            }
        }
    }
    Ok(())
}
