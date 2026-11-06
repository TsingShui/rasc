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
                        if p.follow == Some(l.to) && !p.members.contains(&l.to) {
                            action = TransferAction::Break(id);
                            break;
                        }
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
                    RegionKind::Sequence => {
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
                    RegionKind::If { .. } | RegionKind::Try(_) if r.follow == Some(l.to) => {
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
        f.regions[i].continuation = continuation;
    }
    Ok(())
}
