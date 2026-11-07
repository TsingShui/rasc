//! Which references and exception chains need their own annotation.

use super::super::*;
use super::*;
use crate::p5::ThrowSite;

/// A field reference is owner, name and type. The type is printed only when two
/// delivered references share owner and name but not type, or when the original
/// name form or a use issue already requires an explicit reference.
pub(super) fn field_qualifications(code: &MethodCode) -> Result<Vec<bool>, ExpressionError> {
    let mut explicit = filled(code.references.len(), false)?;
    for (index, reference) in code.references.iter().enumerate() {
        let Ok(ReferenceDeclaration::Member(MemberDeclaration::Field {
            owner,
            name,
            field_type,
        })) = &reference.declaration
        else {
            continue;
        };
        let owner_units = &code.types[owner.0 as usize].units;
        let field_units = &code.types[field_type.0 as usize].units;
        explicit[index] = name.form == NameForm::Original || reference.use_issue.is_some();
        if explicit[index] {
            continue;
        }
        for (other_index, other) in code.references.iter().enumerate() {
            if other_index == index {
                continue;
            }
            let Ok(ReferenceDeclaration::Member(MemberDeclaration::Field {
                owner: other_owner,
                name: other_name,
                field_type: other_type,
            })) = &other.declaration
            else {
                continue;
            };
            if other_name.units == name.units
                && code.types[other_owner.0 as usize].units == *owner_units
                && code.types[other_type.0 as usize].units != *field_units
            {
                explicit[index] = true;
                break;
            }
        }
    }
    Ok(explicit)
}

/// A natural route already printed as the enclosing try/catch needs no repeated
/// annotation. Explicit protection, several targets and unplaced routes keep one.
pub(super) fn site_annotations(code: &MethodCode) -> Result<Vec<bool>, ExpressionError> {
    let mut parent: Vec<Option<ControlId>> = filled(code.controls.len(), None)?;
    for control in &code.controls {
        for &child in &control.children {
            let slot = parent
                .get_mut(child.0 as usize)
                .ok_or(bug("control names an unknown child"))?;
            if slot.replace(control.id).is_some() {
                return Err(bug("control has two containment owners"));
            }
        }
    }
    let mut owner: Vec<Option<ControlId>> = filled(code.labels.len(), None)?;
    for label in &code.labels {
        *owner
            .get_mut(label.node.0 as usize)
            .ok_or(bug("label names an unknown node"))? = Some(label.owner);
    }
    let mut node_of_block: Vec<Option<u32>> = filled(code.terminals.len(), None)?;
    for (node, block) in code.emission.blocks.iter().enumerate() {
        if let BlockEnd::Terminal(terminal) = block.end {
            *node_of_block
                .get_mut(terminal.0 as usize)
                .ok_or(bug("block end names an unknown terminal"))? =
                Some(u32::try_from(node).map_err(|_| crate::RequestError::PlatformCapacity)?);
        }
    }
    let mut required = filled(code.sites.len(), true)?;
    for site in &code.sites {
        if matches!(
            site.syntax,
            ExceptionSyntax::Propagate | ExceptionSyntax::ImpossibleRelease
        ) {
            required[site.id.0 as usize] = false;
            continue;
        }
        if !matches!(site.syntax, ExceptionSyntax::Natural) {
            continue;
        }
        let [route] = site.routes.as_slice() else {
            continue;
        };
        let Delivery::Catch(target) = route.delivery else {
            continue;
        };
        let node = match site.original {
            ThrowSite::Operation(op) => code.operations.get(op.0 as usize).map(|o| o.node),
            ThrowSite::Terminal(block) => node_of_block
                .get(block.0 as usize)
                .copied()
                .flatten()
                .map(crate::p6::NodeId),
        };
        let Some(start) = node.and_then(|node| owner.get(node.0 as usize).copied().flatten())
        else {
            continue;
        };
        if super::sequence::contains(&parent, start, target) {
            required[site.id.0 as usize] = false;
        }
    }
    Ok(required)
}

/// Owner omission depends on the receiver that still prints after composition.
pub(super) fn call_owners(code: &mut MethodCode) -> Result<(), ExpressionError> {
    for index in 0..code.expressions.len() {
        let ExprKind::Call(call) = &code.expressions[index].kind else {
            continue;
        };
        if matches!(call.qualification, Qualification::Natural) {
            continue;
        }
        let owner = match &code.references[call.target.0 as usize].declaration {
            Ok(ReferenceDeclaration::Member(MemberDeclaration::Method { owner, .. })) => *owner,
            _ => continue,
        };
        let disclose = call.receiver.is_some_and(|receiver| {
            if code.emission.composed_use(receiver).is_some()
                || code.emission.inline_checks.iter().any(|check| check.occurrence == receiver)
            {
                return true;
            }
            let ExprKind::Read { storage, .. } = code.expressions[receiver.0 as usize].kind else {
                return true;
            };
            !matches!(code.emission.local_types[storage.0 as usize],
                LocalSpelling::Named(ty) if code.types[ty.0 as usize].units == code.types[owner.0 as usize].units)
        });
        let ExprKind::Call(call) = &mut code.expressions[index].kind else {
            return Err(super::bug("owner selection lost its call"));
        };
        match &mut call.qualification {
            Qualification::Natural => {}
            Qualification::Parameters { owner } | Qualification::FullSignature { owner } => {
                *owner = disclose
            }
        }
    }
    Ok(())
}
