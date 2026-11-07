//! Constructs explicit condition alternatives from selected child interfaces.
//! Child interiors and abrupt-exit scopes are retained; only delivery across
//! the changed parent layout needs a new route or completion.

use super::build::*;
use super::graph::Graph;
use super::*;

pub(super) fn construct(f: &mut RegionFacts, g: &Graph) -> Result<(), RegionError> {
    for region in 0..f.regions.len() {
        let RegionKind::Condition(id) = f.regions[region].kind else {
            continue;
        };
        let plan = &f.conditions[id.0 as usize];
        let source = &f.regions[region];
        let runs = plan.tests.len();
        if source.children.len() != runs + 1 + usize::from(plan.else_arm) {
            return Err(bug("condition lost its source child roles"));
        }
        let mut children = Vec::new();
        for &child in &source.children[..runs] {
            push(&mut children, child)?;
        }
        if plan.else_arm {
            push(&mut children, source.children[runs + 1])?;
        }
        push(&mut children, source.children[runs])?;

        let mut membership = filled(f.nodes.len(), None)?;
        let mut completions = Vec::new();
        for (position, &child) in children.iter().enumerate() {
            let r = &f.regions[child.0 as usize];
            for &node in &r.nodes {
                membership[node.0 as usize] = Some(child);
            }
            let next = children
                .get(position + 1)
                .and_then(|next| f.regions[next.0 as usize].entry);
            let continuation = match r.continuation {
                Continuation::Parent => {
                    // Parent denotes the source condition's follow, not the
                    // next printed arm. A child may itself end in a structured
                    // break or an empty arm with no explicit outgoing link.
                    let follow = r.follow.ok_or(bug("parent completion lost its follow"))?;
                    if source.follow != Some(follow) {
                        return Err(bug("condition child lost its parent completion"));
                    }
                    if position + 1 == children.len() {
                        Continuation::Parent
                    } else {
                        Continuation::Goto(LabelId(follow.0))
                    }
                }
                Continuation::Next(target) if next != Some(target) => {
                    Continuation::Goto(LabelId(target.0))
                }
                continuation => continuation,
            };
            push(
                &mut completions,
                ConditionCompletion {
                    child,
                    continuation,
                },
            )?;
        }
        let mut gotos = Vec::new();
        for test in &plan.tests {
            push(&mut gotos, test.select)?;
            push(&mut gotos, test.other)?;
        }
        for completion in &completions {
            let child = completion.child;
            let r = &f.regions[child.0 as usize];
            let delivery = match completion.continuation {
                Continuation::None => None,
                Continuation::Parent => source.follow,
                Continuation::Next(node) => Some(node),
                Continuation::Goto(_) => None,
            };
            for &node in &r.nodes {
                for &link in &g.outgoing[node.0 as usize] {
                    let original = &f.links[link.0 as usize];
                    if matches!(
                        f.link_plans[link.0 as usize].action,
                        TransferAction::Next(_)
                    ) && membership[original.to.0 as usize] != Some(child)
                        && !(r.follow == Some(original.to) && delivery == Some(original.to))
                        && !gotos.contains(&link)
                    {
                        push(&mut gotos, link)?;
                    }
                }
            }
        }
        // No child changes lexical ancestry or internal placement. Break,
        // Continue, selected arms, exception/lock outlets and their copies keep
        // their source scopes. A scope completing normally still traverses its
        // child's completion above, including after a break to that scope's end.
        // Only implicit child completion can carry an original Next exit.
        // With an explicit child continuation, keep that link's delivery
        // explicit too; the ordinary display pass may remove the repeated jump.
        // Internal Next links retain their established routing.
        f.conditions[id.0 as usize].explicit = Some(ConditionSchedule {
            children,
            completions,
            gotos,
        });
    }
    Ok(())
}
