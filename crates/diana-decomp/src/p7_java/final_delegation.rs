use crate::pipeline::control::InputControl;
use crate::substrate::owned::push;
use crate::{FieldId, MethodId, MethodOutput, RequestError};

pub(super) fn assigned(
    members: &[MethodOutput],
    first: MethodId,
    field: FieldId,
    control: &mut impl InputControl,
) -> Result<bool, RequestError> {
    let mut current = first;
    let mut seen = Vec::new();
    loop {
        control.charge_work(1)?;
        for &old in &seen {
            control.charge_work(1)?;
            if old == current {
                return Ok(false);
            }
        }
        push(&mut seen, current, control)?;
        let mut found = None;
        for member in members {
            control.charge_work(1)?;
            if member.key().method == current {
                if found.is_some() {
                    return Ok(false);
                }
                found = Some(member);
            }
        }
        let Some(MethodOutput::Java {
            facts, declaration, ..
        }) = found
        else {
            return Ok(false);
        };
        if facts.name != const { crate::substrate::owned::ascii_units(*b"<init>") } {
            return Ok(false);
        }
        let Some(proof) = &declaration.final_assignments else {
            return Ok(false);
        };
        if proof.covers(field, control)? {
            return Ok(proof.delegate().is_none());
        }
        let Some(target) = proof.delegate() else {
            return Ok(false);
        };
        current = target;
    }
}
