//! Translation validation over immutable P1/P2 products. Coverage is a bijection
//! and transfers are matched by source site and role, not by aggregate counts.

use super::scratch::{filled, push};
use super::{
    BlockId, Coverage, EdgeRole, EdgeTarget, ExitId, ExitKind, FlowError, FlowMethod, NormalEdge,
    OpId,
};
use crate::p1::{DecodedInstruction, DecodedItem};
use crate::{CodeUnitOffset, TypeId};

fn bug(message: &'static str) -> FlowError {
    FlowError::InternalInvariant(message)
}

pub(crate) fn verify(flow: &FlowMethod<'_>) -> Result<(), FlowError> {
    check_coverage_and_order(flow)?;
    let mut transfers = Transfers::new(flow)?;
    for block in flow.blocks() {
        for index in block.operations.clone() {
            let operation = &flow.operations()[index as usize];
            let instruction = instruction(flow, operation.pc)?;
            super::oracle::verify_operation(flow, instruction, operation)?;
            // The selection comes from the original instruction, never the P2 enum.
            let advanced = instruction.opcode() == 0x1e;
            let selection = if advanced {
                next(instruction)?
            } else {
                instruction.pc()
            };
            if operation.exceptional.is_some() != operation.effects.may_throw() {
                return Err(bug("operation throwability and completion disagree"));
            }
            if operation.effects.may_throw() {
                let completion = operation
                    .exceptional
                    .ok_or(bug("missing exception completion"))?;
                check_exception_completion(
                    completion,
                    operation.pc,
                    selection,
                    operation.normal.heap,
                    if advanced {
                        super::LockTransfer::ReleaseOutcome
                    } else {
                        super::LockTransfer::Preserve
                    },
                )?;
                transfers.exceptions(block.id, Some(operation.id), operation.pc, selection)?;
            }
        }
        if let Some(pc) = block.terminal_pc {
            let original = instruction(flow, pc)?;
            super::oracle::verify_terminal(flow, block, original)?;
            if original.opcode() == 0x27 {
                check_exception_completion(
                    block.exceptional.ok_or(bug("throw lacks completion"))?,
                    pc,
                    pc,
                    super::HeapTransfer::Preserve,
                    super::LockTransfer::Preserve,
                )?;
            } else if block.exceptional.is_some() {
                return Err(bug("non-throw terminal has an exception completion"));
            }
            match original.opcode() {
                0x0e..=0x11 => transfers.exit(block.id, pc, ExitKind::Return)?,
                0x27 => transfers.exceptions(block.id, None, pc, pc)?,
                0x28..=0x2a => transfers.normal(
                    block.id,
                    pc,
                    NormalEdge::Goto,
                    original
                        .branch_target()
                        .ok_or(bug("goto lost checked target"))?,
                )?,
                0x32..=0x3d => {
                    transfers.normal(
                        block.id,
                        pc,
                        NormalEdge::ConditionalTrue,
                        original
                            .branch_target()
                            .ok_or(bug("branch lost checked target"))?,
                    )?;
                    transfers.normal(
                        block.id,
                        pc,
                        NormalEdge::ConditionalFalse,
                        next(original)?,
                    )?;
                }
                0x2b | 0x2c => {
                    transfers.normal(block.id, pc, NormalEdge::SwitchDefault, next(original)?)?;
                    let payload = original
                        .payload_target()
                        .ok_or(bug("switch lost payload"))?;
                    let item = item_index(flow, payload)?;
                    let DecodedItem::Payload(payload) = &flow.decoded().items()[item] else {
                        return Err(bug("switch target is not a payload"));
                    };
                    let cases = payload
                        .switch_cases(flow.decoded().source())
                        .map_err(super::build::request_error)?
                        .ok_or(bug("switch payload has no cases"))?;
                    for (index, case) in cases.enumerate() {
                        let target = relative(pc, case.offset.0)?;
                        transfers.normal(
                            block.id,
                            pc,
                            NormalEdge::SwitchCase {
                                key: case.key,
                                index: u32::try_from(index)
                                    .map_err(|_| FlowError::PlatformCapacity)?,
                            },
                            target,
                        )?;
                    }
                }
                _ => return Err(bug("terminal source is not a terminal opcode")),
            }
        } else {
            if block.exceptional.is_some() {
                return Err(bug("non-terminal block has phantom exception completion"));
            }
            let index = block
                .operations
                .end
                .checked_sub(1)
                .ok_or(bug("empty fallthrough block"))?;
            let operation = &flow.operations()[index as usize];
            let last = instruction(flow, operation.result_pc.unwrap_or(operation.pc))?;
            transfers.normal(block.id, operation.pc, NormalEdge::Fallthrough, next(last)?)?;
        }
    }
    transfers.finish()
}

pub(super) fn instruction<'f, 'a>(
    flow: &'f FlowMethod<'a>,
    pc: CodeUnitOffset,
) -> Result<&'f DecodedInstruction, FlowError> {
    flow.decoded()
        .instruction_at(pc)
        .ok_or(bug("execution site is not a P1 instruction"))
}

pub(super) fn next(instruction: &DecodedInstruction) -> Result<CodeUnitOffset, FlowError> {
    instruction
        .pc()
        .checked_advance(instruction.width())
        .ok_or(bug("checked instruction end overflowed"))
}

pub(super) fn relative(pc: CodeUnitOffset, delta: i32) -> Result<CodeUnitOffset, FlowError> {
    u32::try_from(i64::from(pc.0) + i64::from(delta))
        .map(CodeUnitOffset)
        .map_err(|_| bug("checked relative target overflowed"))
}

fn item_index(flow: &FlowMethod<'_>, pc: CodeUnitOffset) -> Result<usize, FlowError> {
    flow.decoded()
        .items()
        .binary_search_by_key(&pc, DecodedItem::pc)
        .map_err(|_| bug("execution site is not a P1 item"))
}

fn claim(
    flow: &FlowMethod<'_>,
    claims: &mut [Option<Coverage>],
    pc: CodeUnitOffset,
    coverage: Coverage,
) -> Result<(), FlowError> {
    let index = item_index(flow, pc)?;
    if claims[index].replace(coverage).is_some() {
        return Err(bug("original instruction has multiple execution owners"));
    }
    Ok(())
}

fn check_span(flow: &FlowMethod<'_>, span: super::SlotSpan) -> Result<(), FlowError> {
    let end = (span.start as usize)
        .checked_add(span.len as usize)
        .ok_or(bug("slot span arithmetic overflowed"))?;
    if end > flow.slots().len() {
        return Err(bug("slot span is outside the arena"));
    }
    Ok(())
}

fn check_coverage_and_order(flow: &FlowMethod<'_>) -> Result<(), FlowError> {
    if flow
        .blocks()
        .first()
        .is_none_or(|block| block.start_pc != CodeUnitOffset(0))
    {
        return Err(bug("flow has no pc-zero entry block"));
    }
    if flow.coverage().len() != flow.decoded().items().len() {
        return Err(bug("coverage does not align with P1 items"));
    }
    let mut claims = filled(flow.coverage().len(), None)?;
    let mut operation_cursor = 0usize;
    let mut previous_start = None;
    for (index, block) in flow.blocks().iter().enumerate() {
        if block.id.0 as usize != index || previous_start.is_some_and(|pc| pc >= block.start_pc) {
            return Err(bug("block identities or PC order disagree"));
        }
        previous_start = Some(block.start_pc);
        if block.operations.start as usize != operation_cursor
            || block.operations.end < block.operations.start
            || block.operations.end as usize > flow.operations().len()
        {
            return Err(bug("block ranges do not partition the operation arena"));
        }
        let mut pc = block.start_pc;
        for index in block.operations.clone() {
            let operation = &flow.operations()[index as usize];
            if operation.normal.reads != super::ReadOrder::CaptureAllBeforeCommit {
                return Err(bug("operation does not capture inputs before committing"));
            }
            if operation.id.0 != index || operation.block != block.id || operation.pc != pc {
                return Err(bug(
                    "operation identity, owner or execution order disagrees",
                ));
            }
            check_span(flow, operation.reads())?;
            check_span(flow, operation.writes())?;
            let original = instruction(flow, pc)?;
            if operation.result_pc != original.result_pc() {
                return Err(bug("P2 result site disagrees with its original producer"));
            }
            claim(flow, &mut claims, pc, Coverage::Operation(operation.id))?;
            pc = next(original)?;
            if let Some(result_pc) = operation.result_pc {
                let result = instruction(flow, result_pc)?;
                if result_pc != pc
                    || result.producer_pc() != Some(operation.pc)
                    || !matches!(result.opcode(), 0x0a..=0x0c)
                {
                    return Err(bug(
                        "producer/result atom is not the original adjacent pair",
                    ));
                }
                claim(
                    flow,
                    &mut claims,
                    result_pc,
                    Coverage::MergedResult(operation.id),
                )?;
                pc = next(result)?;
            }
        }
        operation_cursor = block.operations.end as usize;
        if block.terminal.is_some() != block.terminal_pc.is_some() {
            return Err(bug("terminal and source site disagree"));
        }
        if let Some(terminal_pc) = block.terminal_pc {
            if terminal_pc != pc {
                return Err(bug("terminal is not after the block's ordered operations"));
            }
            claim(flow, &mut claims, pc, Coverage::Terminal(block.id))?;
        } else if block.operations.is_empty() {
            return Err(bug("empty non-terminal block"));
        }
        check_span(flow, block.terminal_reads())?;
        if block.terminal.is_none() && !block.terminal_reads().is_empty() {
            return Err(bug("non-terminal block contains phantom terminal reads"));
        }
    }
    if operation_cursor != flow.operations().len() {
        return Err(bug("operation arena contains unowned operations"));
    }
    for (index, item) in flow.decoded().items().iter().enumerate() {
        let expected = match item {
            DecodedItem::Payload(_) => {
                if claims[index].is_some() {
                    return Err(bug("payload has an execution owner"));
                }
                Coverage::Payload
            }
            DecodedItem::Instruction(_) if flow.decoded().is_reachable(item.pc()) => {
                claims[index].ok_or(bug("reachable instruction has no execution owner"))?
            }
            DecodedItem::Instruction(_) => {
                if claims[index].is_some() {
                    return Err(bug("unreachable instruction has an execution owner"));
                }
                Coverage::Unreachable
            }
        };
        if flow.coverage()[index] != expected {
            return Err(bug("coverage points to the wrong execution owner"));
        }
    }
    for slot in flow.slots() {
        if slot
            .end()
            .is_none_or(|end| u64::from(end.0) > flow.decoded().registers_size().0)
        {
            let issue_at = |pc| {
                flow.decoded().issues().any(|(at, e)| {
                    at == pc
                        && matches!(
                            e,
                            crate::p1::MethodError::Malformed {
                                problem: crate::p1::MethodProblem::InvalidRegisterSpan,
                                ..
                            }
                        )
                })
            };
            let explained = flow.operations().iter().any(|op| {
                (issue_at(op.pc()) || op.result_pc().is_some_and(issue_at))
                    && (flow.writes(op).contains(slot)
                        || flow.reads(op).is_ok_and(|r| r.contains(slot)))
            }) || flow.blocks().iter().any(|b| {
                b.terminal_pc().is_some_and(issue_at) && flow.terminal_reads(b).contains(slot)
            });
            if !explained {
                return Err(bug("out-of-frame slot has no original register issue"));
            }
        }
    }
    Ok(())
}

// EdgeId is excluded: identity must be unique, while these keys express the
// semantic identity of each original branch/case/handler transfer.
#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
struct Key {
    block: BlockId,
    site: CodeUnitOffset,
    family: u8,
    ordinal: u32,
}

fn normal_key(block: BlockId, site: CodeUnitOffset, role: NormalEdge) -> Key {
    let (family, ordinal) = match role {
        NormalEdge::Fallthrough => (0, 0),
        NormalEdge::Goto => (1, 0),
        NormalEdge::ConditionalTrue => (2, 0),
        NormalEdge::ConditionalFalse => (3, 0),
        NormalEdge::SwitchDefault => (4, 0),
        NormalEdge::SwitchCase { index, .. } => (5, index),
    };
    Key {
        block,
        site,
        family,
        ordinal,
    }
}

struct Transfers<'f, 'a> {
    flow: &'f FlowMethod<'a>,
    keys: Vec<(Key, usize)>,
    seen: Vec<bool>,
    exit_keys: Vec<((BlockId, CodeUnitOffset, u8), ExitId)>,
    exits_seen: Vec<bool>,
}

impl<'f, 'a> Transfers<'f, 'a> {
    fn new(flow: &'f FlowMethod<'a>) -> Result<Self, FlowError> {
        let mut keys = Vec::new();
        for (index, edge) in flow.edges().iter().enumerate() {
            if edge.id.0 as usize != index {
                return Err(bug("edge identity is not its arena index"));
            }
            let block = flow
                .block(edge.from)
                .ok_or(bug("edge source is a foreign block"))?;
            let key = match edge.role {
                EdgeRole::Normal(role) => {
                    let site = if let Some(pc) = block.terminal_pc {
                        pc
                    } else {
                        let last = block
                            .operations
                            .end
                            .checked_sub(1)
                            .ok_or(bug("edge leaves empty block"))?;
                        flow.operations()[last as usize].pc
                    };
                    normal_key(edge.from, site, role)
                }
                EdgeRole::Exception {
                    throw_pc, order, ..
                } => Key {
                    block: edge.from,
                    site: throw_pc,
                    family: 6,
                    ordinal: order,
                },
            };
            push(&mut keys, (key, index))?;
        }
        keys.sort_unstable_by_key(|(key, _)| *key);

        for pair in keys.windows(2) {
            if pair[0].0 == pair[1].0 {
                return Err(bug("duplicate semantic edge replaces an original transfer"));
            }
        }
        let mut exit_keys = Vec::new();
        for (index, exit) in flow.exits().iter().enumerate() {
            if exit.id.0 as usize != index || flow.block(exit.block).is_none() {
                return Err(bug("exit identity or owner is foreign"));
            }
            let kind = if exit.kind == ExitKind::Return { 0 } else { 1 };
            push(&mut exit_keys, ((exit.block, exit.pc, kind), exit.id))?;
        }
        exit_keys.sort_unstable_by_key(|(key, _)| *key);

        for pair in exit_keys.windows(2) {
            if pair[0].0 == pair[1].0 {
                return Err(bug("duplicate exit for the same original site"));
            }
        }
        Ok(Self {
            flow,
            keys,
            exit_keys,
            seen: filled(flow.edges().len(), false)?,
            exits_seen: filled(flow.exits().len(), false)?,
        })
    }

    fn block_at(&self, pc: CodeUnitOffset) -> Result<BlockId, FlowError> {
        self.flow
            .blocks()
            .binary_search_by_key(&pc, |block| block.start_pc)
            .map(|index| self.flow.blocks()[index].id)
            .map_err(|_| bug("transfer enters inside a block/atom"))
    }

    fn exit(
        &mut self,
        block: BlockId,
        pc: CodeUnitOffset,
        kind: ExitKind,
    ) -> Result<(), FlowError> {
        self.exit_id(block, pc, kind).map(|_| ())
    }

    fn exit_id(
        &mut self,
        block: BlockId,
        pc: CodeUnitOffset,
        kind: ExitKind,
    ) -> Result<ExitId, FlowError> {
        let key = (block, pc, if kind == ExitKind::Return { 0 } else { 1 });
        let index = self
            .exit_keys
            .binary_search_by_key(&key, |(key, _)| *key)
            .map_err(|_| bug("original exit has no matching P2 exit"))?;
        let id = self.exit_keys[index].1;
        if std::mem::replace(&mut self.exits_seen[id.0 as usize], true) {
            return Err(bug("exit has multiple transfer owners"));
        }
        Ok(id)
    }

    fn edge(
        &mut self,
        key: Key,
        source: Option<OpId>,
        role: EdgeRole,
        target: EdgeTarget,
    ) -> Result<(), FlowError> {
        let index = self
            .keys
            .binary_search_by_key(&key, |(key, _)| *key)
            .map_err(|_| bug("original transfer has no matching P2 edge"))?;
        let edge_index = self.keys[index].1;
        let edge = &self.flow.edges()[edge_index];
        if edge.from != key.block
            || edge.source_op != source
            || edge.role != role
            || edge.target != target
        {
            return Err(bug(
                "P2 transfer source, role, selection or target disagrees",
            ));
        }
        if std::mem::replace(&mut self.seen[edge_index], true) {
            return Err(bug("edge has multiple original transfer owners"));
        }
        Ok(())
    }

    fn normal(
        &mut self,
        block: BlockId,
        site: CodeUnitOffset,
        role: NormalEdge,
        target: CodeUnitOffset,
    ) -> Result<(), FlowError> {
        self.edge(
            normal_key(block, site, role),
            None,
            EdgeRole::Normal(role),
            EdgeTarget::Block(self.block_at(target)?),
        )
    }

    fn exceptions(
        &mut self,
        block: BlockId,
        source: Option<OpId>,
        throw_pc: CodeUnitOffset,
        selection_pc: CodeUnitOffset,
    ) -> Result<(), FlowError> {
        let clauses = self
            .flow
            .decoded()
            .catch_clauses_at(selection_pc)
            .map_err(super::build::request_error)?;
        let mut catch_all = false;
        for (ordinal, (catch, target)) in clauses.iter().enumerate() {
            let order = u32::try_from(ordinal).map_err(|_| FlowError::PlatformCapacity)?;
            catch_all |= catch.is_none();
            let key = Key {
                block,
                site: throw_pc,
                family: 6,
                ordinal: order,
            };
            self.edge(
                key,
                source,
                EdgeRole::Exception {
                    throw_pc,
                    selection_pc,
                    order,
                    catch: catch.map(TypeId),
                },
                EdgeTarget::Block(self.block_at(CodeUnitOffset(*target))?),
            )?;
        }
        if !catch_all {
            let order = u32::try_from(clauses.len()).map_err(|_| FlowError::PlatformCapacity)?;
            let exit = self.exit_id(block, throw_pc, ExitKind::Uncaught)?;
            self.edge(
                Key {
                    block,
                    site: throw_pc,
                    family: 6,
                    ordinal: order,
                },
                source,
                EdgeRole::Exception {
                    throw_pc,
                    selection_pc,
                    order,
                    catch: None,
                },
                EdgeTarget::Exit(exit),
            )?;
        }
        Ok(())
    }

    fn finish(self) -> Result<(), FlowError> {
        for seen in self.seen.iter().chain(self.exits_seen.iter()) {
            if !seen {
                return Err(bug("P2 edge/exit has no original transfer owner"));
            }
        }
        // keys are grouped by source block. Walk the actual graph once, with
        // discovery marking to bound the worklist by the block count.
        let count = self
            .flow
            .blocks()
            .len()
            .checked_add(1)
            .ok_or(FlowError::PlatformCapacity)?;
        let mut offsets = filled(count, 0usize)?;
        for (key, _) in &self.keys {
            let offset = &mut offsets[key.block.0 as usize + 1];
            *offset = offset.checked_add(1).ok_or(FlowError::PlatformCapacity)?;
        }
        for index in 1..offsets.len() {
            offsets[index] = offsets[index]
                .checked_add(offsets[index - 1])
                .ok_or(FlowError::PlatformCapacity)?;
        }
        let mut visited = filled(self.flow.blocks().len(), false)?;
        let mut pending = Vec::new();
        visited[0] = true;
        push(&mut pending, BlockId(0))?;
        while let Some(block) = pending.pop() {
            let range = offsets[block.0 as usize]..offsets[block.0 as usize + 1];
            for (_, index) in &self.keys[range] {
                if let EdgeTarget::Block(target) = self.flow.edges()[*index].target
                    && !std::mem::replace(&mut visited[target.0 as usize], true)
                {
                    push(&mut pending, target)?;
                }
            }
        }
        for seen in visited {
            if !seen {
                return Err(bug("flow has an unreachable block"));
            }
        }
        Ok(())
    }
}

fn check_exception_completion(
    actual: super::ExceptionCompletion,
    site: CodeUnitOffset,
    selection: CodeUnitOffset,
    heap: super::HeapTransfer,
    lock: super::LockTransfer,
) -> Result<(), FlowError> {
    if actual.throw_pc != site
        || actual.selection_pc != selection
        || actual.frame != super::ExceptionFrame::PreserveBeforeOperation
        || actual.heap != heap
        || actual.lock != lock
    {
        return Err(bug(
            "exceptional frame/heap/lock completion differs from DEX",
        ));
    }
    Ok(())
}
