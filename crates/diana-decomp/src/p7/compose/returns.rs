//! A trailing void return prints nothing; its block keeps an anchor.

use super::super::*;
use super::*;
use crate::p2::BlockId;

/// A void method's closing brace performs the return a bare `return;` states:
/// nothing after it runs and no value is produced. Only the last printed action
/// of the whole method body qualifies; a return inside an arm, a loop or any
/// other scope that still prints after it stays explicit.
pub(super) fn omit_trailing_void_return(code: &mut MethodCode) -> Result<(), ExpressionError> {
    let Some(terminal) = trailing_void_return(code)? else {
        return Ok(());
    };
    let slot = code
        .emission
        .omitted_returns
        .get_mut(terminal.0 as usize)
        .ok_or(bug("trailing return names an unknown terminal"))?;
    *slot = true;
    Ok(())
}

/// The terminal that prints last in the method body, following only containers
/// that add nothing around their children.
pub(super) fn trailing_void_return(code: &MethodCode) -> Result<Option<BlockId>, ExpressionError> {
    let Some(root) = code.root else {
        return Ok(None);
    };
    let mut current = root;
    for _ in 0..=code.emission.controls.len() {
        let control = code
            .emission
            .controls
            .get(current.0 as usize)
            .ok_or(bug("trailing return names an unknown control"))?;
        // A planned continuation prints its `goto` after this control's children.
        if control.continuation.is_some() {
            return Ok(None);
        }
        let block = match &control.layout {
            // An unlabeled Sequence prints nothing around its children.
            EmissionLayout::Sequence if !control.label_required => {
                let source = code
                    .controls
                    .get(current.0 as usize)
                    .ok_or(bug("trailing return names an unknown control"))?;
                let children = control.children(source);
                let Some(last) = children
                    .iter()
                    .rev()
                    .find(|&&child| !code.emission.omitted(child))
                else {
                    return Ok(None);
                };
                current = *last;
                continue;
            }
            EmissionLayout::Block(node) => code
                .emission
                .blocks
                .get(node.0 as usize)
                .ok_or(bug("trailing return names an unknown block"))?,
            // Every other scope prints a closing brace, a predicate or a catch
            // clause after its children, so nothing inside it ends the body.
            _ => return Ok(None),
        };
        return trailing_return_of(code, block);
    }
    Ok(None)
}

/// The return printing a block's end, when it is a bare void return that prints
/// nothing besides the `return;` text and that no label is waiting on.
pub(super) fn trailing_return_of(
    code: &MethodCode,
    block: &EmissionBlock,
) -> Result<Option<BlockId>, ExpressionError> {
    if block.show_label {
        return Ok(None);
    }
    let BlockEnd::Terminal(terminal) = block.end else {
        return Ok(None);
    };
    let source = code
        .terminals
        .get(terminal.0 as usize)
        .ok_or(bug("block end names an unknown terminal"))?;
    if !matches!(source.kind, TerminalKind::Return(None)) || source.site.is_some() {
        return Ok(None);
    }
    let phases = code
        .emission
        .terminals
        .get(terminal.0 as usize)
        .ok_or(bug("trailing return lost its terminal phases"))?;
    if !phases.before.is_empty() || !phases.snapshots.is_empty() || !phases.after.is_empty() {
        return Ok(None);
    }
    Ok(Some(terminal))
}
