//! Display names consume delivered definitions and declarations after composition.
//! Copy destinations and multiply defined locals cannot acquire a source name.

use super::build::{bug, filled};
use super::references::text;
use super::*;
use crate::p5::{DefinitionPlan, LocalKind, Materialization};
use std::collections::{HashMap, HashSet};

pub(super) fn construct(
    code: &mut MethodCode,
    definitions: &[DefinitionPlan],
    parameters: &[String],
) -> Result<(), ExpressionError> {
    let mut unique = filled(code.locals.len(), None)?;
    let mut excluded = filled(code.locals.len(), false)?;
    for definition in definitions {
        let local = match definition.materialization() {
            Materialization::Store(local)
            | Materialization::AllocationCarrier(local)
            | Materialization::ExceptionCapture(local)
            | Materialization::Deferred { local, .. } => local,
            Materialization::Literal(_) | Materialization::Unavailable(_) => continue,
        };
        let index = local.0 as usize;
        let slot = unique
            .get_mut(index)
            .ok_or(bug("local naming definition lost its declaration"))?;
        if slot.replace(definition).is_some() {
            excluded[index] = true;
        }
    }
    for bundle in &code.copies {
        // Requirements include unavailable copies as well as executable steps.
        for assignment in &bundle.assignments {
            if let Ok(local) = assignment.destination() {
                *excluded
                    .get_mut(local.0 as usize)
                    .ok_or(bug("local naming copy lost its declaration"))? = true;
            }
        }
    }
    let mut candidates = Vec::new();
    for (index, local) in code.locals.iter().enumerate() {
        let candidate = if excluded[index]
            || code.emission.hidden_local(local.id)
            || local.gap.is_some()
            || matches!(
                local.original.kind(),
                LocalKind::Parameter { .. }
                    | LocalKind::Snapshot { .. }
                    | LocalKind::ParallelTemporary { .. }
            ) {
            None
        } else if let Some(definition) = unique[index] {
            candidate(code, definition)?
        } else {
            None
        };
        super::build::push(&mut candidates, candidate)?;
    }

    let mut occupied = HashSet::new();
    let capacity = code
        .locals
        .len()
        .checked_add(parameters.len())
        .ok_or(crate::RequestError::PlatformCapacity)?;
    occupied
        .try_reserve(capacity)
        .map_err(crate::RequestError::Allocation)?;
    for (local, candidate) in code.locals.iter().zip(&candidates) {
        if candidate.is_none() && !code.emission.hidden_local(local.id) {
            occupied.insert(local.name.as_str());
        }
    }
    for parameter in parameters {
        occupied.insert(parameter.as_str());
    }
    let mut selected = HashSet::new();
    selected
        .try_reserve(candidates.len())
        .map_err(crate::RequestError::Allocation)?;
    let mut last_suffix = HashMap::new();
    last_suffix
        .try_reserve(candidates.len())
        .map_err(crate::RequestError::Allocation)?;
    // The local arena is constructed in ascending storage/local id order.
    for candidate in candidates.iter_mut().flatten() {
        if occupied.contains(candidate.as_str())
            || selected.contains(candidate.as_str())
            || !identifier(candidate)?
        {
            // Occupied and selected names never become free, so repeated bases
            // resume the suffix search instead of scanning earlier collisions.
            let mut suffix = last_suffix
                .get(candidate.as_str())
                .copied()
                .unwrap_or(1usize);
            loop {
                suffix = suffix
                    .checked_add(1)
                    .ok_or(crate::RequestError::PlatformCapacity)?;
                let suffixed = text(format_args!("{candidate}{suffix}"))?;
                if !occupied.contains(suffixed.as_str()) && !selected.contains(suffixed.as_str()) {
                    last_suffix.insert(text(format_args!("{candidate}"))?, suffix);
                    *candidate = suffixed;
                    break;
                }
            }
        }
        selected.insert(candidate.as_str());
    }
    // Publish atomically: an allocation failure must leave prefix names unchanged.
    drop(selected);
    drop(occupied);
    for (local, candidate) in code.locals.iter_mut().zip(candidates) {
        if let Some(name) = candidate {
            local.name = name;
        }
    }
    Ok(())
}

fn candidate(
    code: &MethodCode,
    definition: &DefinitionPlan,
) -> Result<Option<String>, ExpressionError> {
    let operation = code
        .operations
        .get(definition.operation().0 as usize)
        .ok_or(bug("local naming lost its operation"))?;
    let expression = if let Emission::Construction(group) = operation.emission {
        let group = &code.constructions[group.0 as usize];
        // Contracted aliases still originate in local copies, not allocations.
        if group.allocation != definition.operation() {
            return Ok(None);
        }
        group.expression
    } else {
        let Some(statement) = operation.statement else {
            return Ok(None);
        };
        let StatementKind::Evaluate { expression } = code.statements[statement.0 as usize].kind
        else {
            return Ok(None);
        };
        expression
    };
    let units = match &code.expressions[expression.0 as usize].kind {
        ExprKind::Field {
            reference,
            receiver,
        } if receiver.is_none_or(|receiver| is_this(code, receiver)) => {
            let Ok(ReferenceDeclaration::Member(MemberDeclaration::Field { name, .. })) =
                &code.references[reference.0 as usize].declaration
            else {
                return Ok(None);
            };
            return original_identifier(&name.units);
        }
        ExprKind::New { reference, .. } | ExprKind::AllocateInstance { reference } => {
            let Ok(ReferenceDeclaration::Type { type_use }) =
                code.references[reference.0 as usize].declaration
            else {
                return Ok(None);
            };
            let descriptor = &code.types[type_use.0 as usize].units;
            let Some(body) = descriptor.strip_prefix(&[u16::from(b'L')]) else {
                return Ok(None);
            };
            let Some(body) = body.strip_suffix(&[u16::from(b';')]) else {
                return Ok(None);
            };
            body.rsplit(|&unit| unit == u16::from(b'/'))
                .next()
                .unwrap_or_default()
        }
        ExprKind::Call(call) if call.receiver.is_some() => {
            let Ok(ReferenceDeclaration::Member(MemberDeclaration::Method { name, .. })) =
                &code.references[call.target.0 as usize].declaration
            else {
                return Ok(None);
            };
            if name.form != NameForm::Identifier {
                return Ok(None);
            }
            let Some(rest) = name.units.strip_prefix(&[103, 101, 116]) else {
                return Ok(None);
            };
            if !matches!(char::decode_utf16(rest.iter().copied()).next(), Some(Ok(c)) if c.is_uppercase())
            {
                return Ok(None);
            }
            rest
        }
        _ => return Ok(None),
    };
    let Some(original) = original_identifier(units)? else {
        return Ok(None);
    };
    let mut chars = original.chars();
    let Some(first) = chars.next() else {
        return Ok(None);
    };
    if !first.is_alphabetic() {
        return Ok(None);
    }
    let mut lowered = String::new();
    for c in first.to_lowercase().chain(chars) {
        lowered
            .try_reserve(c.len_utf8())
            .map_err(crate::RequestError::Allocation)?;
        lowered.push(c);
    }
    // Lowercasing can produce a keyword, which the collision pass suffixes.
    if identifier(&text(format_args!("{lowered}2"))?)? {
        Ok(Some(lowered))
    } else {
        Ok(None)
    }
}

fn is_this(code: &MethodCode, expression: ExprId) -> bool {
    let ExprKind::Read { storage, .. } = code.expressions[expression.0 as usize].kind else {
        return false;
    };
    matches!(
        code.locals[storage.0 as usize].original.kind(),
        LocalKind::Parameter {
            role: crate::p3::EntryRole::This,
            ..
        }
    )
}

fn original_identifier(units: &[u16]) -> Result<Option<String>, ExpressionError> {
    if crate::substrate::names::name_form(units) != NameForm::Identifier {
        return Ok(None);
    }
    let mut name = String::new();
    for c in char::decode_utf16(units.iter().copied()) {
        let c = c.map_err(|_| bug("identifier contains invalid UTF-16"))?;
        name.try_reserve(c.len_utf8())
            .map_err(crate::RequestError::Allocation)?;
        name.push(c);
    }
    Ok(Some(name))
}

fn identifier(name: &str) -> Result<bool, ExpressionError> {
    let mut units = Vec::new();
    for unit in name.encode_utf16() {
        super::build::push(&mut units, unit)?;
    }
    Ok(crate::substrate::names::name_form(&units) == NameForm::Identifier)
}
