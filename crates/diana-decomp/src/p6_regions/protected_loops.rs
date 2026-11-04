use crate::p2_flow::{BlockId, EdgeId, Op, Terminal};
use crate::p5_variables::StorageBody;
use crate::substrate::owned::push;
use crate::{RequestError, RunContext};
pub(super) struct Loop {
    pub header: BlockId,
    pub follow: BlockId,
    pub body_yes: bool,
    pub backedges: Vec<EdgeId>,
}
fn ix(id: u32) -> Result<usize, RequestError> {
    usize::try_from(id).map_err(|_| RequestError::PlatformCapacity)
}
fn member(
    values: &[BlockId],
    value: BlockId,
    budget: &mut RunContext<'_>,
) -> Result<bool, RequestError> {
    for &old in values {
        budget.checkpoint()?;
        if old == value {
            return Ok(true);
        }
    }
    Ok(false)
}
pub(super) fn collect(
    storage: &StorageBody<'_>,
    budget: &mut RunContext<'_>,
) -> Result<Option<Vec<Loop>>, RequestError> {
    let ssa = &storage.typed.ssa;
    let mut colors = Vec::new();
    for _ in &ssa.blocks {
        budget.checkpoint()?;
        push(&mut colors, 0u8, budget)?;
    }
    let mut backs = Vec::new();
    for root in 0..ssa.blocks.len() {
        budget.checkpoint()?;
        if colors[root] != 0 {
            continue;
        }
        let mut todo = Vec::new();
        push(
            &mut todo,
            (
                BlockId(u32::try_from(root).map_err(|_| RequestError::PlatformCapacity)?),
                0usize,
            ),
            budget,
        )?;
        colors[root] = 1;
        while let Some((block, cursor)) = todo.pop() {
            budget.checkpoint()?;
            let mut next = None;
            for (index, edge) in ssa.edges.iter().enumerate().skip(cursor) {
                budget.checkpoint()?;
                if edge.source == block {
                    next = Some((index, edge.target));
                    break;
                }
            }
            if let Some((index, target)) = next {
                push(
                    &mut todo,
                    (
                        block,
                        index.checked_add(1).ok_or(RequestError::PlatformCapacity)?,
                    ),
                    budget,
                )?;
                match colors[ix(target.0)?] {
                    0 => {
                        colors[ix(target.0)?] = 1;
                        push(&mut todo, (target, 0), budget)?;
                    }
                    1 => push(
                        &mut backs,
                        EdgeId(u32::try_from(index).map_err(|_| RequestError::PlatformCapacity)?),
                        budget,
                    )?,
                    _ => {}
                }
            } else {
                colors[ix(block.0)?] = 2;
            }
        }
    }
    let mut loops: Vec<Loop> = Vec::new();
    for &back in &backs {
        budget.checkpoint()?;
        let header = ssa.edges[ix(back.0)?].target;
        let mut existing = None;
        for (index, region) in loops.iter().enumerate() {
            budget.checkpoint()?;
            if region.header == header {
                existing = Some(index);
                break;
            }
        }
        if let Some(index) = existing {
            push(&mut loops[index].backedges, back, budget)?;
            continue;
        }
        let Terminal::Test { yes, no, .. } = ssa.blocks[ix(header.0)?].terminal else {
            return Ok(None);
        };
        let mut owned = Vec::new();
        push(&mut owned, header, budget)?;
        let mut todo = Vec::new();
        for &edge in &backs {
            budget.checkpoint()?;
            let original = &ssa.edges[ix(edge.0)?];
            if original.target == header {
                push(&mut todo, original.source, budget)?;
            }
        }
        while let Some(block) = todo.pop() {
            budget.checkpoint()?;
            if member(&owned, block, budget)? {
                continue;
            }
            push(&mut owned, block, budget)?;
            if block == BlockId(0)
                || ssa.blocks[ix(block.0)?]
                    .ops
                    .iter()
                    .any(|op| matches!(op.kind, Op::Exception(_)))
            {
                return Ok(None);
            }
            for edge in &ssa.edges {
                budget.checkpoint()?;
                if edge.target == block {
                    push(&mut todo, edge.source, budget)?;
                }
            }
        }
        let y = member(&owned, yes, budget)?;
        let n = member(&owned, no, budget)?;
        if y == n {
            return Ok(None);
        }
        let follow = if y { no } else { yes };
        for edge in &ssa.edges {
            budget.checkpoint()?;
            let source = member(&owned, edge.source, budget)?;
            let target = member(&owned, edge.target, budget)?;
            if !source && target && edge.target != header
                || source && !target && edge.target != follow
            {
                return Ok(None);
            }
        }
        let mut backedges = Vec::new();
        push(&mut backedges, back, budget)?;
        push(
            &mut loops,
            Loop {
                header,
                follow,
                body_yes: y,
                backedges,
            },
            budget,
        )?;
    }
    Ok(Some(loops))
}
pub(super) fn cut(
    loops: &[Loop],
    id: EdgeId,
    budget: &mut RunContext<'_>,
) -> Result<bool, RequestError> {
    for region in loops {
        for &edge in &region.backedges {
            budget.checkpoint()?;
            if edge == id {
                return Ok(true);
            }
        }
    }
    Ok(false)
}
