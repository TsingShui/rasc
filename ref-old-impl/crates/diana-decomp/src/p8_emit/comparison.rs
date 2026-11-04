use crate::p7_java::conditions::Comparison;
use crate::p7_java::numeric_expression::{ExprId, Expression, Node};
use crate::substrate::owned::{TextBuffer, push};
use crate::{RequestError, RunContext};
enum Work {
    Node(ExprId),
    Text(&'static str),
}
pub(super) fn write(
    tree: &Expression,
    output: &mut TextBuffer,
    budget: &mut RunContext<'_>,
) -> Result<(), RequestError> {
    let mut todo = Vec::new();
    push(&mut todo, Work::Node(tree.root()), budget)?;
    while let Some(work) = todo.pop() {
        budget.checkpoint()?;
        match work {
            Work::Text(text) => output.push(text, budget)?,
            Work::Node(id) => match tree.node(id)? {
                Node::Value(value) => super::java::value_use(value, output, budget)?,
                Node::Literal(value) => output.arguments(format_args!("{value}"), budget)?,
                Node::Test(operator, left, right) => {
                    push(&mut todo, Work::Node(right), budget)?;
                    push(
                        &mut todo,
                        Work::Text(match operator {
                            Comparison::Equal => " == ",
                            Comparison::NotEqual => " != ",
                            Comparison::Less => " < ",
                            Comparison::Greater => " > ",
                            Comparison::LessEqual => " <= ",
                            Comparison::GreaterEqual => " >= ",
                        }),
                        budget,
                    )?;
                    push(&mut todo, Work::Node(left), budget)?;
                }
                Node::Conditional(condition, yes, no) => {
                    push(&mut todo, Work::Node(no), budget)?;
                    push(&mut todo, Work::Text(" : "), budget)?;
                    push(&mut todo, Work::Node(yes), budget)?;
                    push(&mut todo, Work::Text(" ? "), budget)?;
                    push(&mut todo, Work::Node(condition), budget)?;
                }
            },
        }
    }
    Ok(())
}
