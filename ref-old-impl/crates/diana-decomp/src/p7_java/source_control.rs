use super::branch::ValueUse;
use super::dispatcher::{Case, Dispatcher, Edge, Transfer};
use crate::p2_flow::{BlockId, EdgeId};
use crate::substrate::owned::push;
use crate::{RequestError, RunContext};

#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum Label {
    Loop0,
    Nearest,
}
#[cfg_attr(test, derive(Clone))]
pub(crate) enum Step {
    Statements(BlockId),
    Copies {
        source: BlockId,
        edge: EdgeId,
    },
    If(BlockId),
    Else,
    End,
    Loop(Label),
    Break(Label),
    Continue(Label),
    Return(ValueUse),
    ReturnVoid,
    InitializerExit,
    Throw(ValueUse),
    Rethrow(BlockId),
    Synchronized(ValueUse),
    Try(crate::p6_regions::protection::NodeId),
    CloseTryBody,
    EndTry,
    Catch {
        alternatives: Vec<String>,
        handler: BlockId,
    },
}
pub(crate) struct Plan {
    steps: Vec<Step>,
}
impl Plan {
    pub(crate) fn steps(&self) -> &[Step] {
        &self.steps
    }
}
pub(super) struct Builder {
    steps: Vec<Step>,
}
impl Builder {
    pub(super) fn new() -> Self {
        Self { steps: Vec::new() }
    }
    pub(super) fn marker(
        &mut self,
        step: Step,
        budget: &mut RunContext<'_>,
    ) -> Result<(), RequestError> {
        push(&mut self.steps, step, budget)
    }
    pub(super) fn statements(
        &mut self,
        case: &Case,
        budget: &mut RunContext<'_>,
    ) -> Result<(), RequestError> {
        self.marker(Step::Statements(case.key), budget)
    }
    pub(super) fn copies(
        &mut self,
        edge: &Edge,
        budget: &mut RunContext<'_>,
    ) -> Result<(), RequestError> {
        self.marker(
            Step::Copies {
                source: edge.source,
                edge: edge.id,
            },
            budget,
        )
    }
    pub(super) fn condition(
        &mut self,
        case: &Case,
        budget: &mut RunContext<'_>,
    ) -> Result<(), RequestError> {
        self.marker(Step::If(case.key), budget)
    }
    pub(super) fn returned(
        &mut self,
        value: ValueUse,
        budget: &mut RunContext<'_>,
    ) -> Result<(), RequestError> {
        self.marker(Step::Return(value), budget)
    }
    pub(super) fn thrown(
        &mut self,
        transfer: &Transfer,
        budget: &mut RunContext<'_>,
    ) -> Result<(), RequestError> {
        let step = match transfer {
            Transfer::Throw(value) => Step::Throw(*value),
            Transfer::Rethrow { handler, .. } => Step::Rethrow(*handler),
            _ => {
                return Err(RequestError::InternalInvariant(
                    "source throw lost checked completion",
                ));
            }
        };
        self.marker(step, budget)
    }
    pub(super) fn finish(
        self,
        storage: &Dispatcher,
        protection: Option<&crate::p6_regions::protection::Protection>,
        names: &[super::protected::CatchName],
        budget: &mut RunContext<'_>,
    ) -> Result<Plan, RequestError> {
        enum Scope {
            IfYes,
            IfNo,
            Loop(Label),
            Synchronized,
            Try,
            BetweenCatches,
            Catch,
        }
        let mut scopes = Vec::new();
        let mut statements = Vec::new();
        let mut copies = Vec::new();
        for _ in &storage.cases {
            push(&mut statements, false, budget)?;
        }
        for step in &self.steps {
            budget.checkpoint()?;
            match step {
                Step::Statements(id) => {
                    let at = usize::try_from(id.0).map_err(|_| RequestError::PlatformCapacity)?;
                    let seen = statements
                        .get_mut(at)
                        .ok_or(RequestError::InternalInvariant(
                            "source statements lost block",
                        ))?;
                    if *seen {
                        return Err(RequestError::InternalInvariant(
                            "source statements have multiple owners",
                        ));
                    }
                    *seen = true;
                }
                Step::Copies { source, edge } => {
                    let case = storage
                        .cases
                        .get(
                            usize::try_from(source.0)
                                .map_err(|_| RequestError::PlatformCapacity)?,
                        )
                        .ok_or(RequestError::InternalInvariant("source copies lost block"))?;
                    let found = match &case.transfer {
                        Transfer::Jump(e) => e.id == *edge,
                        Transfer::Branch { yes, no, .. } => yes.id == *edge || no.id == *edge,
                        _ => false,
                    };
                    if !found {
                        return Err(RequestError::InternalInvariant(
                            "source copies lost original edge",
                        ));
                    }
                    let at = usize::try_from(edge.0).map_err(|_| RequestError::PlatformCapacity)?;
                    while copies.len() <= at {
                        budget.checkpoint()?;
                        push(&mut copies, false, budget)?;
                    }
                    if copies[at] {
                        return Err(RequestError::InternalInvariant(
                            "source edge has multiple owners",
                        ));
                    }
                    copies[at] = true;
                }
                Step::If(id) => {
                    if !matches!(
                        storage
                            .cases
                            .get(
                                usize::try_from(id.0)
                                    .map_err(|_| RequestError::PlatformCapacity)?
                            )
                            .map(|c| &c.transfer),
                        Some(Transfer::Branch { .. })
                    ) {
                        return Err(RequestError::InternalInvariant("source if lost condition"));
                    }
                    push(&mut scopes, Scope::IfYes, budget)?;
                }
                Step::Else => {
                    let scope = scopes
                        .last_mut()
                        .ok_or(RequestError::InternalInvariant("source else lost scope"))?;
                    if !matches!(scope, Scope::IfYes) {
                        return Err(RequestError::InternalInvariant("source else changed scope"));
                    }
                    *scope = Scope::IfNo;
                }
                Step::Loop(label) => push(&mut scopes, Scope::Loop(*label), budget)?,
                Step::Break(label) | Step::Continue(label) => {
                    let mut found = false;
                    for scope in scopes.iter().rev() {
                        budget.checkpoint()?;
                        if let Scope::Loop(actual) = scope
                            && (*label == Label::Nearest || label == actual)
                        {
                            found = true;
                            break;
                        }
                    }
                    if !found {
                        return Err(RequestError::InternalInvariant(
                            "source transfer has no lexical loop",
                        ));
                    }
                }
                Step::Synchronized(_) => push(&mut scopes, Scope::Synchronized, budget)?,
                Step::Try(_) => push(&mut scopes, Scope::Try, budget)?,
                Step::CloseTryBody => {
                    let scope = scopes
                        .last_mut()
                        .ok_or(RequestError::InternalInvariant("source try lost scope"))?;
                    if !matches!(scope, Scope::Try) {
                        return Err(RequestError::InternalInvariant("source try changed scope"));
                    }
                    *scope = Scope::BetweenCatches;
                }
                Step::Catch { alternatives, .. } => {
                    if alternatives.is_empty() {
                        return Err(RequestError::InternalInvariant(
                            "source catch lost alternatives",
                        ));
                    }
                    let scope = scopes
                        .last_mut()
                        .ok_or(RequestError::InternalInvariant("source catch lost try"))?;
                    if !matches!(scope, Scope::BetweenCatches) {
                        return Err(RequestError::InternalInvariant(
                            "source catch changed scope",
                        ));
                    }
                    *scope = Scope::Catch;
                }
                Step::EndTry => {
                    if !matches!(scopes.pop(), Some(Scope::BetweenCatches)) {
                        return Err(RequestError::InternalInvariant(
                            "source catch group changed scope",
                        ));
                    }
                }
                Step::End => {
                    let scope = scopes
                        .pop()
                        .ok_or(RequestError::InternalInvariant("source close lost scope"))?;
                    if matches!(scope, Scope::Catch) {
                        push(&mut scopes, Scope::BetweenCatches, budget)?;
                    }
                }
                _ => {}
            }
        }
        if !scopes.is_empty() {
            return Err(RequestError::InternalInvariant(
                "source control has unclosed scopes",
            ));
        }
        super::source_replay::verify(&self.steps, storage, protection, names, budget)?;
        Ok(Plan { steps: self.steps })
    }
}
