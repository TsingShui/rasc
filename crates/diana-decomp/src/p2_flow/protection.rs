use super::normal::{Block, BlockId, Edge, Op, Terminal};
use crate::input::RawMethodView;
use crate::input::protection::{CatchType, CheckedProtection};
use crate::substrate::owned::push;
use crate::{CodeUnitOffset, RequestError, RunContext};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ExceptionEdgeId(pub u32);
#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) struct CatchRank(pub u32);
pub(crate) struct ExceptionEdge {
    pub source: BlockId,
    pub target: BlockId,
    pub pc: CodeUnitOffset,
    pub rank: CatchRank,
    pub kind: CatchType,
}
fn ix(value: u32) -> Result<usize, RequestError> {
    usize::try_from(value).map_err(|_| RequestError::PlatformCapacity)
}
pub(super) fn boundary(
    raw: &RawMethodView<'_>,
    pc: CodeUnitOffset,
) -> Result<CodeUnitOffset, RequestError> {
    if pc.0 == raw.code_units().0 {
        return Ok(pc);
    }
    let layout = crate::raw::instruction_layout(raw.instructions(), pc, raw.version())
        .map_err(crate::input::MethodError::Decode)?;
    // P1 has checked adjacency and forbidden explicit entries into this pure result.
    if matches!(layout.kind(), crate::raw::LayoutKind::Instruction(spec) if matches!(spec.opcode, 0x0a..=0x0c))
    {
        return pc
            .checked_advance(layout.width())
            .ok_or(RequestError::PlatformCapacity);
    }
    Ok(pc)
}
fn normal_ingress(
    protection: &CheckedProtection,
    blocks: &[Block],
    normal: &[Edge],
    budget: &mut RunContext<'_>,
) -> Result<bool, RequestError> {
    for edge in normal {
        budget.checkpoint()?;
        let block = blocks
            .get(ix(edge.target.0)?)
            .ok_or(RequestError::InternalInvariant("foreign normal target"))?;
        for target in protection.targets() {
            budget.checkpoint()?;
            if block.pc == target {
                return Ok(false);
            }
        }
    }
    Ok(true)
}
pub(crate) fn selection_pc(op: Op, pc: CodeUnitOffset) -> Result<CodeUnitOffset, RequestError> {
    if matches!(op, Op::Monitor(super::normal::MonitorKind::Exit, _)) {
        pc.checked_advance(crate::CodeUnitCount(1))
            .ok_or(RequestError::PlatformCapacity)
    } else {
        Ok(pc)
    }
}
struct Site {
    instruction: CodeUnitOffset,
    selection: CodeUnitOffset,
}
fn append_site(
    protection: &CheckedProtection,
    blocks: &[Block],
    source: BlockId,
    site: Site,
    result: &mut Vec<ExceptionEdge>,
    budget: &mut RunContext<'_>,
) -> Result<(), RequestError> {
    let Some(chain) = protection.at(site.selection, budget)? else {
        return Ok(());
    };
    for (rank, clause) in chain.iter().enumerate() {
        budget.checkpoint()?;
        let mut target = None;
        for (index, block) in blocks.iter().enumerate() {
            budget.checkpoint()?;
            if block.pc == clause.target {
                target = Some(BlockId(
                    u32::try_from(index).map_err(|_| RequestError::PlatformCapacity)?,
                ));
                break;
            }
        }
        push(
            result,
            ExceptionEdge {
                source,
                target: target.ok_or(RequestError::InternalInvariant(
                    "reachable handler lost its block",
                ))?,
                pc: site.instruction,
                rank: CatchRank(u32::try_from(rank).map_err(|_| RequestError::PlatformCapacity)?),
                kind: clause.kind,
            },
            budget,
        )?;
    }
    Ok(())
}
pub(super) fn build(
    protection: &CheckedProtection,
    blocks: &[Block],
    normal: &[Edge],
    budget: &mut RunContext<'_>,
) -> Result<Option<Vec<ExceptionEdge>>, RequestError> {
    if !normal_ingress(protection, blocks, normal, budget)? {
        return Ok(None);
    }
    let mut result = Vec::new();
    for (index, block) in blocks.iter().enumerate() {
        let source = BlockId(u32::try_from(index).map_err(|_| RequestError::PlatformCapacity)?);
        for op in &block.ops {
            budget.checkpoint()?;
            if op.kind.effects().may_throw {
                append_site(
                    protection,
                    blocks,
                    source,
                    Site {
                        instruction: op.pc,
                        selection: selection_pc(op.kind, op.pc)?,
                    },
                    &mut result,
                    budget,
                )?;
            }
        }
        if matches!(block.terminal, Terminal::Throw(_)) {
            append_site(
                protection,
                blocks,
                source,
                Site {
                    instruction: block.terminal_pc,
                    selection: block.terminal_pc,
                },
                &mut result,
                budget,
            )?;
        }
    }
    Ok(Some(result))
}
fn check_site(
    protection: &CheckedProtection,
    blocks: &[Block],
    source: BlockId,
    site: Site,
    edges: &[ExceptionEdge],
    cursor: &mut usize,
    budget: &mut RunContext<'_>,
) -> Result<(), RequestError> {
    let Some(chain) = protection.at(site.selection, budget)? else {
        return Ok(());
    };
    for (rank, clause) in chain.iter().enumerate() {
        budget.checkpoint()?;
        let edge = edges
            .get(*cursor)
            .ok_or(RequestError::InternalInvariant("missing exception edge"))?;
        let target = blocks
            .get(ix(edge.target.0)?)
            .ok_or(RequestError::InternalInvariant("foreign handler block"))?;
        if edge.source != source
            || edge.pc != site.instruction
            || edge.rank.0 != u32::try_from(rank).map_err(|_| RequestError::PlatformCapacity)?
            || edge.kind != clause.kind
            || target.pc != clause.target
        {
            return Err(RequestError::InternalInvariant(
                "exception edge changed protection or order",
            ));
        }
        *cursor = cursor
            .checked_add(1)
            .ok_or(RequestError::PlatformCapacity)?;
    }
    Ok(())
}
pub(crate) fn verify(
    raw: &RawMethodView<'_>,
    protection: &CheckedProtection,
    blocks: &[Block],
    normal: &[Edge],
    edges: &[ExceptionEdge],
    budget: &mut RunContext<'_>,
) -> Result<(), RequestError> {
    if !normal_ingress(protection, blocks, normal, budget)? {
        return Err(RequestError::InternalInvariant(
            "normal ingress into a handler",
        ));
    }
    for split in protection.boundaries() {
        let projected = boundary(raw, split)?;
        for block in blocks {
            budget.checkpoint()?;
            if block.pc == projected {
                continue;
            }
            if block.terminal_pc == projected {
                return Err(RequestError::InternalInvariant(
                    "protection boundary was not split",
                ));
            }
            for op in &block.ops {
                budget.checkpoint()?;
                if op.pc == projected {
                    return Err(RequestError::InternalInvariant(
                        "protection boundary crossed an operation",
                    ));
                }
            }
        }
    }
    let mut cursor = 0usize;
    for (index, block) in blocks.iter().enumerate() {
        let source = BlockId(u32::try_from(index).map_err(|_| RequestError::PlatformCapacity)?);
        for op in &block.ops {
            budget.checkpoint()?;
            let layout = crate::raw::instruction_layout(raw.instructions(), op.pc, raw.version())
                .map_err(crate::input::MethodError::Decode)?;
            let crate::raw::LayoutKind::Instruction(spec) = layout.kind() else {
                return Err(RequestError::InternalInvariant(
                    "throw site entered payload",
                ));
            };
            if matches!(op.kind, Op::Exception(_)) != (spec.opcode == 0x0d) {
                return Err(RequestError::InternalInvariant(
                    "handler exception changed raw definition",
                ));
            }
            if let Op::Exception(register) = op.kind {
                let operands =
                    crate::input::operands::instruction_operands(raw.instructions(), op.pc, layout)
                        .map_err(crate::input::MethodError::Decode)?;
                let mut found = false;
                for target in protection.targets() {
                    budget.checkpoint()?;
                    found |= target == op.pc;
                }
                if !found
                    || op.pc != block.pc
                    || operands.registers().iter().next() != Some(register)
                {
                    return Err(RequestError::InternalInvariant(
                        "exception definition lost handler entry",
                    ));
                }
            }
            if spec.may_throw() != op.kind.effects().may_throw {
                return Err(RequestError::InternalInvariant(
                    "operation changed raw throw effect",
                ));
            }
            if !spec.may_throw() {
                continue;
            }
            if protection
                .at(selection_pc(op.kind, op.pc)?, budget)?
                .is_some()
                && (block.ops.last().map(|last| last.id) != Some(op.id)
                    || block.terminal_pc != op.pc
                    || !matches!(block.terminal, Terminal::Jump(_)))
            {
                return Err(RequestError::InternalInvariant("throw site was not split"));
            }
            check_site(
                protection,
                blocks,
                source,
                Site {
                    instruction: op.pc,
                    selection: selection_pc(op.kind, op.pc)?,
                },
                edges,
                &mut cursor,
                budget,
            )?;
        }
        if let Terminal::Throw(register) = block.terminal {
            let layout = crate::raw::instruction_layout(
                raw.instructions(),
                block.terminal_pc,
                raw.version(),
            )
            .map_err(crate::input::MethodError::Decode)?;
            let operands = crate::input::operands::instruction_operands(
                raw.instructions(),
                block.terminal_pc,
                layout,
            )
            .map_err(crate::input::MethodError::Decode)?;
            if !matches!(layout.kind(),crate::raw::LayoutKind::Instruction(spec) if spec.opcode == 0x27)
                || operands.registers().iter().next() != Some(register)
            {
                return Err(RequestError::InternalInvariant(
                    "throw terminal changed original operand",
                ));
            }
            check_site(
                protection,
                blocks,
                source,
                Site {
                    instruction: block.terminal_pc,
                    selection: block.terminal_pc,
                },
                edges,
                &mut cursor,
                budget,
            )?;
        }
    }
    if cursor != edges.len() {
        return Err(RequestError::InternalInvariant("foreign exception edge"));
    }
    Ok(())
}
