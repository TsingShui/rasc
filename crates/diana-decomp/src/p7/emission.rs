//! Final syntax placement. No graph, type or copy solver runs here: every role,
//! target and storage location is already delivered by P5/P6 and owned by P7.
use super::build::{bug, copy, filled, id, push};
use super::*;
use crate::p2::{BlockId, OpId};
use crate::p5::{CopyLocation, ThrowSite, TransferSite};
use serde::Serialize;

#[derive(Default, Debug, Serialize)]
pub struct EvaluationPhases {
    pub before: Vec<CopyPlanId>,
    pub snapshots: Vec<CopyPlanId>,
    pub after: Vec<CopyPlanId>,
}
#[derive(Clone, Copy, Debug, Serialize)]
pub enum BodyAction {
    Statement {
        operation: OpId,
        statement: StatementId,
    },
    /// This syntax owns the original matched release. It closes only the named
    /// lock scope, not lexical try scopes; subsequent nodes stay at their origins.
    Release {
        operation: OpId,
        statement: StatementId,
        scope: ControlId,
    },
    /// Executed only on exceptional arrival, after capture when present.
    Receive { handler: u32 },
}
#[derive(Debug, Serialize)]
pub enum BlockEnd {
    None,
    Terminal(BlockId),
    Transfers(Vec<TransferId>),
    /// Continue at the original initializer's node after a selected compound
    /// expression, including backward-PC channels. This is the P5 contraction,
    /// not a newly analysed runtime edge or an inferred lexical fallthrough.
    ConstructionFollow {
        group: ConstructionId,
        target: LabelId,
    },
}
#[derive(Debug, Serialize)]
pub struct EmissionBlock {
    pub actions: Vec<BodyAction>,
    pub end: BlockEnd,
}
#[derive(Clone, Copy, Debug, Serialize)]
pub struct PredicateEmission {
    pub terminal: BlockId,
    /// Copies belong to selection of these original edges, never to a common
    /// loop continuation. A break does not execute the predicate-exit copies.
    pub when_true: TransferId,
    pub when_false: TransferId,
    pub negate: bool,
}
#[derive(Debug, Serialize)]
pub enum LoopPlacement {
    Infinite,
    While {
        predicate: ControlId,
        test: PredicateEmission,
    },
    DoWhile {
        predicate: ControlId,
        test: PredicateEmission,
    },
}
#[derive(Debug, Serialize)]
pub enum EmissionLayout {
    Block(crate::p6::NodeId),
    Sequence,
    If {
        header: ControlId,
        yes: ControlId,
        no: ControlId,
        test: PredicateEmission,
    },
    Loop {
        body: ControlId,
        placement: LoopPlacement,
    },
    Switch {
        header: ControlId,
        body: ControlId,
        terminal: BlockId,
    },
    Labelled,
    Try {
        body: ControlId,
        catches: Vec<ControlId>,
    },
    /// Explicit planned exits prevent implicit second releases, including when
    /// an unlocked routing terminal remains within its original nested catch.
    Synchronized {
        entry: ControlId,
        body: ControlId,
        acquire: OpId,
    },
    Graph,
}
#[derive(Debug, Serialize)]
pub struct EmissionControl {
    pub label: String,
    pub continuation: Option<LabelId>,
    pub layout: EmissionLayout,
}
#[derive(Clone, Copy, Debug, Serialize)]
pub enum TransferSpelling {
    Goto(LabelId),
    Fallthrough,
    Break(ControlId),
    Continue(ControlId),
    Selected,
}
#[derive(Debug, Serialize)]
pub struct EmissionTransfer {
    pub copies: Vec<CopyPlanId>,
    pub spelling: TransferSpelling,
}
#[derive(Debug, Serialize)]
pub enum LocalSpelling {
    Named(TypeUseId),
    Machine(#[serde(serialize_with = "super::debug")] crate::p5::StorageType),
}
/// Sole execution layout consumed by P9. Source arenas and outlet-completion
/// records remain evidence, not additional schedules to emit alongside this one.
#[derive(Debug, Serialize)]
pub struct EmissionPlan {
    pub(super) local_types: Vec<LocalSpelling>,
    pub(super) controls: Vec<EmissionControl>,
    pub(super) blocks: Vec<EmissionBlock>,
    pub(super) operations: Vec<EvaluationPhases>,
    pub(super) terminals: Vec<EvaluationPhases>,
    pub(super) transfers: Vec<EmissionTransfer>,
    pub(super) initial: Vec<CopyPlanId>,
    pub(super) statement_operations: Vec<Option<OpId>>,
}
impl EmissionPlan {
    pub(super) fn empty() -> Self {
        Self {
            local_types: Vec::new(),
            controls: Vec::new(),
            blocks: Vec::new(),
            operations: Vec::new(),
            terminals: Vec::new(),
            transfers: Vec::new(),
            initial: Vec::new(),
            statement_operations: Vec::new(),
        }
    }
    pub fn local_types(&self) -> &[LocalSpelling] {
        &self.local_types
    }
    pub fn controls(&self) -> &[EmissionControl] {
        &self.controls
    }
    pub fn blocks(&self) -> &[EmissionBlock] {
        &self.blocks
    }
    pub fn operations(&self) -> &[EvaluationPhases] {
        &self.operations
    }
    pub fn terminals(&self) -> &[EvaluationPhases] {
        &self.terminals
    }
    pub fn transfers(&self) -> &[EmissionTransfer] {
        &self.transfers
    }
    pub fn initial(&self) -> &[CopyPlanId] {
        &self.initial
    }
    pub fn statement_operation(&self, id: StatementId) -> Option<OpId> {
        self.statement_operations
            .get(id.0 as usize)
            .copied()
            .flatten()
    }
}
fn child(c: &Control, n: usize) -> Result<ControlId, ExpressionError> {
    c.children
        .get(n)
        .copied()
        .ok_or(bug("syntax role has no selected child"))
}
fn predicate(
    code: &MethodCode,
    terminal: BlockId,
    repeat: Option<TransferId>,
) -> Result<PredicateEmission, ExpressionError> {
    let TerminalKind::If {
        when_true,
        when_false,
        ..
    } = code.terminals[terminal.0 as usize].kind
    else {
        return Err(bug("selected condition lost its original branch"));
    };
    if repeat.is_some_and(|e| e != when_true && e != when_false) {
        return Err(bug("loop repeat is not an original condition edge"));
    }
    Ok(PredicateEmission {
        terminal,
        when_true,
        when_false,
        negate: repeat == Some(when_false),
    })
}
pub(super) fn local_spelling(types: &[TypeUse], l: &LocalDeclaration) -> LocalSpelling {
    if matches!(
        l.original.storage_type(),
        crate::p5::StorageType::Family {
            family: crate::p4::Family::Reference,
            ..
        }
    ) && !l.type_source_unknown
        && let Some(&first) = l.declared_types.first()
        && l.declared_types
            .iter()
            .all(|t| types[t.0 as usize].units == types[first.0 as usize].units)
    {
        return LocalSpelling::Named(first);
    }
    LocalSpelling::Machine(l.original.storage_type())
}
pub(super) fn construct(code: &mut MethodCode) -> Result<(), ExpressionError> {
    let mut plan = EmissionPlan::empty();
    for l in &code.locals {
        let syntax = local_spelling(&code.types, l);
        push(&mut plan.local_types, syntax)?;
    }
    crate::substrate::owned::reserve(&mut plan.operations, code.operations.len())?;
    plan.operations
        .resize_with(code.operations.len(), EvaluationPhases::default);
    crate::substrate::owned::reserve(&mut plan.terminals, code.terminals.len())?;
    plan.terminals
        .resize_with(code.terminals.len(), EvaluationPhases::default);
    plan.statement_operations = filled(code.statements.len(), None)?;
    let mut nodes = Vec::new();
    crate::substrate::owned::reserve(&mut nodes, code.blocks.len())?;
    nodes.resize_with(code.blocks.len(), Vec::new);
    for op in &code.operations {
        push(&mut nodes[op.node.0 as usize], op.source)?;
        if let Some(s) = op.statement {
            plan.statement_operations[s.0 as usize] = Some(op.source);
        }
    }
    let mut edge_links = filled(
        code.transfers
            .iter()
            .filter_map(|t| t.edge)
            .map(|e| e.0)
            .max()
            .map(|n| {
                usize::try_from(n)
                    .ok()
                    .and_then(|n| n.checked_add(1))
                    .ok_or(ExpressionError::from(crate::RequestError::PlatformCapacity))
            })
            .transpose()?
            .unwrap_or(0),
        None,
    )?;
    for t in &code.transfers {
        if let Some(edge) = t.edge {
            edge_links[edge.0 as usize] = Some(t.id);
        }
        let spelling = match t.kind {
            TransferKind::Goto(l) => TransferSpelling::Goto(l),
            TransferKind::Next(_) => TransferSpelling::Fallthrough,
            TransferKind::Break(c) => TransferSpelling::Break(c),
            TransferKind::Continue(c) => TransferSpelling::Continue(c),
            TransferKind::LeaveSync { .. } | TransferKind::ReleasedSync { .. } => {
                TransferSpelling::Goto(t.destination)
            }
            TransferKind::SwitchArm(_) => TransferSpelling::Goto(t.destination),
            TransferKind::IfArm { .. }
            | TransferKind::LoopCondition { .. }
            | TransferKind::EnterSync(_) => TransferSpelling::Selected,
        };
        push(
            &mut plan.transfers,
            EmissionTransfer {
                copies: Vec::new(),
                spelling,
            },
        )?;
    }
    for bundle in &code.copies {
        if matches!(bundle.emission, CopyEmission::Construction(_)) {
            continue;
        }
        let at = bundle.id;
        match bundle.location {
            CopyLocation::Transfer {
                site: TransferSite::InitialEntry,
                ..
            } => push(&mut plan.initial, at)?,
            CopyLocation::Transfer {
                site: TransferSite::BeforeOperation(op),
                ..
            } => push(&mut plan.operations[op.0 as usize].before, at)?,
            CopyLocation::BeforeThrow(ThrowSite::Operation(op)) => {
                push(&mut plan.operations[op.0 as usize].snapshots, at)?
            }
            CopyLocation::Transfer {
                site: TransferSite::AfterCommit(op),
                ..
            } => push(&mut plan.operations[op.0 as usize].after, at)?,
            CopyLocation::Transfer {
                site: TransferSite::BeforeTerminal(b),
                ..
            } => push(&mut plan.terminals[b.0 as usize].before, at)?,
            CopyLocation::BeforeThrow(ThrowSite::Terminal(b)) => {
                push(&mut plan.terminals[b.0 as usize].snapshots, at)?
            }
            CopyLocation::Transfer {
                site: TransferSite::NormalEdge(edge),
                ..
            } => {
                let link = edge_links
                    .get(edge.0 as usize)
                    .copied()
                    .flatten()
                    .ok_or(bug("copy location lost its original normal transfer"))?;
                push(&mut plan.transfers[link.0 as usize].copies, at)?;
            }
            CopyLocation::HandlerEntry { .. } => {}
        }
    }
    for g in &code.constructions {
        if let Some(prelude) = g.prelude {
            push(
                &mut plan.operations[g.allocation.0 as usize].snapshots,
                prelude,
            )?;
        }
    }
    let mut handlers = filled(code.blocks.len(), None)?;
    for (i, h) in code.handlers.iter().enumerate() {
        handlers[h.node.0 as usize] = Some(id(i)?);
    }
    let mut compound_follows = filled(code.blocks.len(), None)?;
    for g in &code.constructions {
        let allocation = code.operations[g.allocation.0 as usize].node;
        let initialization = code.operations[g.initialization.0 as usize].node;
        if allocation != initialization {
            compound_follows[allocation.0 as usize] = Some((g.id, LabelId(initialization.0)));
        }
    }
    for b in &code.blocks {
        let h = handlers[b.node.0 as usize];
        let mut actions = Vec::new();
        if let Some(handler) = h
            && matches!(
                code.handlers[handler as usize].capture,
                HandlerCapture::None
            )
        {
            push(&mut actions, BodyAction::Receive { handler })?;
        }
        for &op in &nodes[b.node.0 as usize] {
            let o = &code.operations[op.0 as usize];
            let action = match o.emission {
                Emission::Block { .. } | Emission::Catch(_) => Some(BodyAction::Statement {
                    operation: op,
                    statement: o
                        .statement
                        .ok_or(bug("emitted operation has no statement"))?,
                }),
                Emission::Construction(g) if code.constructions[g.0 as usize].allocation == op => {
                    Some(BodyAction::Statement {
                        operation: op,
                        statement: code.constructions[g.0 as usize].statement,
                    })
                }
                Emission::Release(scope) => Some(BodyAction::Release {
                    operation: op,
                    statement: o.statement.ok_or(bug("release has no statement"))?,
                    scope,
                }),
                Emission::Acquire(_) | Emission::Literal | Emission::Construction(_) => None,
            };
            if let Some(a) = action {
                push(&mut actions, a)?;
            }
            if let Some(handler) = h {
                let capture = match code.handlers[handler as usize].capture {
                    HandlerCapture::None => None,
                    HandlerCapture::Body(s) | HandlerCapture::Catch { statement: s, .. } => Some(s),
                };
                if capture.is_some() && capture == o.statement {
                    push(&mut actions, BodyAction::Receive { handler })?;
                }
            }
        }
        let end = if let Some((group, target)) = compound_follows[b.node.0 as usize] {
            BlockEnd::ConstructionFollow { group, target }
        } else {
            match b.completion {
                BlockCompletion::Ordinary(t)
                | BlockCompletion::Syntax {
                    terminal: t,
                    owner: TerminalEmission::SyncExit(_),
                } => BlockEnd::Terminal(t),
                BlockCompletion::Syntax { .. } => BlockEnd::None,
                BlockCompletion::None => {
                    let mut links = Vec::new();
                    for &t in &b.transfers {
                        if code.transfers[t.0 as usize].construction.is_none()
                            && !matches!(
                                plan.transfers[t.0 as usize].spelling,
                                TransferSpelling::Selected
                            )
                        {
                            push(&mut links, t)?;
                        }
                    }
                    BlockEnd::Transfers(links)
                }
            }
        };
        push(&mut plan.blocks, EmissionBlock { actions, end })?;
    }
    for c in &code.controls {
        let layout = match &c.kind {
            ControlKind::Block { node } => EmissionLayout::Block(*node),
            ControlKind::Sequence => EmissionLayout::Sequence,
            ControlKind::If { terminal, .. } => EmissionLayout::If {
                header: child(c, 0)?,
                yes: child(c, 1)?,
                no: child(c, 2)?,
                test: predicate(code, *terminal, None)?,
            },
            ControlKind::Loop { syntax, .. } => EmissionLayout::Loop {
                body: child(c, 0)?,
                placement: match syntax {
                    LoopSyntax::Infinite => LoopPlacement::Infinite,
                    LoopSyntax::While {
                        terminal, repeat, ..
                    } => LoopPlacement::While {
                        predicate: child(c, 1)?,
                        test: predicate(code, *terminal, Some(*repeat))?,
                    },
                    LoopSyntax::DoWhile {
                        terminal, repeat, ..
                    } => LoopPlacement::DoWhile {
                        predicate: child(c, 1)?,
                        test: predicate(code, *terminal, Some(*repeat))?,
                    },
                },
            },
            ControlKind::Switch { terminal, .. } => EmissionLayout::Switch {
                header: child(c, 0)?,
                body: child(c, 1)?,
                terminal: *terminal,
            },
            ControlKind::LabelledBlock { .. } => EmissionLayout::Labelled,
            ControlKind::Try { .. } => EmissionLayout::Try {
                body: child(c, 0)?,
                catches: copy(
                    c.children
                        .get(1..)
                        .ok_or(bug("try lost handler children"))?,
                )?,
            },
            ControlKind::Synchronized { acquire, .. } => EmissionLayout::Synchronized {
                entry: child(c, 0)?,
                body: child(c, 1)?,
                acquire: plan
                    .statement_operation(*acquire)
                    .ok_or(bug("synchronized acquire lost its original operation"))?,
            },
            ControlKind::Graph { .. } => EmissionLayout::Graph,
        };
        push(
            &mut plan.controls,
            EmissionControl {
                label: super::references::text(format_args!("R{}", c.id.0))?,
                continuation: match c.continuation {
                    Continuation::Goto(label) => Some(label),
                    _ => None,
                },
                layout,
            },
        )?;
    }
    let mut headers = filled(plan.controls.len(), false)?;
    for c in &plan.controls {
        let header = match c.layout {
            EmissionLayout::If { header, .. } | EmissionLayout::Switch { header, .. } => {
                Some(header)
            }
            EmissionLayout::Loop {
                placement:
                    LoopPlacement::While { predicate, .. } | LoopPlacement::DoWhile { predicate, .. },
                ..
            } => Some(predicate),
            EmissionLayout::Synchronized { entry, .. } => Some(entry),
            _ => None,
        };
        if let Some(header) = header {
            headers[header.0 as usize] = true;
        }
    }
    for (i, c) in plan.controls.iter_mut().enumerate() {
        let deferred = match c.layout {
            EmissionLayout::Block(n) => match &plan.blocks[n.0 as usize].end {
                BlockEnd::None | BlockEnd::ConstructionFollow { .. } => true,
                BlockEnd::Transfers(ts) => ts.is_empty(),
                BlockEnd::Terminal(_) => false,
            },
            _ => false,
        };
        if headers[i] || deferred {
            c.continuation = None;
        }
    }
    code.emission = plan;
    Ok(())
}
