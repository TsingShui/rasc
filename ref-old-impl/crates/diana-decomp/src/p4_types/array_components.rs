use super::PrimitiveArray;
use crate::input::{MethodSource, TypeProjection};
use crate::p2_flow::{ArrayKind, DeclarationType, Op};
use crate::p3_ssa::{Definition, SsaBody, Sym, ValueId};
use crate::substrate::owned::push;
use crate::{RequestError, RunContext};

#[derive(Clone, Copy)]
struct NamedOrigin {
    value: ValueId,
    array: TypeProjection,
}
pub(super) struct ArrayComponents {
    masks: Vec<u16>,
    named: Vec<NamedOrigin>,
}
fn ix(value: u32) -> Result<usize, RequestError> {
    usize::try_from(value).map_err(|_| RequestError::PlatformCapacity)
}
fn mask(kind: PrimitiveArray) -> u16 {
    match kind {
        PrimitiveArray::Boolean => 1,
        PrimitiveArray::Byte => 2,
        PrimitiveArray::Short => 4,
        PrimitiveArray::Char => 8,
        PrimitiveArray::Int => 16,
        PrimitiveArray::Float => 32,
        PrimitiveArray::Long => 64,
        PrimitiveArray::Double => 128,
    }
}
impl ArrayComponents {
    fn add(
        &mut self,
        value: ValueId,
        kind: DeclarationType,
        budget: &mut RunContext<'_>,
    ) -> Result<bool, RequestError> {
        let bits = match kind {
            DeclarationType::PrimitiveArray(kind) => mask(kind),
            DeclarationType::ObjectArray => 256,
            DeclarationType::ReferenceArray(array) => {
                for fact in &self.named {
                    budget.checkpoint()?;
                    if fact.value == value && fact.array == array {
                        return Ok(false);
                    }
                }
                push(&mut self.named, NamedOrigin { value, array }, budget)?;
                return Ok(true);
            }
            _ => return Ok(false),
        };
        let target = &mut self.masks[ix(value.0)?];
        let merged = *target | bits;
        let changed = merged != *target;
        *target = merged;
        Ok(changed)
    }
    pub(super) fn component(
        &self,
        input: &MethodSource<'_, '_>,
        value: ValueId,
        budget: &mut RunContext<'_>,
    ) -> Result<Option<DeclarationType>, RequestError> {
        let mut named = None;
        for fact in &self.named {
            budget.checkpoint()?;
            if fact.value == value {
                if named.is_some_and(|old| old != fact.array) {
                    return Ok(None);
                }
                named = Some(fact.array);
            }
        }
        let bits = self.masks[ix(value.0)?];
        if let Some(key) = named {
            if bits != 0 {
                return Ok(None);
            }
            return Ok(Some(ArrayKind::Reference(key).element_type(input, budget)?));
        }
        Ok(match bits {
            1 => Some(DeclarationType::Boolean),
            2 => Some(DeclarationType::Narrow(super::Narrow::Byte)),
            4 => Some(DeclarationType::Narrow(super::Narrow::Short)),
            8 => Some(DeclarationType::Narrow(super::Narrow::Char)),
            16 => Some(DeclarationType::Int),
            32 => Some(DeclarationType::Float),
            64 => Some(DeclarationType::Long),
            128 => Some(DeclarationType::Double),
            256 => Some(DeclarationType::Object),
            _ => None,
        })
    }
    fn propagate(
        &mut self,
        source: ValueId,
        target: ValueId,
        budget: &mut RunContext<'_>,
    ) -> Result<bool, RequestError> {
        let before = self.masks[ix(target.0)?];
        self.masks[ix(target.0)?] |= self.masks[ix(source.0)?];
        let mut changed = before != self.masks[ix(target.0)?];
        let end = self.named.len();
        for index in 0..end {
            budget.checkpoint()?;
            let fact = self.named[index];
            if fact.value == source {
                changed |= self.add(target, DeclarationType::ReferenceArray(fact.array), budget)?;
            }
        }
        Ok(changed)
    }
}
pub(super) fn collect(
    input: &MethodSource<'_, '_>,
    ssa: &SsaBody<'_>,
    budget: &mut RunContext<'_>,
) -> Result<ArrayComponents, RequestError> {
    let mut facts = ArrayComponents {
        masks: Vec::new(),
        named: Vec::new(),
    };
    for _ in &ssa.values {
        budget.checkpoint()?;
        push(&mut facts.masks, 0, budget)?;
    }
    for (index, definition) in ssa.values.iter().enumerate() {
        budget.checkpoint()?;
        let kind = match *definition {
            Definition::Parameter(index) => Some(ssa.parameters[ix(index)?]),
            Definition::OpResult(id) => match ssa.operation(id)?.op {
                Op::NewArray(_, _, id) => {
                    Some(ssa.array_creations[ix(id.0)?].component.declaration())
                }
                Op::FilledArray(id, _) => {
                    Some(ssa.filled_arrays[ix(id.0)?].component.declaration())
                }
                Op::FieldLoad(_, _, _, kind) | Op::StaticLoad(_, _, kind) => Some(kind),
                Op::Cast(_, id) => Some(ssa.casts[ix(id.0)?].kind),
                Op::Call(id, _) => match ssa.calls[ix(id.0)?].result {
                    crate::p2_flow::InvokeResult::Reference(Some(kind)) => Some(kind),
                    _ => None,
                },
                _ => None,
            },
            Definition::Phi | Definition::ExceptionAt(_) => None,
        };
        if let Some(kind) = kind {
            facts.add(
                ValueId(u32::try_from(index).map_err(|_| RequestError::PlatformCapacity)?),
                kind,
                budget,
            )?;
        }
    }
    loop {
        let mut changed = false;
        for op in &ssa.operations {
            budget.checkpoint()?;
            if matches!(op.op, Op::Move(..) | Op::MoveWide(..)) {
                changed |= facts.propagate(
                    op.input(0)?,
                    op.result
                        .ok_or(RequestError::InternalInvariant("array copy lost result"))?,
                    budget,
                )?;
            } else if matches!(op.op, Op::ArrayLoad(..)) {
                let source = op.input(0)?;
                let target = op
                    .result
                    .ok_or(RequestError::InternalInvariant("array read lost result"))?;
                let end = facts.named.len();
                // Close every origin before selecting a unique component for a typed use.
                for index in 0..end {
                    budget.checkpoint()?;
                    let fact = facts.named[index];
                    if fact.value == source {
                        let component =
                            ArrayKind::Reference(fact.array).element_type(input, budget)?;
                        changed |= facts.add(target, component, budget)?;
                    }
                }
            }
        }
        for phi in &ssa.phis {
            for &(_, incoming) in &phi.inputs {
                budget.checkpoint()?;
                let Sym::Value(source) = incoming else {
                    return Err(RequestError::InternalInvariant(
                        "array facts acquired unavailable input",
                    ));
                };
                changed |= facts.propagate(source, phi.result, budget)?;
            }
        }
        if !changed {
            return Ok(facts);
        }
    }
}
