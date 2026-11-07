//! Select enclosing interfaces before their children; commit on stack unwind.
//! No child is constructed against a boundary that its selected parent later
//! replaces. Sibling selections own disjoint whole parts, including empty arms.

use super::super::build::*;
use super::super::dominance::Structure;
use super::super::graph::Graph;
use super::super::*;
use super::completion::CompletionInterface;
use super::parts::*;
use crate::p5::StorageMethod;

#[derive(Clone, Copy, Eq, PartialEq)]
enum Selection {
    Loop(LoopId),
    Synchronized(SyncId),
    Try(TryId),
    Predicate(NodeId),
}
fn selection(c: &Candidate) -> Selection {
    match c.kind {
        RegionKind::Loop(id) => Selection::Loop(id),
        RegionKind::Synchronized(id) => Selection::Synchronized(id),
        RegionKind::Try(id) => Selection::Try(id),
        _ => Selection::Predicate(c.entry),
    }
}

enum Work {
    Scope(Part, Vec<Selection>),
    Candidate(Candidate, Vec<Selection>),
    Commit(Candidate, usize),
    Assemble(Part, usize),
}

pub(super) fn construct(
    storage: &StorageMethod<'_>,
    f: &mut RegionFacts,
    g: &Graph,
    s: &Structure,
    owners: &mut [RegionId],
    root: Part,
) -> Result<RegionId, RegionError> {
    let mut work = Vec::new();
    let mut results = Vec::new();
    push(&mut work, Work::Scope(root, Vec::new()))?;
    while let Some(task) = work.pop() {
        match task {
            Work::Scope(part, excluded) => {
                let interface = CompletionInterface::collect(storage, f, g, &part.nodes)?;
                let mut boundary = interface.boundary(f)?;
                if let Some(follow) = part.follow {
                    boundary[follow.0 as usize] = true;
                }
                let set = mask(f.nodes.len(), &interface.nodes)?;
                let mut proposals =
                    super::candidates::candidates(storage, f, g, s, &part, &boundary)?;
                // Larger enclosing interfaces are selected first. Equal-size
                // protection/loop priorities preserve established scope ownership.
                proposals.sort_unstable_by_key(|c| {
                    (
                        usize::MAX - c.members.len(),
                        match c.kind {
                            RegionKind::Loop(_) => 0,
                            RegionKind::Synchronized(_) => 1,
                            RegionKind::Try(_) => 2,
                            RegionKind::Condition(_) => 3,
                            _ => 4,
                        },
                        match c.kind {
                            RegionKind::Try(id) => {
                                usize::MAX - f.tries[id.0 as usize].handlers.len()
                            }
                            _ => 0,
                        },
                        c.entry.0,
                        match c.successor_boundary {
                            None => 0,
                            Some(SuccessorBoundary::True(link)) => 1u64 + u64::from(link.0),
                            Some(SuccessorBoundary::False(link)) => {
                                (1u64 << 32) + u64::from(link.0)
                            }
                        },
                        c.follow.map(|n| n.0),
                    )
                });
                let mut occupied = filled(f.nodes.len(), false)?;
                let mut selected = Vec::new();
                for mut candidate in proposals {
                    if excluded.contains(&selection(&candidate))
                        || match candidate.kind {
                            RegionKind::Condition(id) => {
                                f.conditions[id.0 as usize].tests.iter().any(|test| {
                                    excluded.contains(&Selection::Predicate(test.predicate))
                                })
                            }
                            _ => false,
                        }
                        || candidate
                            .members
                            .iter()
                            .any(|n| !set[n.0 as usize] || occupied[n.0 as usize])
                    {
                        continue;
                    }
                    if let Some(parts) =
                        super::switch::private_switch_parts(storage, f, g, &candidate, &boundary)?
                    {
                        candidate.parts = parts;
                        if let RegionKind::Switch { private_cases, .. } = &mut candidate.kind {
                            *private_cases = true;
                        }
                    }
                    let completion =
                        CompletionInterface::collect(storage, f, g, &candidate.members)?;
                    let accepted = match super::accept::accept(
                        storage,
                        f,
                        g,
                        owners,
                        &candidate,
                        &completion,
                    )? {
                        Ok(()) => super::accept::match_branch_loops(f, &candidate)?,
                        Err(reason) => Err(reason),
                    };
                    if let Err(reason) = accepted {
                        super::trace::rejected(
                            candidate.entry,
                            &reason,
                            candidate.members.len(),
                            candidate.follow,
                            candidate.successor_boundary.is_some(),
                        );
                        push(
                            &mut f.issues,
                            PlanningIssue::Graph {
                                entry: candidate.entry,
                                reason,
                            },
                        )?;
                        continue;
                    }
                    for &node in &completion.nodes {
                        occupied[node.0 as usize] = true;
                    }
                    push(&mut selected, candidate)?;
                }
                let count = selected.len();
                push(&mut work, Work::Assemble(part, count))?;
                for candidate in selected.into_iter().rev() {
                    push(&mut work, Work::Candidate(candidate, copy(&excluded)?))?;
                }
            }
            Work::Candidate(mut candidate, mut excluded) => {
                push(&mut excluded, selection(&candidate))?;
                if let RegionKind::Condition(id) = candidate.kind {
                    for test in &f.conditions[id.0 as usize].tests {
                        push(&mut excluded, Selection::Predicate(test.predicate))?;
                    }
                }
                let mut parts = std::mem::take(&mut candidate.parts);
                if let RegionKind::Loop(id) = candidate.kind
                    && let Some(body) = parts.first_mut()
                {
                    body.completion = PartCompletion::LoopBody(id);
                }
                let count = parts.len();
                push(&mut work, Work::Commit(candidate, count))?;
                for part in parts.into_iter().rev() {
                    push(&mut work, Work::Scope(part, copy(&excluded)?))?;
                }
            }
            Work::Commit(candidate, count) => {
                let start = results
                    .len()
                    .checked_sub(count)
                    .ok_or(bug("region child result underflow"))?;
                let children = copy(&results[start..])?;
                results.truncate(start);
                if let Err(reason) = super::accept::match_successor_boundary(f, owners, &candidate)?
                {
                    super::trace::rejected(
                        candidate.entry,
                        &reason,
                        candidate.members.len(),
                        candidate.follow,
                        candidate.successor_boundary.is_some(),
                    );
                    push(
                        &mut f.issues,
                        PlanningIssue::Graph {
                            entry: candidate.entry,
                            reason,
                        },
                    )?;
                    // The interface and its child ownership remain intact on
                    // refusal. Assemble those same children once; no alternate
                    // boundary, analysis or reconstruction is attempted.
                    let region = super::stack::compose(
                        storage,
                        f,
                        g,
                        owners,
                        Part {
                            nodes: candidate.members,
                            entry: Some(candidate.entry),
                            follow: candidate.follow,
                            completion: PartCompletion::Ordinary,
                        },
                    )?;
                    push(&mut results, region)?;
                    continue;
                }
                let region = add(
                    f,
                    candidate.kind,
                    Some(candidate.entry),
                    candidate.follow,
                    children,
                    copy(&candidate.members)?,
                )?;
                for node in candidate.members {
                    owners[node.0 as usize] = region;
                }
                push(&mut results, region)?;
            }
            Work::Assemble(part, count) => {
                let start = results
                    .len()
                    .checked_sub(count)
                    .ok_or(bug("region scope result underflow"))?;
                results.truncate(start);
                let region = super::stack::compose(storage, f, g, owners, part)?;
                push(&mut results, region)?;
            }
        }
    }
    if results.len() != 1 {
        return Err(bug("region construction lost its root result"));
    }
    results.pop().ok_or(bug("region construction has no root"))
}
