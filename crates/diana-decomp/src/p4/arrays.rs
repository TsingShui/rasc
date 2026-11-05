use super::build::*;
use super::*;
use crate::p2::OpKind;
use crate::p3::SsaMethod;

fn interpretation(
    ssa: &SsaMethod<'_>,
    f: &TypeFacts,
    access: &ArrayAccessView,
) -> Result<ArrayElementType, Problem> {
    let operation = access.operation;
    let state = access.array;
    let s = ssa.facts();
    if !s
        .state(state)
        .is_some_and(|s| s.availability().is_available())
    {
        return Err(Problem::SourceUnavailable(state));
    }
    let value = state_value(s, state)
        .and_then(|v| f.value(v))
        .ok_or(Problem::SourceUnavailable(state))?;
    if !value.families().contains(Family::Reference) {
        return Err(Problem::FamilyConflict);
    }
    let payload = ssa
        .execution()
        .array_payload(operation)
        .map_err(|error| Problem::ArrayPayloadUnavailable(error.kind()))?
        .ok_or(Problem::Missing(MissingFact::ArrayPayload))?;
    let mut selected = None;
    for &name in access.component.types() {
        let units = &f.names[name.0 as usize].units;
        let element = match units.as_slice() {
            [0x5a] => ArrayElementType::Boolean,
            [0x42] => ArrayElementType::Byte,
            [0x43] => ArrayElementType::Char,
            [0x53] => ArrayElementType::Short,
            [0x49] => ArrayElementType::Int,
            [0x4a] => ArrayElementType::Long,
            [0x46] => ArrayElementType::Float,
            [0x44] => ArrayElementType::Double,
            _ => return Err(Problem::NonPrimitiveArrayComponent(name)),
        };
        if element.width() != payload.element_width() {
            return Err(Problem::ArrayPayloadWidth {
                expected: element.width(),
                actual: payload.element_width(),
            });
        }
        if selected.is_some_and(|previous| previous != element) {
            return Err(Problem::ArrayComponentConflict);
        }
        selected = Some(element);
    }
    if !access.component.is_complete() {
        return Err(Problem::Missing(MissingFact::ArrayComponent));
    }
    selected.ok_or(Problem::Missing(MissingFact::ArrayComponent))
}

pub(super) fn finish(ssa: &SsaMethod<'_>, f: &mut TypeFacts) -> Result<(), TypeError> {
    for i in 0..f.array_accesses.len() {
        let access = &f.array_accesses[i];
        if !matches!(
            ssa.execution().operations()[access.operation.0 as usize].kind(),
            OpKind::FillArrayData { .. }
        ) {
            continue;
        }
        let operation = access.operation;
        let array = access.array;
        let element = interpretation(ssa, f, access);
        if let Err(problem) = element {
            issue(f, IssueSite::Operation(operation), problem)?;
        }
        push(
            &mut f.array_fills,
            ArrayFillView {
                operation,
                array,
                element,
            },
        )?;
    }
    Ok(())
}
