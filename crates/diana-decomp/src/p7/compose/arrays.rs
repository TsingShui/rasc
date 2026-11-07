//! A fill payload becomes an array literal only when every element is exact.

use super::super::*;
use super::*;

/// An allocation immediately followed by the fill that states its whole content
/// prints that payload as its own array literal. Every other allocation keeps
/// the separate `new Element[count]` and `fill_array` statements: a display flag
/// is only ever set from facts that already decide each condition.
pub(super) fn array_literals(code: &mut MethodCode) -> Result<(), ExpressionError> {
    let Some((_, protected)) = super::access::local_access(code)? else {
        return Ok(());
    };
    let mut literals: Vec<Option<ArrayLiteral>> = filled(code.statements.len(), None)?;
    let mut folded = filled(code.statements.len(), false)?;
    for node in 0..code.emission.blocks.len() {
        let actions = &code.emission.blocks[node].actions;
        for index in 0..actions.len().saturating_sub(1) {
            let (
                BodyAction::Statement {
                    operation: allocation_operation,
                    statement: allocation,
                },
                BodyAction::Statement {
                    operation: fill_operation,
                    statement: fill,
                },
            ) = (actions[index], actions[index + 1])
            else {
                continue;
            };
            let Some(literal) = array_literal(
                code,
                &protected,
                allocation_operation,
                allocation,
                fill_operation,
                fill,
            ) else {
                continue;
            };
            *literals
                .get_mut(allocation.0 as usize)
                .ok_or(bug("statement names an unknown statement"))? = Some(literal);
            *folded
                .get_mut(fill.0 as usize)
                .ok_or(bug("statement names an unknown statement"))? = true;
        }
    }
    if !folded.contains(&true) {
        return Ok(());
    }
    code.emission.array_literals = literals;
    // The literal states the fill's whole effect, so the fill prints no text.
    // Every other action stays at its own position.
    for block in &mut code.emission.blocks {
        block.actions.retain(|action| match action {
            BodyAction::Statement { statement, .. } | BodyAction::Release { statement, .. } => {
                !folded.get(statement.0 as usize).copied().unwrap_or(false)
            }
            BodyAction::Receive { .. } => true,
        });
    }
    Ok(())
}

pub(super) fn array_literal(
    code: &MethodCode,
    protected: &[bool],
    allocation_operation: crate::p2::OpId,
    allocation: StatementId,
    fill_operation: crate::p2::OpId,
    fill: StatementId,
) -> Option<ArrayLiteral> {
    let operation = code.operations.get(allocation_operation.0 as usize)?;
    if !matches!(operation.emission, Emission::Block { .. })
        || code.emission.composed_operation(allocation_operation)
    {
        return None;
    }
    let allocation = code.statements.get(allocation.0 as usize)?;
    let StatementKind::Evaluate { expression } = allocation.kind else {
        return None;
    };
    let ExprKind::NewArray { reference, length } =
        code.expressions.get(expression.0 as usize)?.kind
    else {
        return None;
    };
    // The array local is the allocation's own committed result, not a copy, an
    // alias, a phi or a snapshot of it.
    let [definition] = allocation.outputs.as_slice() else {
        return None;
    };
    let crate::p5::Materialization::Store(destination) = definition.materialization() else {
        return None;
    };
    if definition.commit() != crate::p2::CommitRule::AtOperation {
        return None;
    }
    let storage = StorageId(destination.0);
    let declared = code.locals.get(storage.0 as usize)?;
    // One SSA value: a parameter, an uninitialized carrier, a saved snapshot or a
    // copy temporary would make the local something other than this allocation.
    // No copy or publication may observe it either, because such a step prints
    // after the allocation and before the fill.
    if declared.gap.is_some()
        || !matches!(
            declared.original.kind(),
            crate::p5::LocalKind::Value(_) | crate::p5::LocalKind::ReferenceCarrier(_)
        )
        || protected[storage.0 as usize]
    {
        return None;
    }
    // The fill writes only that array and prints nothing else: no definition, no
    // printed exception marker and no copy phase of its own. A site that prints no
    // text states a route already visible in the enclosing chains and catches.
    let fill_statement = code.statements.get(fill.0 as usize)?;
    let StatementKind::FillArray {
        array,
        payload: payload_id,
    } = fill_statement.kind
    else {
        return None;
    };
    let phases = code.emission.operations.get(fill_operation.0 as usize)?;
    if !fill_statement.outputs.is_empty()
        || fill_statement
            .sites
            .iter()
            .any(|&site| code.emission.site_annotation(site))
        || !phases.before.is_empty()
        || !phases.snapshots.is_empty()
        || !phases.after.is_empty()
    {
        return None;
    }
    // The fill's only read of that local is its array operand, and it prints the
    // local itself rather than a composed producer.
    if !matches!(
        code.expressions.get(array.0 as usize)?.kind,
        ExprKind::Read { storage: read, .. } if read == storage
    ) || code.emission.composed_use(array).is_some()
    {
        return None;
    }
    let payload = code.payloads.get(payload_id.0 as usize)?;
    // One dimension of the payload's own element type, allocated with the
    // payload's exact element count.
    let Ok(ReferenceDeclaration::Type { type_use }) =
        &code.references.get(reference.0 as usize)?.declaration
    else {
        return None;
    };
    let Ok(element) = payload.element else {
        return None;
    };
    let units = [u16::from(b'['), u16::from(element_descriptor(element))];
    if code.types.get(type_use.0 as usize)?.units != units {
        return None;
    }
    let ExprKind::Literal { bits, .. } = code.expressions.get(length.0 as usize)?.kind else {
        return None;
    };
    if bits.width != crate::SlotWidth::One
        || bits.value != u64::from(payload.count)
        || usize::try_from(payload.count).ok()? != payload.bits.len()
        || !exact_elements(element, &payload.bits)
    {
        return None;
    }
    Some(ArrayLiteral {
        payload: payload_id,
        fill,
        operation: fill_operation,
        pc: code.operations.get(fill_operation.0 as usize)?.pc,
    })
}

/// The descriptor letter of one primitive array element type.
pub(super) fn element_descriptor(element: crate::p4::ArrayElementType) -> u8 {
    match element {
        crate::p4::ArrayElementType::Boolean => b'Z',
        crate::p4::ArrayElementType::Byte => b'B',
        crate::p4::ArrayElementType::Char => b'C',
        crate::p4::ArrayElementType::Short => b'S',
        crate::p4::ArrayElementType::Int => b'I',
        crate::p4::ArrayElementType::Long => b'J',
        crate::p4::ArrayElementType::Float => b'F',
        crate::p4::ArrayElementType::Double => b'D',
    }
}

/// Whether every element has an exact Java literal spelling. P9 prints these
/// elements with the bit-preserving forms the ordinary literal printer uses: the
/// shortest decimal for a finite float already round-trips to the same bits,
/// while NaN and either infinity have no finite decimal spelling at all.
pub(super) fn exact_elements(element: crate::p4::ArrayElementType, bits: &[u64]) -> bool {
    use crate::p4::ArrayElementType::*;
    match element {
        Boolean => bits.iter().all(|&value| value <= 1),
        Float => bits
            .iter()
            .all(|&value| f32::from_bits(value as u32).is_finite()),
        Double => bits.iter().all(|&value| f64::from_bits(value).is_finite()),
        Byte | Char | Short | Int | Long => true,
    }
}
