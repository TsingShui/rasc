//! A Kotlin `when` alternative for an already selected chain of integer Ifs.
//! Only the presentation changes. Arms, default, abrupt exits and edge copies
//! retain their original owners; no state variable or dispatcher is introduced.

use super::build::{filled, push};
use super::graph::Graph;
use super::*;
use crate::p2::{BranchOp, Terminal};
use crate::p4::{Family, Interpretation};
use crate::p5::{StorageMethod, StorageRead};

pub(super) fn construct(
    storage: &StorageMethod<'_>,
    facts: &mut RegionFacts,
    graph: &Graph,
) -> Result<(), RegionError> {
    let mut consumed = filled(facts.regions.len(), false)?;
    let mut work = Vec::new();
    if let Some(root) = facts.root {
        push(&mut work, root)?;
    }
    // Enclosing alternatives claim their nested tests first. The retained
    // source tree still supplies the ancestry of every body and abrupt exit.
    while let Some(region) = work.pop() {
        if !consumed[region.0 as usize]
            && let Some(plan) = alternative(storage, facts, graph, region)?
        {
            for test in &plan.tests {
                consumed[test.region.0 as usize] = true;
            }
            if cfg!(feature = "trace-passes") {
                let key = storage.execution().owner_facts().key();
                eprintln!(
                    "[decision] dex={} class={} method={} region={} layout=kotlin_when arms={}",
                    key.dex.0,
                    key.class_def.0,
                    storage.execution().method_id().0,
                    region.0,
                    plan.tests.len()
                );
            }
            facts.regions[region.0 as usize].decision = Some(plan);
        }
        for &child in facts.regions[region.0 as usize].children.iter().rev() {
            push(&mut work, child)?;
        }
    }
    Ok(())
}

fn alternative(
    storage: &StorageMethod<'_>,
    f: &RegionFacts,
    graph: &Graph,
    root: RegionId,
) -> Result<Option<DecisionPlan>, RegionError> {
    let follow = f.regions[root.0 as usize].follow;
    let mut current = root;
    let mut selector = None;
    let mut tests = Vec::new();
    for _ in 0..f.regions.len() {
        let region = &f.regions[current.0 as usize];
        let RegionKind::If {
            true_link,
            false_link,
        } = region.kind
        else {
            return Ok(None);
        };
        let [header, yes, no] = region.children.as_slice() else {
            return Ok(None);
        };
        let RegionKind::Block(predicate) = f.regions[header.0 as usize].kind else {
            return Ok(None);
        };
        let node = &f.nodes[predicate.0 as usize];
        if let Some(previous) = tests.last() {
            let previous: &DecisionTest = previous;
            if graph.incoming[predicate.0 as usize].as_slice() != [previous.remaining]
                || graph.exceptions.iter().flatten().any(|&edge| {
                    Graph::exception_possible(f, edge)
                        && Graph::exception_target(f, storage, edge) == Some(predicate)
                })
            {
                return Ok(None);
            }
        }
        // Replacing later comparisons cannot move any original effect, copy or
        // throwable operand out of its original evaluation position.
        if !node.operations.is_empty() || !node.terminal || region.follow != follow {
            return Ok(None);
        }
        if current != root
            && !(matches!(region.continuation, Continuation::Parent)
                || matches!(region.continuation, Continuation::Next(target) if Some(target) == follow))
        {
            return Ok(None);
        }
        if f.throws.iter().any(|site| site.site == crate::p5::ThrowSite::Terminal(node.block))
            || f.copies.iter().any(|copy| matches!(copy.location,
                crate::p5::CopyLocation::BeforeThrow(crate::p5::ThrowSite::Terminal(block))
                | crate::p5::CopyLocation::Transfer { site: crate::p5::TransferSite::BeforeTerminal(block), .. }
                if block == node.block))
        {
            return Ok(None);
        }
        let Some((read, argument, key, equal)) = comparison(storage, node.block) else {
            return Ok(None);
        };
        if selector.is_some_and(|old| old != read)
            || tests.iter().any(|test: &DecisionTest| test.key == key)
        {
            return Ok(None);
        }
        selector = Some(read);
        let (selected, remaining, body, next) = if equal {
            (true_link, false_link, *yes, *no)
        } else {
            (false_link, true_link, *no, *yes)
        };
        push(
            &mut tests,
            DecisionTest {
                region: current,
                header: *header,
                predicate,
                argument,
                key,
                selected,
                remaining,
                body,
            },
        )?;
        if matches!(f.regions[next.0 as usize].kind, RegionKind::If { .. })
            && f.regions[next.0 as usize].follow == follow
        {
            // The failed comparisons are pure routing only. A copied value on
            // an intermediate failed edge must remain at that edge, so refuse.
            if !f.link_plans[remaining.0 as usize].copies.is_empty() {
                return Ok(None);
            }
            current = next;
            continue;
        }
        if tests.len() < 2 {
            return Ok(None);
        }
        return Ok(Some(DecisionPlan {
            tests,
            default: next,
        }));
    }
    Ok(None)
}

type Selector = (crate::p5::LocalId, crate::p3::ValueId, crate::p3::StateId);
type Comparison = (Selector, u32, i32, bool);

/// Storage identity includes the original value and state, not a matching name.
fn comparison(storage: &StorageMethod<'_>, block: crate::p2::BlockId) -> Option<Comparison> {
    let flow = storage.execution();
    let Terminal::Branch { op, .. } = flow.blocks()[block.0 as usize].terminal()? else {
        return None;
    };
    let uses = storage.facts().terminal_uses(block)?;
    let (argument, key, equal) = match (*op, uses) {
        (BranchOp::Eqz | BranchOp::Nez, [_]) => (0, 0, *op == BranchOp::Eqz),
        (BranchOp::Eq | BranchOp::Ne, [left, right]) => {
            let (argument, bits) = match (left.read().ok()?, right.read().ok()?) {
                (
                    StorageRead::Value { .. },
                    StorageRead::Literal {
                        bits,
                        view:
                            Interpretation::Literal {
                                family: Family::Integral,
                                ..
                            },
                        ..
                    },
                ) => (0, bits),
                (
                    StorageRead::Literal {
                        bits,
                        view:
                            Interpretation::Literal {
                                family: Family::Integral,
                                ..
                            },
                        ..
                    },
                    StorageRead::Value { .. },
                ) => (1, bits),
                _ => return None,
            };
            if bits.width != crate::SlotWidth::One {
                return None;
            }
            let value = u32::try_from(bits.value).ok()?;
            (
                argument,
                i32::from_ne_bytes(value.to_ne_bytes()),
                *op == BranchOp::Eq,
            )
        }
        _ => return None,
    };
    let StorageRead::Value {
        local,
        value,
        state,
        view: Interpretation::Direct(Family::Integral),
    } = uses.get(argument as usize)?.read().ok()?
    else {
        return None;
    };
    Some(((local, value, state), argument, key, equal))
}
