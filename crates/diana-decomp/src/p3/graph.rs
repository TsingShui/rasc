use super::scratch::{bug, filled, id, push};
use super::*;
use crate::p2::{EdgeRole, EdgeTarget, ExecutionFacts, ExitKind};

pub(super) struct Csr {
    offsets: Vec<usize>,
    items: Vec<usize>,
}
impl Csr {
    pub fn new(
        n: usize,
        pairs: impl Iterator<Item = (usize, usize)> + Clone,
    ) -> Result<Self, SsaError> {
        let mut offsets = filled(n.checked_add(1).ok_or(SsaError::PlatformCapacity)?, 0usize)?;
        for (from, _) in pairs.clone() {
            if from >= n {
                return Err(bug("adjacency source outside point arena"));
            }
            offsets[from + 1] = offsets[from + 1]
                .checked_add(1)
                .ok_or(SsaError::PlatformCapacity)?;
        }
        for i in 1..=n {
            offsets[i] = offsets[i]
                .checked_add(offsets[i - 1])
                .ok_or(SsaError::PlatformCapacity)?;
        }
        let mut items = filled(offsets[n], 0)?;
        let mut cursor = filled(n, 0)?;
        for (from, value) in pairs {
            items[offsets[from] + cursor[from]] = value;
            cursor[from] += 1;
        }
        Ok(Self { offsets, items })
    }
    pub fn row(&self, node: usize) -> &[usize] {
        &self.items[self.offsets[node]..self.offsets[node + 1]]
    }
}

pub(super) struct Graph {
    pub outgoing: Csr,
    pub incoming: Csr,
    pub terminals: Vec<Option<PointId>>,
}
impl Graph {
    pub fn index(f: &SsaFacts) -> Result<Self, SsaError> {
        let n = f.points.len();
        let outgoing = Csr::new(
            n,
            f.transfers.iter().enumerate().filter_map(|(i, t)| {
                matches!(t.target, Target::Point(_)).then_some((t.from.0 as usize, i))
            }),
        )?;
        let incoming = Csr::new(
            n,
            f.transfers
                .iter()
                .enumerate()
                .filter_map(|(i, t)| match t.target {
                    Target::Point(p) => Some((p.0 as usize, i)),
                    _ => None,
                }),
        )?;
        let mut terminals = filled(f.block_entries.len(), None)?;
        for point in &f.points {
            if let PointKind::Terminal(b) = point.kind {
                terminals[b.0 as usize] = Some(point.id);
            }
        }
        Ok(Self {
            outgoing,
            incoming,
            terminals,
        })
    }
    pub fn successor(f: &SsaFacts, transfer: usize) -> usize {
        match f.transfers[transfer].target {
            Target::Point(p) => p.0 as usize,
            _ => unreachable!("CSR excludes exits"),
        }
    }
}

fn point(f: &mut SsaFacts, kind: PointKind) -> Result<PointId, SsaError> {
    let p = PointId(id(f.points.len())?);
    push(
        &mut f.points,
        StatePoint {
            id: p,
            kind,
            idom: None,
            before: 0,
            after: 0,
        },
    )?;
    Ok(p)
}
fn transfer(
    f: &mut SsaFacts,
    from: PointId,
    target: Target,
    origin: TransferOrigin,
) -> Result<(), SsaError> {
    let t = TransferId(id(f.transfers.len())?);
    push(
        &mut f.transfers,
        StateTransfer {
            id: t,
            from,
            target,
            origin,
        },
    )
}

pub(super) fn construct(flow: ExecutionFacts<'_, '_>, f: &mut SsaFacts) -> Result<Graph, SsaError> {
    let entry = point(f, PointKind::Entry)?;
    for block in flow.blocks() {
        let p = point(f, PointKind::BlockEntry(block.id()))?;
        push(&mut f.block_entries, p)?;
    }
    let first = *f
        .block_entries
        .first()
        .ok_or(bug("SSA input has no entry block"))?;
    transfer(f, entry, Target::Point(first), TransferOrigin::Entry)?;
    f.terminals = filled(flow.blocks().len(), None)?;
    let mut tails = filled(flow.blocks().len(), entry)?;
    let mut terminals = filled(flow.blocks().len(), None)?;
    for block in flow.blocks() {
        let mut previous = f.block_entries[block.id().0 as usize];
        for index in block.operations() {
            let op = &flow.operations()[index as usize];
            let execute = point(f, PointKind::Execute(op.id()))?;
            transfer(
                f,
                previous,
                Target::Point(execute),
                TransferOrigin::Continue,
            )?;
            if flow.writes(op).is_empty() != (op.normal().commit() == crate::p2::CommitRule::None)
                || (!flow.writes(op).is_empty() && op.normal().commit_pc().is_none())
            {
                return Err(bug("P2 output and explicit normal commit disagree"));
            }
            let commit = if flow.writes(op).is_empty() {
                None
            } else {
                let commit = point(f, PointKind::Commit(op.id()))?;
                transfer(
                    f,
                    execute,
                    Target::Point(commit),
                    TransferOrigin::NormalCommit(op.id()),
                )?;
                Some(commit)
            };
            push(
                &mut f.operations,
                OperationSite {
                    operation: op.id(),
                    execute,
                    commit,
                    commit_rule: op.normal().commit(),
                    commit_pc: op.normal().commit_pc(),
                    use_start: 0,
                    use_len: 0,
                    inputs_available: flow.reads(op).is_ok(),
                    output_start: 0,
                    output_len: 0,
                },
            )?;
            previous = commit.unwrap_or(execute);
        }
        if block.terminal().is_some() {
            let terminal = point(f, PointKind::Terminal(block.id()))?;
            transfer(
                f,
                previous,
                Target::Point(terminal),
                TransferOrigin::Continue,
            )?;
            terminals[block.id().0 as usize] = Some(terminal);
            f.terminals[block.id().0 as usize] = Some(TerminalSite {
                block: block.id(),
                point: terminal,
                use_start: 0,
                use_len: 0,
            });
            previous = terminal;
        }
        tails[block.id().0 as usize] = previous;
    }
    for edge in flow.edges() {
        let from = match edge.role() {
            EdgeRole::Normal(_) => tails[edge.from().0 as usize],
            EdgeRole::Exception { .. } => match edge.source_op() {
                Some(op) => f.operations[op.0 as usize].execute,
                None => terminals[edge.from().0 as usize]
                    .ok_or(bug("exception terminal has no state point"))?,
            },
        };
        let target = match edge.target() {
            EdgeTarget::Block(b) => Target::Point(f.block_entries[b.0 as usize]),
            EdgeTarget::Exit(e) => Target::Exit(e),
        };
        transfer(f, from, target, TransferOrigin::Edge(edge.id()))?;
    }
    for exit in flow.exits() {
        if exit.kind() == ExitKind::Return {
            let from =
                terminals[exit.block().0 as usize].ok_or(bug("return has no state point"))?;
            transfer(
                f,
                from,
                Target::Exit(exit.id()),
                TransferOrigin::Return(exit.id()),
            )?;
        }
    }
    Graph::index(f)
}
