use std::collections::BTreeMap;

use super::{
    BinaryForm, BlockId, Coverage, Edge, EdgeRole, EdgeTarget, ExceptionSelection, FieldAccess,
    FlowError, FlowMethod, NormalEdge, OpKind, Terminal,
};
use crate::{CodeUnitOffset, RunContext};

pub(crate) fn verify(flow: &FlowMethod<'_>, control: &mut RunContext<'_>) -> Result<(), FlowError> {
    if flow.blocks().is_empty() {
        return Err(FlowError::InternalInvariant("flow has no entry block"));
    }
    if flow.blocks()[0].start_pc != crate::CodeUnitOffset(0) {
        return Err(FlowError::InternalInvariant(
            "flow entry block does not start at pc zero",
        ));
    }
    let block_count = flow.blocks().len();
    if flow.coverage().len() != flow.decoded().items().len() {
        return Err(FlowError::InternalInvariant(
            "coverage directory does not align with P1 items",
        ));
    }
    for (index, edge) in flow.edges().iter().enumerate() {
        if edge.id.0 as usize != index {
            return Err(FlowError::InternalInvariant(
                "edge identity is not its construction index",
            ));
        }
    }
    for block in flow.blocks() {
        if block.id.0 as usize >= block_count {
            return Err(FlowError::InternalInvariant(
                "block identity is out of range",
            ));
        }
        if block.operations.start > block.operations.end
            || block.operations.end as usize > flow.operations().len()
        {
            return Err(FlowError::InternalInvariant(
                "block operation range is invalid",
            ));
        }
        for operation_index in block.operations.clone() {
            let operation = &flow.operations()[operation_index as usize];
            if operation.block != block.id {
                return Err(FlowError::InternalInvariant(
                    "operation belongs to a different block",
                ));
            }
            if operation.reads().start as usize + operation.reads().len() as usize
                > flow.slots().len()
                || operation.writes().start as usize + operation.writes().len() as usize
                    > flow.slots().len()
            {
                return Err(FlowError::InternalInvariant(
                    "operation slot span is outside the slot arena",
                ));
            }
        }
        if block.terminal_reads().start as usize + block.terminal_reads().len() as usize
            > flow.slots().len()
        {
            return Err(FlowError::InternalInvariant(
                "terminal slot span is outside the slot arena",
            ));
        }
    }
    for block in flow.blocks() {
        if block.terminal.is_some() != block.terminal_pc.is_some() {
            return Err(FlowError::InternalInvariant(
                "block terminal and terminal pc disagree",
            ));
        }
    }
    let mut reachable_blocks = vec![false; block_count];
    let mut pending = vec![BlockId(0)];
    while let Some(block) = pending.pop() {
        if block.0 as usize >= block_count || reachable_blocks[block.0 as usize] {
            continue;
        }
        reachable_blocks[block.0 as usize] = true;
        for edge in flow.edges() {
            if edge.from != block {
                continue;
            }
            if let EdgeTarget::Block(target) = edge.target {
                pending.push(target);
            }
        }
    }
    if reachable_blocks.iter().any(|reachable| !reachable) {
        for reachable in &reachable_blocks {
            if !reachable {}
        }
        return Err(FlowError::InternalInvariant(
            "flow contains a block unreachable from the entry",
        ));
    }
    let mut covered_ops = Vec::new();
    for coverage in flow.coverage() {
        if let Coverage::Operation(id) | Coverage::MergedResult(id) = coverage {
            if id.0 as usize >= flow.operations().len() {
                return Err(FlowError::InternalInvariant(
                    "coverage names a foreign operation",
                ));
            }
            covered_ops.push(*id);
        }
    }
    for (index, item) in flow.decoded().items().iter().enumerate() {
        let coverage = flow.coverage()[index];
        let reachable = flow.decoded().is_reachable(item.pc());
        match coverage {
            Coverage::Payload => {
                if !matches!(item, crate::p1::DecodedItem::Payload(_)) {
                    return Err(FlowError::InternalInvariant(
                        "instruction is marked as a payload",
                    ));
                }
            }
            Coverage::Unreachable => {
                if reachable {
                    return Err(FlowError::InternalInvariant(
                        "reachable item has no execution fact",
                    ));
                }
            }
            Coverage::Operation(_) | Coverage::MergedResult(_) | Coverage::Terminal(_) => {
                if !reachable {
                    return Err(FlowError::InternalInvariant(
                        "unreachable item has an execution fact",
                    ));
                }
            }
        }
    }
    for edge in flow.edges() {
        if edge.id.0 as usize >= flow.edges().len() {
            return Err(FlowError::InternalInvariant(
                "edge identity is out of range",
            ));
        }
        if edge.from.0 as usize >= block_count {
            return Err(FlowError::InternalInvariant("edge source is not a block"));
        }
        match edge.target {
            EdgeTarget::Block(id) => {
                if id.0 as usize >= block_count {
                    return Err(FlowError::InternalInvariant("edge target is not a block"));
                }
            }
            EdgeTarget::Exit(id) => {
                if id.0 as usize >= flow.exits().len() {
                    return Err(FlowError::InternalInvariant("edge target is not an exit"));
                }
            }
        }
        if let EdgeRole::Exception {
            throw_pc,
            selection_pc,
            ..
        } = edge.role
            && selection_pc.0 < throw_pc.0
        {
            return Err(FlowError::InternalInvariant(
                "exception selection precedes its throw site",
            ));
        }
    }
    let _ = covered_ops;
    check_normal_edges(flow)?;
    check_exception_edges(flow, control)?;
    check_entry_slots(flow)?;
    check_terminal_reads(flow)?;
    check_result_operations(flow)?;
    check_operation_slots(flow)?;
    super::oracle::verify(flow, control)?;
    Ok(())
}

fn check_terminal_reads(flow: &FlowMethod<'_>) -> Result<(), FlowError> {
    for block in flow.blocks() {
        let Some(pc) = block.terminal_pc else {
            continue;
        };
        let Some(instruction) = flow.decoded().instruction_at(pc) else {
            return Err(FlowError::InternalInvariant(
                "terminal pc does not name an instruction",
            ));
        };
        let expected = match instruction.opcode() {
            0x28..=0x2a => Vec::new(),
            0x32..=0x37 => vec![
                crate::RegisterSlot::new(
                    instruction
                        .registers()
                        .next()
                        .ok_or(FlowError::InternalInvariant(
                            "conditional lost its first read",
                        ))?,
                    crate::SlotWidth::One,
                ),
                crate::RegisterSlot::new(
                    instruction
                        .registers()
                        .nth(1)
                        .ok_or(FlowError::InternalInvariant(
                            "conditional lost its second read",
                        ))?,
                    crate::SlotWidth::One,
                ),
            ],
            0x38..=0x3d | 0x2b | 0x2c | 0x27 => vec![crate::RegisterSlot::new(
                instruction
                    .registers()
                    .next()
                    .ok_or(FlowError::InternalInvariant("terminal lost its read"))?,
                crate::SlotWidth::One,
            )],
            0x0e => Vec::new(),
            0x0f => vec![crate::RegisterSlot::new(
                instruction
                    .registers()
                    .next()
                    .ok_or(FlowError::InternalInvariant("return lost its read"))?,
                crate::SlotWidth::One,
            )],
            0x10 => vec![crate::RegisterSlot::new(
                instruction
                    .registers()
                    .next()
                    .ok_or(FlowError::InternalInvariant("return-wide lost its read"))?,
                crate::SlotWidth::Two,
            )],
            0x11 => vec![crate::RegisterSlot::new(
                instruction
                    .registers()
                    .next()
                    .ok_or(FlowError::InternalInvariant("return-object lost its read"))?,
                crate::SlotWidth::One,
            )],
            _ => {
                return Err(FlowError::InternalInvariant(
                    "terminal pc does not name a terminal instruction",
                ));
            }
        };
        if flow.terminal_reads(block) != expected.as_slice() {
            return Err(FlowError::InternalInvariant(
                "terminal reads disagree with the original instruction",
            ));
        }
    }
    Ok(())
}

fn slot_width(slot: crate::RegisterSlot) -> crate::SlotWidth {
    slot.width
}

fn check_operation_slots(flow: &FlowMethod<'_>) -> Result<(), FlowError> {
    let limit = flow.decoded().registers_size().0;
    for operation in flow.operations() {
        for slot in flow
            .reads(operation)
            .iter()
            .chain(flow.writes(operation).iter())
        {
            if slot
                .start
                .0
                .checked_add(slot.words())
                .is_none_or(|end| u64::from(end) > limit)
            {
                return Err(FlowError::InternalInvariant(
                    "operation slot is outside the register frame",
                ));
            }
        }
        let reads = flow.reads(operation);
        let writes = flow.writes(operation);
        let expect = |reads_len: usize, writes_len: usize| -> Result<(), FlowError> {
            if reads.len() != reads_len || writes.len() != writes_len {
                return Err(FlowError::InternalInvariant(
                    "operation read/write count disagrees with its semantics",
                ));
            }
            Ok(())
        };
        match &operation.kind {
            OpKind::Nop => expect(0, 0)?,
            OpKind::Move { kind } => {
                expect(1, 1)?;
                let width = match kind {
                    super::MoveKind::Wide => crate::SlotWidth::Two,
                    _ => crate::SlotWidth::One,
                };
                if slot_width(reads[0]) != width || slot_width(writes[0]) != width {
                    return Err(FlowError::InternalInvariant("move width disagrees"));
                }
            }
            OpKind::Const { width, .. } => {
                expect(0, 1)?;
                if slot_width(writes[0]) != *width {
                    return Err(FlowError::InternalInvariant("constant width disagrees"));
                }
            }
            OpKind::ConstString { .. }
            | OpKind::ConstClass { .. }
            | OpKind::ConstMethodHandle { .. }
            | OpKind::ConstMethodType { .. }
            | OpKind::MoveException => {
                expect(0, 1)?;
            }
            OpKind::Unary { op } => {
                expect(1, 1)?;
                if slot_width(reads[0]) != op.source_width()
                    || slot_width(writes[0]) != op.dest_width()
                {
                    return Err(FlowError::InternalInvariant("unary width disagrees"));
                }
            }
            OpKind::Binary { op, form, .. } => {
                let count = match form {
                    BinaryForm::ThreeAddress | BinaryForm::TwoAddress => 2,
                    BinaryForm::Literal => 1,
                };
                expect(count, 1)?;
                match form {
                    BinaryForm::ThreeAddress | BinaryForm::TwoAddress => {
                        if slot_width(reads[0]) != op.left_width()
                            || slot_width(reads[1]) != op.right_width()
                        {
                            return Err(FlowError::InternalInvariant(
                                "binary operand width disagrees",
                            ));
                        }
                    }
                    BinaryForm::Literal => {
                        if slot_width(reads[0]) != crate::SlotWidth::One {
                            return Err(FlowError::InternalInvariant(
                                "literal operand width disagrees",
                            ));
                        }
                    }
                }
                if slot_width(writes[0]) != op.width() {
                    return Err(FlowError::InternalInvariant(
                        "binary result width disagrees",
                    ));
                }
            }
            OpKind::Compare { op } => {
                expect(2, 1)?;
                let width = match op {
                    super::CompareOp::CmplDouble
                    | super::CompareOp::CmpgDouble
                    | super::CompareOp::CmpLong => crate::SlotWidth::Two,
                    _ => crate::SlotWidth::One,
                };
                if reads.iter().any(|slot| slot_width(*slot) != width)
                    || slot_width(writes[0]) != crate::SlotWidth::One
                {
                    return Err(FlowError::InternalInvariant("compare width disagrees"));
                }
            }
            OpKind::Field {
                access,
                static_field,
                kind,
                ..
            } => {
                let width = kind.width();
                match access {
                    FieldAccess::Get => {
                        expect(if *static_field { 0 } else { 1 }, 1)?;
                        if !*static_field && slot_width(reads[0]) != crate::SlotWidth::One {
                            return Err(FlowError::InternalInvariant(
                                "field receiver is not one word",
                            ));
                        }
                        if slot_width(writes[0]) != width {
                            return Err(FlowError::InternalInvariant(
                                "field result width disagrees",
                            ));
                        }
                    }
                    FieldAccess::Put => {
                        expect(if *static_field { 1 } else { 2 }, 0)?;
                        if slot_width(reads[0]) != width {
                            return Err(FlowError::InternalInvariant(
                                "field value width disagrees",
                            ));
                        }
                        if !*static_field && slot_width(reads[1]) != crate::SlotWidth::One {
                            return Err(FlowError::InternalInvariant(
                                "field receiver is not one word",
                            ));
                        }
                    }
                }
            }
            OpKind::Array { access, kind } => match access {
                super::ArrayAccess::Get => {
                    expect(2, 1)?;
                    if slot_width(reads[0]) != crate::SlotWidth::One
                        || slot_width(reads[1]) != crate::SlotWidth::One
                        || slot_width(writes[0]) != kind.width()
                    {
                        return Err(FlowError::InternalInvariant("array get width disagrees"));
                    }
                }
                super::ArrayAccess::Put => {
                    expect(3, 0)?;
                    if slot_width(reads[0]) != kind.width()
                        || slot_width(reads[1]) != crate::SlotWidth::One
                        || slot_width(reads[2]) != crate::SlotWidth::One
                    {
                        return Err(FlowError::InternalInvariant("array put width disagrees"));
                    }
                }
            },
            OpKind::ArrayLength | OpKind::InstanceOf { .. } => {
                expect(1, 1)?;
                if slot_width(reads[0]) != crate::SlotWidth::One
                    || slot_width(writes[0]) != crate::SlotWidth::One
                {
                    return Err(FlowError::InternalInvariant(
                        "array-length/instance-of width disagrees",
                    ));
                }
            }
            OpKind::Cast { .. } => expect(1, 0)?,
            OpKind::NewInstance { .. } => expect(0, 1)?,
            OpKind::NewArray { .. } => expect(1, 1)?,
            OpKind::FilledNewArray { .. } => {
                if writes.len() > 1 {
                    return Err(FlowError::InternalInvariant(
                        "filled array has too many writes",
                    ));
                }
            }
            OpKind::FillArrayData { .. } => expect(1, 0)?,
            OpKind::Monitor { .. } => expect(1, 0)?,
            OpKind::Invoke { .. } => {
                let layout = flow
                    .decoded()
                    .invoke_layout_at(operation.pc)
                    .ok_or(FlowError::InternalInvariant("invoke has no checked layout"))?;
                let instruction = flow.decoded().instruction_at(operation.pc).ok_or(
                    FlowError::InternalInvariant("invoke pc is not an instruction"),
                )?;
                let mut expected = Vec::new();
                let mut registers = instruction.registers();
                if layout.has_receiver() {
                    expected.push(crate::RegisterSlot::new(
                        registers
                            .next()
                            .ok_or(FlowError::InternalInvariant("invoke receiver missing"))?,
                        crate::SlotWidth::One,
                    ));
                }
                for width in layout.arguments() {
                    let first = registers
                        .next()
                        .ok_or(FlowError::InternalInvariant("invoke argument missing"))?;
                    expected.push(crate::RegisterSlot::new(first, *width));
                    if *width == crate::SlotWidth::Two {
                        let second = registers
                            .next()
                            .ok_or(FlowError::InternalInvariant("invoke wide word missing"))?;
                        if first.0.checked_add(1) != Some(second.0) {
                            return Err(FlowError::InternalInvariant(
                                "invoke wide argument is not adjacent",
                            ));
                        }
                    }
                }
                if registers.next().is_some() || reads != expected.as_slice() {
                    return Err(FlowError::InternalInvariant(
                        "invoke reads disagree with P1 argument layout",
                    ));
                }
            }
        }
        if operation.commit == super::CommitRule::None && !writes.is_empty() {
            return Err(FlowError::InternalInvariant(
                "operation writes without a commit",
            ));
        }
        if operation.commit == super::CommitRule::AfterResult && operation.result_pc.is_none() {
            return Err(FlowError::InternalInvariant(
                "operation commits after a missing result",
            ));
        }
    }
    Ok(())
}

fn check_result_operations(flow: &FlowMethod<'_>) -> Result<(), FlowError> {
    for operation in flow.operations() {
        let Some(result_pc) = operation.result_pc else {
            continue;
        };
        let result =
            flow.decoded()
                .instruction_at(result_pc)
                .ok_or(FlowError::InternalInvariant(
                    "producer result pc does not name an instruction",
                ))?;
        if !matches!(result.opcode(), 0x0a..=0x0c) {
            return Err(FlowError::InternalInvariant(
                "producer result is not a move-result",
            ));
        }
        let dest = result
            .registers()
            .next()
            .ok_or(FlowError::InternalInvariant(
                "move-result lost its destination",
            ))?;
        let width = if result.opcode() == 0x0b {
            crate::SlotWidth::Two
        } else {
            crate::SlotWidth::One
        };
        let expected = crate::RegisterSlot::new(dest, width);
        let writes = flow.writes(operation);
        if writes != [expected] {
            return Err(FlowError::InternalInvariant(
                "producer normal write disagrees with its move-result",
            ));
        }
        if operation.commit != super::CommitRule::AfterResult {
            return Err(FlowError::InternalInvariant(
                "producer result does not commit after move-result",
            ));
        }
    }
    Ok(())
}

fn block_at_pc(flow: &FlowMethod<'_>, pc: CodeUnitOffset) -> Result<BlockId, FlowError> {
    flow.blocks()
        .iter()
        .find(|block| block.start_pc == pc)
        .map(|block| block.id)
        .ok_or(FlowError::InternalInvariant(
            "checked target does not name a block start",
        ))
}

fn normal_edges<'a>(flow: &'a FlowMethod<'_>, block: BlockId) -> Vec<&'a Edge> {
    flow.edges()
        .iter()
        .filter(|edge| edge.from == block && matches!(edge.role, EdgeRole::Normal(_)))
        .collect()
}

fn expected_fallthrough(flow: &FlowMethod<'_>, block: &super::Block) -> Option<CodeUnitOffset> {
    let last = flow
        .operations()
        .get(block.operations.end.checked_sub(1)? as usize)?;
    let after = last.result_pc.unwrap_or(last.pc);
    let instruction = flow.decoded().instruction_at(after)?;
    after.checked_advance(instruction.width())
}

fn check_normal_edges(flow: &FlowMethod<'_>) -> Result<(), FlowError> {
    for block in flow.blocks() {
        let edges = normal_edges(flow, block.id);
        match &block.terminal {
            Some(Terminal::Goto { target }) => {
                if edges.len() != 1
                    || !matches!(edges[0].role, EdgeRole::Normal(NormalEdge::Goto))
                    || edges[0].target != EdgeTarget::Block(block_at_pc(flow, *target)?)
                {
                    return Err(FlowError::InternalInvariant(
                        "goto block has an invalid normal edge",
                    ));
                }
            }
            Some(Terminal::Branch {
                target,
                fallthrough,
                ..
            }) => {
                if edges.len() != 2 {
                    return Err(FlowError::InternalInvariant(
                        "conditional block must have two normal edges",
                    ));
                }
                let mut true_target = None;
                let mut false_target = None;
                for edge in edges {
                    match edge.role {
                        EdgeRole::Normal(NormalEdge::ConditionalTrue) => {
                            true_target = Some(edge.target)
                        }
                        EdgeRole::Normal(NormalEdge::ConditionalFalse) => {
                            false_target = Some(edge.target)
                        }
                        _ => {
                            return Err(FlowError::InternalInvariant(
                                "conditional block has a foreign edge role",
                            ));
                        }
                    }
                }
                if true_target != Some(EdgeTarget::Block(block_at_pc(flow, *target)?))
                    || false_target != Some(EdgeTarget::Block(block_at_pc(flow, *fallthrough)?))
                {
                    return Err(FlowError::InternalInvariant(
                        "conditional block edge targets disagree with P1",
                    ));
                }
            }
            Some(Terminal::Switch {
                kind,
                default,
                cases,
                ..
            }) => {
                if edges.len() != cases.len() + 1 {
                    return Err(FlowError::InternalInvariant(
                        "switch normal edge count disagrees with its cases",
                    ));
                }
                let default_block = block_at_pc(flow, *default)?;
                let mut default_seen = false;
                for edge in edges {
                    match edge.role {
                        EdgeRole::Normal(NormalEdge::SwitchDefault) => {
                            if edge.target != EdgeTarget::Block(default_block) {
                                return Err(FlowError::InternalInvariant(
                                    "switch default target disagrees with P2",
                                ));
                            }
                            default_seen = true;
                        }
                        EdgeRole::Normal(NormalEdge::SwitchCase { key, index }) => {
                            let Some((expected_key, expected_target)) = cases.get(index as usize)
                            else {
                                return Err(FlowError::InternalInvariant(
                                    "switch case index is outside the terminal cases",
                                ));
                            };
                            if key != *expected_key
                                || edge.target
                                    != EdgeTarget::Block(block_at_pc(flow, *expected_target)?)
                            {
                                return Err(FlowError::InternalInvariant(
                                    "switch case edge disagrees with its terminal",
                                ));
                            }
                        }
                        _ => {
                            return Err(FlowError::InternalInvariant(
                                "switch block has a foreign edge role",
                            ));
                        }
                    }
                }
                if !default_seen {
                    return Err(FlowError::InternalInvariant(
                        "switch block has no default edge",
                    ));
                }
                let _ = kind;
            }
            Some(Terminal::Return { .. }) | Some(Terminal::Throw { .. }) => {
                if !edges.is_empty() {
                    return Err(FlowError::InternalInvariant(
                        "abrupt block has a normal successor",
                    ));
                }
            }
            None => {
                if edges.len() != 1
                    || !matches!(edges[0].role, EdgeRole::Normal(NormalEdge::Fallthrough))
                {
                    return Err(FlowError::InternalInvariant(
                        "fallthrough block must have one normal edge",
                    ));
                }
                let expected = expected_fallthrough(flow, block)
                    .ok_or(FlowError::InternalInvariant("fallthrough has no next pc"))?;
                if edges[0].target != EdgeTarget::Block(block_at_pc(flow, expected)?) {
                    return Err(FlowError::InternalInvariant(
                        "fallthrough edge target disagrees with P1",
                    ));
                }
            }
        }
    }
    Ok(())
}

fn check_exception_edges(
    flow: &FlowMethod<'_>,
    control: &mut RunContext<'_>,
) -> Result<(), FlowError> {
    let mut expected =
        BTreeMap::<(BlockId, Option<super::OpId>), Vec<(u32, Option<u32>, Option<u32>)>>::new();
    for block in flow.blocks() {
        for index in block.operations.clone() {
            let operation = &flow.operations()[index as usize];
            if !operation.effects.may_throw() {
                continue;
            }
            let selection = match operation.exception_selection {
                ExceptionSelection::Operation => operation.pc,
                ExceptionSelection::Advanced => {
                    let instruction = flow.decoded().instruction_at(operation.pc).ok_or(
                        FlowError::InternalInvariant("throwing operation has no instruction"),
                    )?;
                    operation
                        .pc
                        .checked_advance(instruction.width())
                        .ok_or(FlowError::PlatformCapacity)?
                }
            };
            let clauses = flow
                .decoded()
                .catch_clauses_at(selection, control)
                .map_err(super::build::request_error)?;
            let list = expected.entry((block.id, Some(operation.id))).or_default();
            let mut catch_all = false;
            for (order, (catch, target)) in clauses.into_iter().enumerate() {
                catch_all |= catch.is_none();
                list.push((
                    u32::try_from(order).map_err(|_| FlowError::PlatformCapacity)?,
                    catch,
                    Some(target),
                ));
            }
            if !catch_all {
                let order = u32::try_from(list.len()).map_err(|_| FlowError::PlatformCapacity)?;
                list.push((order, None, None));
            }
        }
        if let Some(Terminal::Throw { pc }) = block.terminal {
            let clauses = flow
                .decoded()
                .catch_clauses_at(pc, control)
                .map_err(super::build::request_error)?;
            let list = expected.entry((block.id, None)).or_default();
            let mut catch_all = false;
            for (order, (catch, target)) in clauses.into_iter().enumerate() {
                catch_all |= catch.is_none();
                list.push((
                    u32::try_from(order).map_err(|_| FlowError::PlatformCapacity)?,
                    catch,
                    Some(target),
                ));
            }
            if !catch_all {
                let order = u32::try_from(list.len()).map_err(|_| FlowError::PlatformCapacity)?;
                list.push((order, None, None));
            }
        }
    }
    for block in flow.blocks() {
        let mut seen = 0usize;
        for edge in flow.edges() {
            let EdgeRole::Exception { order, catch, .. } = edge.role else {
                continue;
            };
            if edge.from != block.id {
                continue;
            }
            let Some(list) = expected.get(&(block.id, edge.source_op)) else {
                return Err(FlowError::InternalInvariant(
                    "exception edge has no may-throw source",
                ));
            };
            let Some((expected_order, expected_catch, expected_target)) = list.get(order as usize)
            else {
                return Err(FlowError::InternalInvariant(
                    "exception edge order is outside its catch chain",
                ));
            };
            let expected_target = match expected_target {
                Some(target) => EdgeTarget::Block(block_at_pc(flow, CodeUnitOffset(*target))?),
                None => {
                    let EdgeTarget::Exit(id) = edge.target else {
                        return Err(FlowError::InternalInvariant(
                            "uncaught exception edge has a block target",
                        ));
                    };
                    EdgeTarget::Exit(id)
                }
            };
            if *expected_order != order
                || *expected_catch != catch.map(|id| id.0)
                || edge.target != expected_target
            {
                return Err(FlowError::InternalInvariant(
                    "exception edge disagrees with P1 protection",
                ));
            }
            seen += 1;
        }
        let expected_count = expected
            .iter()
            .filter(|((block_id, _), _)| *block_id == block.id)
            .map(|(_, list)| {
                list.len() + usize::from(!list.iter().any(|(_, catch, _)| catch.is_none()))
            })
            .sum::<usize>();
        if seen != expected_count {
            return Err(FlowError::InternalInvariant(
                "exception edge count disagrees with P1 protection",
            ));
        }
    }
    Ok(())
}

fn check_entry_slots(flow: &FlowMethod<'_>) -> Result<(), FlowError> {
    let limit = flow.decoded().registers_size().0;
    let check = |slot: crate::RegisterSlot| -> Result<(), FlowError> {
        if slot
            .start
            .0
            .checked_add(slot.words())
            .is_none_or(|end| u64::from(end) > limit)
        {
            return Err(FlowError::InternalInvariant(
                "entry slot is outside the register frame",
            ));
        }
        Ok(())
    };
    if let Some(slot) = flow.entry().this() {
        check(slot)?;
    }
    for parameter in flow.entry().parameters() {
        check(parameter.slot())?;
    }
    Ok(())
}
