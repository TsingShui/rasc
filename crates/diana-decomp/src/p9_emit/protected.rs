use crate::p7_regions::protection::{Catch, Node, NodeId};
use crate::p8_java::dispatcher::{Case, Transfer};
use crate::p8_java::protected::ProtectedBody;
use crate::substrate::owned::{TextBuffer, push};
use crate::{MethodBudget, RequestError};

enum Work<'a> {
    Node(NodeId),
    Text(&'static str),
    Edge(&'a crate::p8_java::dispatcher::Edge),
    Leave(&'a crate::p8_java::dispatcher::Edge),
    Catch(&'a Catch),
}
fn block(body: &ProtectedBody, id: crate::p2_flow::normal::BlockId) -> Result<&Case, RequestError> {
    body.storage
        .cases
        .get(usize::try_from(id.0).map_err(|_| RequestError::PlatformCapacity)?)
        .ok_or(RequestError::InternalInvariant(
            "protected source lost block",
        ))
}
pub(super) fn write(
    body: &ProtectedBody,
    names: &crate::p8_java::types::TypeNames,
    output: &mut TextBuffer,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<(), RequestError> {
    super::dispatcher::locals(&body.storage, names, output, budget)?;
    let mut todo = Vec::new();
    push(&mut todo, Work::Node(body.region.root), budget)?;
    while let Some(work) = todo.pop() {
        budget.charge_work(1)?;
        match work {
            Work::Text(text) => output.push(text, budget)?,
            Work::Edge(edge) => super::java::assignments(&edge.copies, output, budget)?,
            Work::Leave(edge) => {
                super::java::assignments(&edge.copies, output, budget)?;
                output.push("break;\n", budget)?;
            }
            Work::Catch(catch) => {
                output.push(" catch (", budget)?;
                for (index, clause) in catch.clauses.iter().enumerate() {
                    budget.charge_work(1)?;
                    if index != 0 {
                        output.push(" | ", budget)?;
                    }
                    let mut name = None;
                    for candidate in &body.names {
                        budget.charge_work(1)?;
                        if candidate.kind == clause.kind {
                            name = Some(candidate);
                            break;
                        }
                    }
                    let name =
                        name.ok_or(RequestError::InternalInvariant("catch lost source type"))?;
                    output.push(&name.source, budget)?;
                }
                let first = catch
                    .clauses
                    .first()
                    .ok_or(RequestError::InternalInvariant("empty catch alternatives"))?;
                output.push(" caught", budget)?;
                output.unsigned(u64::from(first.handler.0), budget)?;
                output.push(") {\n", budget)?;
            }
            Work::Node(id) => match &body.region.nodes
                [usize::try_from(id.0).map_err(|_| RequestError::PlatformCapacity)?]
            {
                Node::Synchronized {
                    enter, body: inner, ..
                } => {
                    let case = block(body, *enter)?;
                    super::dispatcher::statements(case, output, budget)?;
                    let mut lock = None;
                    for statement in &case.statements {
                        budget.charge_work(1)?;
                        if let crate::p8_java::calls::Statement::Monitor {
                            kind: crate::p2_flow::normal::MonitorKind::Enter,
                            lock: value,
                            ..
                        } = statement
                        {
                            lock = Some(*value);
                        }
                    }
                    let lock = lock.ok_or(RequestError::InternalInvariant(
                        "synchronized AST lost lock expression",
                    ))?;
                    output.push("synchronized (", budget)?;
                    super::java::value_use(lock, output, budget)?;
                    output.push(") {\n", budget)?;
                    let Transfer::Jump(edge) = &case.transfer else {
                        return Err(RequestError::InternalInvariant(
                            "synchronized AST lost ingress",
                        ));
                    };
                    super::java::assignments(&edge.copies, output, budget)?;
                    push(&mut todo, Work::Text("}\n"), budget)?;
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
                    output.push("while (true) {\n", budget)?;
                    super::dispatcher::statements(case, output, budget)?;
                    super::dispatcher::open_if(&case.transfer, output, budget)?;
                    push(&mut todo, Work::Text("}\n}\n"), budget)?;
                    if *body_yes {
                        push(&mut todo, Work::Leave(no), budget)?;
                        push(&mut todo, Work::Text("} else {\n"), budget)?;
                        push(&mut todo, Work::Node(*inner), budget)?;
                        push(&mut todo, Work::Edge(yes), budget)?;
                    } else {
                        push(&mut todo, Work::Node(*inner), budget)?;
                        push(&mut todo, Work::Edge(no), budget)?;
                        push(&mut todo, Work::Text("} else {\n"), budget)?;
                        push(&mut todo, Work::Leave(yes), budget)?;
                    }
                }
                Node::Sequence(children) => {
                    for &child in children.iter().rev() {
                        budget.charge_work(1)?;
                        push(&mut todo, Work::Node(child), budget)?;
                    }
                }
                Node::Block(id) => {
                    let case = block(body, *id)?;
                    super::dispatcher::statements(case, output, budget)?;
                    match &case.transfer {
                        Transfer::Jump(edge) => {
                            super::java::assignments(&edge.copies, output, budget)?
                        }
                        Transfer::Return(value) => super::java::returned(*value, output, budget)?,
                        Transfer::ReturnVoid => output.push("return;\n", budget)?,
                        Transfer::Throw(_) | Transfer::Rethrow { .. } => {
                            super::java::thrown(&case.transfer, output, budget)?
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
                    super::dispatcher::statements(case, output, budget)?;
                    super::dispatcher::open_if(&case.transfer, output, budget)?;
                    push(&mut todo, Work::Text("}\n"), budget)?;
                    push(&mut todo, Work::Node(*no), budget)?;
                    push(&mut todo, Work::Edge(no_edge), budget)?;
                    push(&mut todo, Work::Text("} else {\n"), budget)?;
                    push(&mut todo, Work::Node(*yes), budget)?;
                    push(&mut todo, Work::Edge(yes_edge), budget)?;
                }
                Node::Try {
                    body: inner,
                    catches,
                } => {
                    output.push("try {\n", budget)?;
                    for catch in catches.iter().rev() {
                        budget.charge_work(1)?;
                        push(&mut todo, Work::Text("}\n"), budget)?;
                        push(&mut todo, Work::Node(catch.body), budget)?;
                        push(&mut todo, Work::Catch(catch), budget)?;
                    }
                    push(&mut todo, Work::Text("}"), budget)?;
                    push(&mut todo, Work::Node(*inner), budget)?;
                }
            },
        }
    }
    Ok(())
}
