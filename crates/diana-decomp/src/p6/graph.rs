use super::build::*;
use super::*;
use crate::p2::{EdgeRole, EdgeTarget, OpKind};
use crate::p5::{CopyEmission, CopyLocation, StorageMethod, TransferSite};

pub(super) struct Graph {
    pub outgoing: Vec<Vec<LinkId>>,
    pub incoming: Vec<Vec<LinkId>>,
    pub exceptions: Vec<Vec<crate::p2::EdgeId>>,
    pub op_node: Vec<NodeId>,
    pub terminal_node: Vec<NodeId>,
    pub roots: Vec<NodeId>,
}
#[derive(Clone, Copy, Eq, PartialEq)]
struct Guard {
    order: u32,
    catch: Option<crate::TypeId>,
    target: Option<crate::p2::BlockId>,
}
fn node(
    f: &mut RegionFacts,
    block: crate::p2::BlockId,
    operations: std::ops::Range<u32>,
    terminal: bool,
) -> Result<NodeId, RegionError> {
    let n = NodeId(id(f.nodes.len())?);
    push(
        &mut f.nodes,
        Node {
            id: n,
            block,
            operations,
            terminal,
            reachable: false,
        },
    )?;
    push(
        &mut f.labels,
        LabelPlan {
            id: LabelId(n.0),
            node: n,
            block,
            kind: LabelKind::Continuation,
            owner: RegionId(n.0),
        },
    )?;
    Ok(n)
}
fn link(
    f: &mut RegionFacts,
    from: NodeId,
    to: NodeId,
    origin: LinkOrigin,
    edge_copies: &[Vec<crate::p5::PlanId>],
) -> Result<LinkId, RegionError> {
    let id = LinkId(id(f.links.len())?);
    let mut copies = Vec::new();
    if let LinkOrigin::Edge(edge) = origin {
        for &plan in &edge_copies[edge.0 as usize] {
            push(&mut copies, plan)?;
        }
    }
    push(
        &mut f.links,
        NormalLink {
            id,
            from,
            to,
            origin,
        },
    )?;
    push(
        &mut f.link_plans,
        LinkPlan {
            link: id,
            action: TransferAction::Goto(LabelId(to.0)),
            copies,
        },
    )?;
    Ok(id)
}
pub(super) fn construct(
    storage: &StorageMethod<'_>,
    f: &mut RegionFacts,
) -> Result<Graph, RegionError> {
    let flow = storage.execution();
    let mut edge_copies = lists(flow.edges().len())?;
    for plan in storage.facts().copy_plans() {
        if storage.facts().copy_emission(plan.id()) == Some(CopyEmission::Original)
            && let CopyLocation::Transfer {
                site: TransferSite::NormalEdge(edge),
                ..
            } = plan.location()
        {
            push(&mut edge_copies[edge.0 as usize], plan.id())?;
        }
    }
    if flow.blocks().is_empty() {
        return Err(bug("P6 input has no original entry block"));
    }
    let mut guards = lists(flow.operations().len())?;
    for e in flow.edges() {
        if let (Some(op), EdgeRole::Exception { order, catch, .. }) = (e.source_op(), e.role()) {
            push(
                &mut guards[op.0 as usize],
                Guard {
                    order,
                    catch,
                    target: match e.target() {
                        EdgeTarget::Block(b) => Some(b),
                        EdgeTarget::Exit(_) => None,
                    },
                },
            )?;
        }
    }
    let mut op_node = filled(flow.operations().len(), NodeId(0))?;
    let mut terminal_node = filled(flow.blocks().len(), NodeId(0))?;
    for block in flow.blocks() {
        let range = block.operations();
        let mut cuts = Vec::new();
        push(&mut cuts, range.start)?;
        let mut previous_guard = None;
        for index in range.clone() {
            let op = &flow.operations()[index as usize];
            if !guards[index as usize].is_empty() {
                if previous_guard.is_some_and(|p: usize| guards[p] != guards[index as usize]) {
                    push(&mut cuts, index)?;
                }
                previous_guard = Some(index as usize);
            }
            if matches!(op.kind(), OpKind::Monitor { .. }) {
                push(&mut cuts, index)?;
                push(
                    &mut cuts,
                    index
                        .checked_add(1)
                        .ok_or(crate::RequestError::PlatformCapacity)?,
                )?;
            }
        }
        push(&mut cuts, range.end)?;
        cuts.sort_unstable();
        cuts.dedup();
        let mut parts = Vec::new();
        for pair in cuts.windows(2) {
            if pair[0] == pair[1] {
                continue;
            }
            let n = node(f, block.id(), pair[0]..pair[1], false)?;
            for index in pair[0]..pair[1] {
                op_node[index as usize] = n;
            }
            push(&mut parts, n)?;
        }
        if block.terminal().is_some() || parts.is_empty() {
            let n = node(
                f,
                block.id(),
                range.end..range.end,
                block.terminal().is_some(),
            )?;
            push(&mut parts, n)?;
        }
        terminal_node[block.id().0 as usize] =
            *parts.last().ok_or(bug("P6 block lost its portions"))?;
        f.labels[parts[0].0 as usize].kind = LabelKind::BlockEntry;
        push(
            &mut f.blocks,
            BlockPlan {
                block: block.id(),
                parts,
            },
        )?;
    }
    for b in 0..f.blocks.len() {
        for i in 1..f.blocks[b].parts.len() {
            link(
                f,
                f.blocks[b].parts[i - 1],
                f.blocks[b].parts[i],
                LinkOrigin::WithinBlock(f.blocks[b].block),
                &edge_copies,
            )?;
        }
    }
    let mut exceptions = lists(f.nodes.len())?;
    for edge in flow.edges() {
        let route = match edge.role() {
            EdgeRole::Normal(_) => {
                let EdgeTarget::Block(target) = edge.target() else {
                    return Err(bug("normal P2 edge has no block target"));
                };
                EdgeRoute::Normal(link(
                    f,
                    terminal_node[edge.from().0 as usize],
                    f.blocks[target.0 as usize].parts[0],
                    LinkOrigin::Edge(edge.id()),
                    &edge_copies,
                )?)
            }
            EdgeRole::Exception { order, .. } => {
                let from = edge
                    .source_op()
                    .map_or(terminal_node[edge.from().0 as usize], |op| {
                        op_node[op.0 as usize]
                    });
                push(&mut exceptions[from.0 as usize], edge.id())?;
                EdgeRoute::Exception {
                    site: ThrowSiteId(0),
                    order,
                    delivery: ExceptionDelivery::Explicit,
                }
            }
        };
        push(
            &mut f.edges,
            EdgePlan {
                edge: edge.id(),
                route,
            },
        )?;
    }
    let mut outgoing = lists(f.nodes.len())?;
    let mut incoming = lists(f.nodes.len())?;
    for e in &f.links {
        push(&mut outgoing[e.from.0 as usize], e.id)?;
        push(&mut incoming[e.to.0 as usize], e.id)?;
    }
    let entry = f.blocks[0].parts[0];
    let mut queue = Vec::new();
    push(&mut queue, entry)?;
    f.nodes[entry.0 as usize].reachable = true;
    while let Some(n) = queue.pop() {
        for &l in &outgoing[n.0 as usize] {
            let target = f.links[l.0 as usize].to;
            if !f.nodes[target.0 as usize].reachable {
                f.nodes[target.0 as usize].reachable = true;
                push(&mut queue, target)?;
            }
        }
        for &e in &exceptions[n.0 as usize] {
            if let EdgeTarget::Block(b) = flow.edges()[e.0 as usize].target() {
                let target = f.blocks[b.0 as usize].parts[0];
                if !f.nodes[target.0 as usize].reachable {
                    f.nodes[target.0 as usize].reachable = true;
                    push(&mut queue, target)?;
                }
            }
        }
    }
    let mut roots = Vec::new();
    push(&mut roots, entry)?;
    for e in flow.exception_edges() {
        if let EdgeTarget::Block(b) = e.target() {
            let target = f.blocks[b.0 as usize].parts[0];
            if !roots.contains(&target) {
                push(&mut roots, target)?;
            }
        }
    }
    let mut seen = filled(f.nodes.len(), false)?;
    for ordinal in 0..=f.nodes.len() {
        if ordinal == 0 {
            for &r in &roots {
                if !seen[r.0 as usize] {
                    seen[r.0 as usize] = true;
                    push(&mut queue, r)?;
                }
            }
        } else {
            let n = NodeId(id(ordinal - 1)?);
            if !seen[n.0 as usize] {
                push(&mut roots, n)?;
                seen[n.0 as usize] = true;
                push(&mut queue, n)?;
            }
        }
        while let Some(n) = queue.pop() {
            for &l in &outgoing[n.0 as usize] {
                let q = f.links[l.0 as usize].to;
                if !seen[q.0 as usize] {
                    seen[q.0 as usize] = true;
                    push(&mut queue, q)?;
                }
            }
        }
    }
    for plan in storage.facts().copy_plans() {
        push(
            &mut f.copies,
            CopyPlacement {
                plan: plan.id(),
                location: plan.location(),
                construction: match storage.facts().copy_emission(plan.id()) {
                    Some(CopyEmission::Construction { group }) => Some(group),
                    _ => None,
                },
            },
        )?;
    }
    for exit in flow.exits() {
        push(&mut f.exits, exit.id())?;
    }
    for op in flow.operations() {
        if let Some(uses) = storage.facts().operation_uses(op.id()) {
            match uses {
                Err(gap) => push(
                    &mut f.issues,
                    PlanningIssue::Storage {
                        operation: Some(op.id()),
                        block: op.block(),
                        gap,
                    },
                )?,
                Ok(uses) => {
                    for u in uses {
                        if let Err(gap) = u.read() {
                            push(
                                &mut f.issues,
                                PlanningIssue::Storage {
                                    operation: Some(op.id()),
                                    block: op.block(),
                                    gap,
                                },
                            )?;
                        }
                    }
                }
            }
        }
    }
    for block in flow.blocks() {
        if let Some(uses) = storage.facts().terminal_uses(block.id()) {
            for u in uses {
                if let Err(gap) = u.read() {
                    push(
                        &mut f.issues,
                        PlanningIssue::Storage {
                            operation: None,
                            block: block.id(),
                            gap,
                        },
                    )?;
                }
            }
        }
    }
    Ok(Graph {
        outgoing,
        incoming,
        exceptions,
        op_node,
        terminal_node,
        roots,
    })
}
impl Graph {
    pub fn entry(f: &RegionFacts, block: crate::p2::BlockId) -> NodeId {
        f.blocks[block.0 as usize].parts[0]
    }
    pub fn exception_target(
        f: &RegionFacts,
        storage: &StorageMethod<'_>,
        edge: crate::p2::EdgeId,
    ) -> Option<NodeId> {
        match storage.execution().edges()[edge.0 as usize].target() {
            EdgeTarget::Block(b) => Some(Self::entry(f, b)),
            EdgeTarget::Exit(_) => None,
        }
    }
    pub fn external_entry(&self, f: &RegionFacts, set: &[bool], entry: NodeId) -> bool {
        f.links
            .iter()
            .any(|l| set[l.to.0 as usize] && !set[l.from.0 as usize] && l.to != entry)
    }
}
