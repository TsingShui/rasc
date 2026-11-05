use super::build::*;
use super::*;
use crate::RequestError;
use crate::p4::Interpretation;
use std::collections::HashMap;

fn root(parent: &mut [usize], mut n: usize) -> usize {
    let start = n;
    while parent[n] != n {
        n = parent[n];
    }
    let mut at = start;
    while parent[at] != at {
        let next = parent[at];
        parent[at] = n;
        at = next;
    }
    n
}
fn union(parent: &mut [usize], a: usize, b: usize) {
    let a = root(parent, a);
    let b = root(parent, b);
    if a != b {
        parent[a] = b;
    }
}
struct Node {
    users: Vec<usize>,
    bad: Option<(PlanId, CopyId)>,
}
fn connect(nodes: &mut [Node], from: usize, to: usize) -> Result<(), StorageError> {
    push(&mut nodes[from].users, to)
}

// Dependency propagation concerns synthetic storage, not SSA/type reconstruction.
// A missing copy can affect its storage-interference component and consumers of
// its destinations, without poisoning an independent original producer's local.
fn dependencies(f: &mut StorageFacts) -> Result<(), StorageError> {
    if f.copies
        .iter()
        .all(|p| p.assignments.iter().all(|a| a.issue.is_none()))
    {
        return Ok(());
    }
    let nlocals = f.locals.len();
    let mut nodes = Vec::new();
    for _ in 0..nlocals {
        push(
            &mut nodes,
            Node {
                users: Vec::new(),
                bad: None,
            },
        )?;
    }
    let mut components = Vec::new();
    for pi in 0..f.copies.len() {
        let plan = &f.copies[pi];
        let count = plan.assignments.len();
        let mut parent = Vec::new();
        for i in 0..count {
            push(&mut parent, i)?;
        }
        let mut touched = HashMap::<LocalId, usize>::new();
        for (i, copy) in plan.assignments.iter().enumerate() {
            for local in [copy.destination.ok(), copy.dependency]
                .into_iter()
                .flatten()
            {
                if let Some(&other) = touched.get(&local) {
                    union(&mut parent, i, other);
                } else {
                    touched.try_reserve(1).map_err(RequestError::Allocation)?;
                    touched.insert(local, i);
                }
            }
        }
        let mut groups = HashMap::new();
        let mut ids = Vec::new();
        for (i, copy) in plan.assignments.iter().enumerate() {
            let group = root(&mut parent, i);
            let node = if let Some(&node) = groups.get(&group) {
                node
            } else {
                groups.try_reserve(1).map_err(RequestError::Allocation)?;
                let node = nodes.len();
                push(
                    &mut nodes,
                    Node {
                        users: Vec::new(),
                        bad: None,
                    },
                )?;
                groups.insert(group, node);
                node
            };
            push(&mut ids, node)?;
            if copy.issue.is_some() {
                nodes[node].bad.get_or_insert((plan.id, copy.id));
            }
            if let Some(local) = copy.dependency {
                connect(&mut nodes, local.0 as usize, node)?;
            }
            if let Ok(local) = copy.destination {
                connect(&mut nodes, node, local.0 as usize)?;
            }
        }
        push(&mut components, ids)?;
    }
    let mut queue = Vec::new();
    for (i, node) in nodes.iter().enumerate() {
        if node.bad.is_some() {
            push(&mut queue, i)?;
        }
    }
    while let Some(index) = queue.pop() {
        let cause = nodes[index].bad.ok_or(StorageError::InternalInvariant(
            "storage dependency wake has no cause",
        ))?;
        for i in 0..nodes[index].users.len() {
            let user = nodes[index].users[i];
            if nodes[user].bad.is_none() {
                nodes[user].bad = Some(cause);
                push(&mut queue, user)?;
            }
        }
    }
    for (i, node) in nodes.iter().take(nlocals).enumerate() {
        f.local_gaps[i] = node
            .bad
            .map(|(plan, copy)| StorageGap::Dependency { plan, copy });
    }
    for (pi, indices) in components.iter().enumerate() {
        for (ci, &node) in indices.iter().enumerate() {
            let cause = nodes[node]
                .bad
                .map(|(plan, copy)| StorageGap::Dependency { plan, copy });
            let copy = &mut f.copies[pi].assignments[ci];
            if copy.issue.is_none() {
                copy.issue = cause;
            }
            if let Some(gap) = copy.issue {
                let site = StorageIssueSite::Copy {
                    plan: f.copies[pi].id,
                    copy: CopyId(id(ci)?),
                };
                issue(f, site, gap)?;
            }
        }
    }
    for i in 0..f.uses.len() {
        if let Ok(read) = f.uses[i].read
            && let Some(gap) = read.local().and_then(|local| f.local_gap(local))
        {
            f.uses[i].gap = Some(gap);
            issue(f, StorageIssueSite::Use(f.uses[i].site), gap)?;
        }
    }
    for construction in &mut f.constructions {
        if let Some(Ok(read)) = construction.receiver
            && let Some(gap) = read
                .local()
                .and_then(|local| f.local_gaps.get(local.0 as usize).copied().flatten())
        {
            construction.receiver = Some(Err(gap));
        }
    }
    Ok(())
}
fn identity(f: &StorageFacts, copy: &CopyAssignment) -> bool {
    let (Ok(destination), Ok(StorageRead::Value { local, view, .. })) =
        (copy.destination, copy.source)
    else {
        return false;
    };
    if destination != local {
        return false;
    }
    match (view, f.locals[destination.0 as usize].storage_type) {
        (Interpretation::Direct(a), StorageType::Family { family: b, .. }) => a == b,
        (
            Interpretation::CopyBits(a),
            StorageType::Family { width: b, .. } | StorageType::NumericBits { width: b, .. },
        ) => a == b,
        _ => false,
    }
}
fn schedule(f: &mut StorageFacts, pi: usize) -> Result<(), StorageError> {
    let count = f.copies[pi].assignments.len();
    let mut pending = filled(count, false)?;
    let mut sources = filled(count, None)?;
    let mut writers = HashMap::<LocalId, usize>::new();
    let mut readers = HashMap::<LocalId, Vec<usize>>::new();
    let mut remaining = HashMap::<LocalId, usize>::new();
    let mut left = 0;
    for i in 0..count {
        let copy = f.copies[pi].assignments[i];
        if copy.issue.is_some() || identity(f, &copy) {
            continue;
        }
        let (Ok(destination), Ok(source)) = (copy.destination, copy.source) else {
            return Err(StorageError::InternalInvariant(
                "ready copy has unavailable operands",
            ));
        };
        writers.try_reserve(1).map_err(RequestError::Allocation)?;
        if writers.insert(destination, i).is_some() {
            return Err(StorageError::InternalInvariant(
                "parallel copy has duplicate destinations",
            ));
        }
        pending[i] = true;
        sources[i] = Some(source);
        left += 1;
        if let Some(local) = source.local() {
            readers.try_reserve(1).map_err(RequestError::Allocation)?;
            push(readers.entry(local).or_default(), i)?;
            remaining.try_reserve(1).map_err(RequestError::Allocation)?;
            let value = remaining.entry(local).or_default();
            *value = value.checked_add(1).ok_or(RequestError::PlatformCapacity)?;
        }
    }
    let mut ready = Vec::new();
    for (i, &is_pending) in pending.iter().enumerate() {
        if is_pending {
            let destination = f.copies[pi].assignments[i]
                .destination
                .map_err(|_| StorageError::InternalInvariant("scheduled copy lost destination"))?;
            if remaining.get(&destination).copied().unwrap_or(0) == 0 {
                push(&mut ready, i)?;
            }
        }
    }
    let mut cursor = 0;
    while left > 0 {
        if let Some(i) = ready.pop() {
            if !pending[i] {
                continue;
            }
            let assignment = f.copies[pi].assignments[i];
            let destination = assignment
                .destination
                .map_err(|_| StorageError::InternalInvariant("scheduled copy lost destination"))?;
            let source = sources[i].ok_or(StorageError::InternalInvariant(
                "scheduled copy lost source",
            ))?;
            push(
                &mut f.copies[pi].steps,
                CopyStep::Assign {
                    copy: assignment.id,
                    destination,
                    source,
                },
            )?;
            pending[i] = false;
            left -= 1;
            if let Some(local) = source.local() {
                let n = remaining
                    .get_mut(&local)
                    .ok_or(StorageError::InternalInvariant(
                        "copy read dependency is missing",
                    ))?;
                *n = n.checked_sub(1).ok_or(StorageError::InternalInvariant(
                    "copy read dependency underflow",
                ))?;
                if *n == 0
                    && let Some(&writer) = writers.get(&local)
                    && pending[writer]
                {
                    push(&mut ready, writer)?;
                }
            }
        } else {
            while cursor < count && !pending[cursor] {
                cursor += 1;
            }
            if cursor == count {
                return Err(StorageError::InternalInvariant(
                    "parallel copy pending count disagrees with its arena",
                ));
            }
            let saved = f.copies[pi].assignments[cursor]
                .destination
                .map_err(|_| StorageError::InternalInvariant("cycle copy lost destination"))?;
            let old = remaining.get(&saved).copied().unwrap_or(0);
            if old == 0 {
                return Err(StorageError::InternalInvariant(
                    "ready parallel copy was not queued",
                ));
            }
            let storage_type = f.locals[saved.0 as usize].storage_type;
            let plan = f.copies[pi].id;
            let temporary = local(
                f,
                storage_type,
                LocalKind::ParallelTemporary { plan, saved },
            )?;
            push(
                &mut f.copies[pi].steps,
                CopyStep::Save {
                    destination: temporary,
                    source: saved,
                },
            )?;
            let uses = readers
                .remove(&saved)
                .ok_or(StorageError::InternalInvariant(
                    "copy cycle lost old-value readers",
                ))?;
            for &i in &uses {
                if pending[i] {
                    sources[i]
                        .as_mut()
                        .ok_or(StorageError::InternalInvariant("cycle reader lost source"))?
                        .redirect(temporary);
                }
            }
            // Move every pending reader, not merely one occurrence of the old
            // storage. Completed entries remain harmless historical indices.
            readers.try_reserve(1).map_err(RequestError::Allocation)?;
            readers.insert(temporary, uses);
            remaining.insert(saved, 0);
            remaining.try_reserve(1).map_err(RequestError::Allocation)?;
            remaining.insert(temporary, old);
            push(&mut ready, cursor)?;
        }
    }
    Ok(())
}
pub(super) fn construct(f: &mut StorageFacts) -> Result<(), StorageError> {
    dependencies(f)?;
    for pi in 0..f.copies.len() {
        schedule(f, pi)?;
    }
    Ok(())
}
