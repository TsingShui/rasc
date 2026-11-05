use super::build::*;
use super::*;
use crate::p2::{OpId, OpKind};
use crate::p3::{SsaMethod, StateId};

fn interpretation(
    ssa: &SsaMethod<'_>,
    f: &TypeFacts,
    operation: OpId,
    state: StateId,
) -> Result<ArrayElementType, Problem> {
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
    for &name in value.types() {
        let units = &f.names[name.0 as usize].units;
        let element = match units.as_slice() {
            [0x5b, 0x5a] => ArrayElementType::Boolean,
            [0x5b, 0x42] => ArrayElementType::Byte,
            [0x5b, 0x43] => ArrayElementType::Char,
            [0x5b, 0x53] => ArrayElementType::Short,
            [0x5b, 0x49] => ArrayElementType::Int,
            [0x5b, 0x4a] => ArrayElementType::Long,
            [0x5b, 0x46] => ArrayElementType::Float,
            [0x5b, 0x44] => ArrayElementType::Double,
            [0x5b, ..] => return Err(Problem::NonPrimitiveArrayComponent(name)),
            _ => return Err(Problem::NonArrayDescriptor(name)),
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
    if value.has_unknown_type_source() {
        return Err(Problem::Missing(MissingFact::ArrayComponent));
    }
    selected.ok_or(Problem::Missing(MissingFact::ArrayComponent))
}

pub(super) fn finish(ssa: &SsaMethod<'_>, f: &mut TypeFacts) -> Result<(), TypeError> {
    for operation in ssa.execution().operations() {
        if !matches!(operation.kind(), OpKind::FillArrayData { .. }) {
            continue;
        }
        let array = ssa
            .facts()
            .operation_uses(operation.id())
            .and_then(Result::ok)
            .and_then(|u| u.first())
            .ok_or(TypeError::InternalInvariant(
                "array fill has no SSA input binding",
            ))?
            .state();
        let element = interpretation(ssa, f, operation.id(), array);
        if let Err(problem) = element {
            issue(f, IssueSite::Operation(operation.id()), problem)?;
        }
        push(
            &mut f.array_fills,
            ArrayFillView {
                operation: operation.id(),
                array,
                element,
            },
        )?;
    }
    Ok(())
}
