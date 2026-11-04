use super::source_control::{Builder, Label, Plan, Step};
use crate::p6_regions::protection::Protection;
use crate::p6_regions::protection::{Catch, Node, NodeId};
use crate::p7_java::dispatcher::{Case, Transfer};
use crate::p7_java::protected::CatchName;
struct Input<'a> {
    storage: &'a crate::p7_java::dispatcher::Dispatcher,
    region: &'a Protection,
    names: &'a [CatchName],
}
use crate::substrate::owned::{TextBuffer, push};
use crate::{RequestError, RunContext};

enum Work<'a> {
    Node(NodeId),
    Step(Step),
    Edge(&'a crate::p7_java::dispatcher::Edge),
    Leave(&'a crate::p7_java::dispatcher::Edge),
    Catch(&'a Catch),
}
fn block<'a>(body: &Input<'a>, id: crate::p2_flow::BlockId) -> Result<&'a Case, RequestError> {
    body.storage
        .cases
        .get(usize::try_from(id.0).map_err(|_| RequestError::PlatformCapacity)?)
        .ok_or(RequestError::InternalInvariant(
            "protected source lost block",
        ))
}
fn lower_into(
    body: &Input<'_>,
    output: &mut Builder,
    budget: &mut RunContext<'_>,
) -> Result<(), RequestError> {
    let mut todo = Vec::new();
    push(&mut todo, Work::Node(body.region.root), budget)?;
    while let Some(work) = todo.pop() {
        budget.checkpoint()?;
        match work {
            Work::Step(step) => output.marker(step, budget)?,
            Work::Edge(edge) => output.copies(edge, budget)?,
            Work::Leave(edge) => {
                output.copies(edge, budget)?;
                output.marker(Step::Break(Label::Nearest), budget)?;
            }
            Work::Catch(catch) => {
                let mut alternatives = Vec::new();
                for clause in &catch.clauses {
                    budget.checkpoint()?;
                    let mut name = None;
                    for candidate in body.names {
                        budget.checkpoint()?;
                        if candidate.kind == clause.kind {
                            name = Some(candidate);
                            break;
                        }
                    }
                    let name =
                        name.ok_or(RequestError::InternalInvariant("catch lost source type"))?;
                    let mut source = TextBuffer::new();
                    source.push(&name.source, budget)?;
                    push(&mut alternatives, source.finish(), budget)?;
                }
                let first = catch
                    .clauses
                    .first()
                    .ok_or(RequestError::InternalInvariant("empty catch alternatives"))?;
                output.marker(
                    Step::Catch {
                        alternatives,
                        handler: first.handler,
                    },
                    budget,
                )?;
            }
            Work::Node(id) => match &body.region.nodes
                [usize::try_from(id.0).map_err(|_| RequestError::PlatformCapacity)?]
            {
                Node::Synchronized {
                    scope,
                    enter,
                    body: inner,
                } => {
                    let case = block(body, *enter)?;
                    output.statements(case, budget)?;
                    let mut lock = None;
                    for statement in &case.statements {
                        budget.checkpoint()?;
                        if let crate::p7_java::calls::Statement::Monitor {
                            kind: crate::p2_flow::MonitorKind::Enter,
                            lock: value,
                            operation,
                        } = statement
                        {
                            let expected = body
                                .region
                                .monitors
                                .as_ref()
                                .ok_or(RequestError::InternalInvariant(
                                    "synchronized source lost scope",
                                ))?
                                .scope(*scope)?;
                            if *operation != expected.enter || lock.is_some() {
                                return Err(RequestError::InternalInvariant(
                                    "synchronized source changed acquisition",
                                ));
                            }
                            lock = Some(*value);
                        }
                    }
                    let lock = lock.ok_or(RequestError::InternalInvariant(
                        "synchronized AST lost lock expression",
                    ))?;
                    output.marker(Step::Synchronized(lock), budget)?;
                    let Transfer::Jump(edge) = &case.transfer else {
                        return Err(RequestError::InternalInvariant(
                            "synchronized AST lost ingress",
                        ));
                    };
                    output.copies(edge, budget)?;
                    push(&mut todo, Work::Step(Step::End), budget)?;
                    push(&mut todo, Work::Node(*inner), budget)?;
                }
                Node::Loop {
                    header,
                    body: inner,
                    body_yes,
                    ..
                } => {
                    let case = block(body, *header)?;
                    let Transfer::Branch { yes, no, .. } = &case.transfer else {
                        return Err(RequestError::InternalInvariant(
                            "protected loop lost condition",
                        ));
                    };
                    output.marker(Step::Loop(Label::Nearest), budget)?;
                    output.statements(case, budget)?;
                    output.condition(case, budget)?;
                    push(&mut todo, Work::Step(Step::End), budget)?;
                    push(&mut todo, Work::Step(Step::End), budget)?;
                    if *body_yes {
                        push(&mut todo, Work::Leave(no), budget)?;
                        push(&mut todo, Work::Step(Step::Else), budget)?;
                        push(&mut todo, Work::Node(*inner), budget)?;
                        push(&mut todo, Work::Edge(yes), budget)?;
                    } else {
                        push(&mut todo, Work::Node(*inner), budget)?;
                        push(&mut todo, Work::Edge(no), budget)?;
                        push(&mut todo, Work::Step(Step::Else), budget)?;
                        push(&mut todo, Work::Leave(yes), budget)?;
                    }
                }
                Node::Sequence(children) => {
                    for &child in children.iter().rev() {
                        budget.checkpoint()?;
                        push(&mut todo, Work::Node(child), budget)?;
                    }
                }
                Node::Block(id) => {
                    let case = block(body, *id)?;
                    output.statements(case, budget)?;
                    match &case.transfer {
                        Transfer::Jump(edge) => output.copies(edge, budget)?,
                        Transfer::Return(value) => output.returned(*value, budget)?,
                        Transfer::ReturnVoid => output.marker(Step::ReturnVoid, budget)?,
                        Transfer::InitializerExit => {
                            output.marker(Step::InitializerExit, budget)?
                        }
                        Transfer::Throw(_) | Transfer::Rethrow { .. } => {
                            output.thrown(&case.transfer, budget)?
                        }
                        _ => {
                            return Err(RequestError::InternalInvariant(
                                "protected leaf changed transfer",
                            ));
                        }
                    }
                }
                Node::If { block: id, yes, no } => {
                    let case = block(body, *id)?;
                    let Transfer::Branch {
                        yes: yes_edge,
                        no: no_edge,
                        ..
                    } = &case.transfer
                    else {
                        return Err(RequestError::InternalInvariant(
                            "protected if changed transfer",
                        ));
                    };
                    output.statements(case, budget)?;
                    output.condition(case, budget)?;
                    push(&mut todo, Work::Step(Step::End), budget)?;
                    push(&mut todo, Work::Node(*no), budget)?;
                    push(&mut todo, Work::Edge(no_edge), budget)?;
                    push(&mut todo, Work::Step(Step::Else), budget)?;
                    push(&mut todo, Work::Node(*yes), budget)?;
                    push(&mut todo, Work::Edge(yes_edge), budget)?;
                }
                Node::Try {
                    body: inner,
                    catches,
                } => {
                    output.marker(Step::Try(id), budget)?;
                    push(&mut todo, Work::Step(Step::EndTry), budget)?;
                    for catch in catches.iter().rev() {
                        budget.checkpoint()?;
                        push(&mut todo, Work::Step(Step::End), budget)?;
                        push(&mut todo, Work::Node(catch.body), budget)?;
                        push(&mut todo, Work::Catch(catch), budget)?;
                    }
                    push(&mut todo, Work::Step(Step::CloseTryBody), budget)?;
                    push(&mut todo, Work::Node(*inner), budget)?;
                }
            },
        }
    }
    Ok(())
}

pub(super) fn lower(
    storage: &crate::p7_java::dispatcher::Dispatcher,
    region: &Protection,
    names: &[CatchName],
    budget: &mut RunContext<'_>,
) -> Result<Plan, RequestError> {
    let input = Input {
        storage,
        region,
        names,
    };
    let mut plan = Builder::new();
    lower_into(&input, &mut plan, budget)?;
    plan.finish(storage, Some(region), names, budget)
}
