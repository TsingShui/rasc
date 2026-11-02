use crate::p2_flow::{Op, Terminal};
use crate::p4_calls::field_facts::AssignmentObligation;
use crate::p5_types::initialization::ChainKind;
use crate::p6_variables::StorageBody;
use crate::substrate::owned::push;
use crate::{FieldId, MethodBudget, RequestError};

pub(super) struct Certificate {
    fields: Vec<FieldId>,
    delegate: Option<crate::MethodId>,
}
impl Certificate {
    pub(super) fn delegate(&self) -> Option<crate::MethodId> {
        self.delegate
    }
    pub(super) fn covers(
        &self,
        field: FieldId,
        control: &mut impl crate::pipeline::control::InputControl,
    ) -> Result<bool, RequestError> {
        for &assigned in &self.fields {
            control.charge_work(1)?;
            if assigned == field {
                return Ok(true);
            }
        }
        Ok(false)
    }
}

pub(super) fn verify(
    storage: &StorageBody<'_>,
    shape: &crate::p7_regions::Shape,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<Option<Certificate>, RequestError> {
    let delegate = storage
        .constructor_entry
        .as_ref()
        .filter(|entry| entry.kind == ChainKind::This)
        .map(|entry| entry.reference);
    let typed = &storage.typed;
    let ssa = &typed.ssa;
    let mut fields = Vec::new();
    for evidence in typed.field_declarations.iter().flatten() {
        budget.charge_work(1)?;
        if let crate::p4_calls::FieldDeclaration::Known(access) = &evidence.declaration
            && access.assignment == AssignmentObligation::OwnFinalInitialization
        {
            let mut found = false;
            for &field in &fields {
                budget.charge_work(1)?;
                found |= field == access.reference;
            }
            if !found {
                push(&mut fields, access.reference, budget)?;
            }
        }
    }
    if fields.is_empty() {
        return Ok(Some(Certificate { fields, delegate }));
    }
    if !matches!(
        shape,
        crate::p7_regions::Shape::Structured(
            crate::p7_regions::Region::Diamond(_) | crate::p7_regions::Region::Sequence(_)
        )
    ) || !ssa.exception_edges.is_empty()
        || !(matches!(storage.constructor_entry.as_ref(), Some(entry) if entry.kind == ChainKind::Super)
            || matches!(ssa.frame, crate::p2_flow::MethodFrame::Static))
    {
        return Ok(None);
    }
    for &field in &fields {
        budget.charge_work(1)?;
        let mut incoming = Vec::new();
        let mut states = Vec::new();
        let mut done = Vec::new();
        for _ in &ssa.blocks {
            push(&mut incoming, 0usize, budget)?;
            push(&mut states, 0u8, budget)?;
            push(&mut done, false, budget)?;
        }
        for edge in &ssa.edges {
            budget.charge_work(1)?;
            let target =
                usize::try_from(edge.target.0).map_err(|_| RequestError::PlatformCapacity)?;
            incoming[target] = incoming[target]
                .checked_add(1)
                .ok_or(RequestError::PlatformCapacity)?;
        }
        // Bits represent the finite path-count set {Zero, One, Many}.
        states[0] = 1;
        for _ in &ssa.blocks {
            budget.charge_work(1)?;
            let mut ready = None;
            for (index, &count) in incoming.iter().enumerate() {
                budget.charge_work(1)?;
                if count == 0 && !done[index] {
                    ready = Some(index);
                    break;
                }
            }
            let Some(block) = ready else { return Ok(None) };
            done[block] = true;
            let mut state = states[block];
            if state == 0 {
                return Ok(None);
            }
            for &id in &ssa.bound[block].ops {
                budget.charge_work(1)?;
                let op = ssa.operation(id)?;
                if matches!(op.op, Op::FieldLoad(_, _, reference, _) | Op::StaticLoad(_, reference, _) if reference == field)
                {
                    return Ok(None);
                }
                if matches!(op.op, Op::FieldStore(_, _, reference, _) | Op::StaticStore(_, reference, _) if reference == field)
                {
                    state = ((state & 1) << 1) | if state & 6 != 0 { 4 } else { 0 };
                }
            }
            match ssa.blocks[block].terminal {
                Terminal::ReturnVoid if state == 2 => {}
                Terminal::ReturnVoid
                | Terminal::Return(_)
                | Terminal::Throw(_)
                | Terminal::Switch { .. } => return Ok(None),
                Terminal::Jump(_) | Terminal::Test { .. } => {}
            }
            for edge in &ssa.edges {
                budget.charge_work(1)?;
                if usize::try_from(edge.source.0).map_err(|_| RequestError::PlatformCapacity)?
                    == block
                {
                    let target = usize::try_from(edge.target.0)
                        .map_err(|_| RequestError::PlatformCapacity)?;
                    states[target] |= state;
                    incoming[target] =
                        incoming[target]
                            .checked_sub(1)
                            .ok_or(RequestError::InternalInvariant(
                                "final path lost incoming edge",
                            ))?;
                }
            }
        }
    }
    Ok(Some(Certificate { fields, delegate }))
}
