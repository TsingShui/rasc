//! Plans what each original link prints as, then what each region's own
//! continuation is. A transfer keeps its original destination; the plan only
//! names the selected construct that already carries that destination, so no
//! transfer is invented, retargeted or dropped.

use super::graph::Graph;
use super::*;

pub(super) fn transfers(f: &mut RegionFacts, g: &Graph) -> Result<(), RegionError> {
    for index in 0..f.links.len() {
        let l = f.links[index];
        let mut action = TransferAction::Goto(LabelId(l.to.0));
        let mut at = Some(f.labels[l.from.0 as usize].owner);
        let mut handled = false;
        while let Some(id) = at {
            let r = &f.regions[id.0 as usize];
            match &r.kind {
                RegionKind::If {
                    true_link,
                    false_link,
                } if r.entry == Some(l.from) => {
                    if l.id == *true_link || l.id == *false_link {
                        action = TransferAction::IfArm {
                            region: id,
                            taken: l.id == *true_link,
                        };
                        handled = true;
                        break;
                    }
                }
                RegionKind::Switch { arms, .. } if arms.contains(&l.id) => {
                    action = TransferAction::SwitchArm { region: id };
                    handled = true;
                    break;
                }
                RegionKind::Loop(loop_id) => {
                    let p = &f.loops[loop_id.0 as usize];
                    if p.evaluation.windows(2).any(|pair| pair == [l.from, l.to]) {
                        action = TransferAction::Next(l.to);
                        handled = true;
                        break;
                    }
                    if p.condition() == Some(l.from) {
                        action = TransferAction::LoopCondition {
                            region: id,
                            repeat: p.members.contains(&l.to),
                        };
                        handled = true;
                        break;
                    }
                }
                RegionKind::Synchronized(scope) => {
                    let p = &f.synchronized[scope.0 as usize];
                    if p.enter_link == l.id {
                        action = TransferAction::EnterSynchronized(id);
                        handled = true;
                        break;
                    }
                    if let Some(exit) = p.exits.iter().find(|exit| exit.link == l.id) {
                        action = TransferAction::SynchronizedReleased {
                            region: id,
                            release: exit.release,
                        };
                        handled = true;
                        break;
                    }
                }
                _ => {}
            }
            at = f.parents[id.0 as usize];
        }
        if !handled {
            let mut crossed_loop = false;
            let mut at = Some(f.labels[l.from.0 as usize].owner);
            while let Some(id) = at {
                let r = &f.regions[id.0 as usize];
                match &r.kind {
                    RegionKind::Loop(loop_id) => {
                        let p = &f.loops[loop_id.0 as usize];
                        if l.to == p.continue_target() {
                            action = TransferAction::Continue(*loop_id);
                            break;
                        }
                        if p.follow == Some(l.to) && !r.nodes.contains(&l.to) {
                            action = TransferAction::Break(id);
                            break;
                        }
                        // Reaching an outer sibling is not fallthrough through
                        // this loop's closing brace: repetition would run first.
                        crossed_loop |= !r.nodes.contains(&l.to);
                    }
                    RegionKind::Synchronized(sync) => {
                        let p = &f.synchronized[sync.0 as usize];
                        if p.body.contains(&l.from)
                            && let Some(exit) = p
                                .exits
                                .iter()
                                .find(|exit| g.op_node[exit.release.0 as usize] == l.to)
                        {
                            action = TransferAction::LeaveSynchronized {
                                region: id,
                                release: exit.release,
                            };
                            break;
                        }
                    }
                    RegionKind::Switch { .. } | RegionKind::LabelledBlock { .. }
                        if r.follow == Some(l.to) && !r.nodes.contains(&l.to) =>
                    {
                        action = TransferAction::Break(id);
                        break;
                    }
                    RegionKind::Sequence if !crossed_loop => {
                        for pair in r.children.windows(2) {
                            let a = &f.regions[pair[0].0 as usize];
                            let b = &f.regions[pair[1].0 as usize];
                            if a.nodes.contains(&l.from) && b.entry == Some(l.to) {
                                action = TransferAction::Next(l.to);
                                handled = true;
                                break;
                            }
                        }
                        if handled {
                            break;
                        }
                    }
                    RegionKind::If { .. } | RegionKind::Condition(_) | RegionKind::Try(_)
                        if !crossed_loop && r.follow == Some(l.to) =>
                    {
                        action = TransferAction::Next(l.to);
                        break;
                    }
                    _ => {}
                }
                at = f.parents[id.0 as usize];
            }
        }
        f.link_plans[index].action = action;
    }
    for i in 0..f.regions.len() {
        let r = &f.regions[i];
        let mut continuation = r
            .follow
            .map_or(Continuation::None, |n| Continuation::Goto(LabelId(n.0)));
        if let (Some(parent), Some(follow)) = (f.parents[i], r.follow) {
            let p = &f.regions[parent.0 as usize];
            match &p.kind {
                RegionKind::Sequence => {
                    if let Some(position) = p.children.iter().position(|r| r.0 as usize == i) {
                        if let Some(next) = p.children.get(position + 1) {
                            if f.regions[next.0 as usize].entry == Some(follow) {
                                continuation = Continuation::Next(follow);
                            }
                        } else if p.follow == Some(follow) {
                            continuation = Continuation::Parent;
                        }
                    }
                }
                RegionKind::If { .. }
                | RegionKind::Condition(_)
                | RegionKind::Switch { .. }
                | RegionKind::Try(_)
                | RegionKind::LabelledBlock { .. }
                    if p.follow == Some(follow) =>
                {
                    continuation = Continuation::Parent
                }
                RegionKind::Loop(loop_id)
                    if f.loops[loop_id.0 as usize].continue_target() == follow =>
                {
                    continuation = Continuation::Parent
                }
                RegionKind::Synchronized(sync)
                    if f.synchronized[sync.0 as usize].follow == Some(follow)
                        || f.synchronized[sync.0 as usize]
                            .exits
                            .iter()
                            .any(|exit| g.op_node[exit.release.0 as usize] == follow) =>
                {
                    continuation = Continuation::Parent
                }
                _ => {}
            }
        }
        if let Some(follow) = r.follow {
            let mut normal = r.nodes.is_empty();
            for link in &f.links {
                if !r.nodes.contains(&link.from) || link.to != follow {
                    continue;
                }
                let consumed = match f.link_plans[link.id.0 as usize].action {
                    TransferAction::Break(scope) => !f.regions[scope.0 as usize]
                        .nodes
                        .iter()
                        .all(|n| r.nodes.contains(n)),
                    TransferAction::Continue(loop_id) => !f.loops[loop_id.0 as usize]
                        .lexical_members
                        .iter()
                        .all(|n| r.nodes.contains(n)),
                    _ => false,
                };
                normal |= !consumed;
            }
            if !normal {
                continuation = Continuation::None;
            } else {
                let mut at = f.parents[i];
                while let Some(scope) = at {
                    let parent = &f.regions[scope.0 as usize];
                    match parent.kind {
                        RegionKind::Switch {
                            private_cases: true,
                            ..
                        } if parent.follow == Some(follow) => {
                            continuation = Continuation::Break(scope);
                            break;
                        }
                        RegionKind::Loop(loop_id)
                            if matches!(continuation, Continuation::Goto(_)) =>
                        {
                            let plan = &f.loops[loop_id.0 as usize];
                            if plan.follow == Some(follow) {
                                continuation = Continuation::Break(scope);
                                break;
                            }
                            if plan.continue_target() == follow {
                                continuation = Continuation::Continue(loop_id);
                                break;
                            }
                        }
                        _ => {}
                    }
                    // An implicit child completion belongs to its source parent.
                    // Only the case body's boundary, not every descendant, owns
                    // the switch completion after that parent's normal work.
                    if matches!(continuation, Continuation::Parent | Continuation::Next(_)) {
                        break;
                    }
                    at = f.parents[scope.0 as usize];
                }
            }
        }
        f.regions[i].continuation = continuation;
    }
    Ok(())
}
