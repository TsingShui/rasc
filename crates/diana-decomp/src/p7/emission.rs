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
}
/// The exact printed carrier scope that shows an original `check-cast` instead
/// of its own statement. A check writes no register, so only a read of the
/// checked value can carry it.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub enum CheckCarrier {
    /// A statement scope; `InlineCheck::occurrence` names its exact read.
    Statement(StatementId),
    /// One step of one original copy bundle. Only this step's right-hand side
    /// reads the value; the bundle's other steps and gap reports keep their own
    /// text at their own positions.
    Copy { bundle: CopyPlanId, step: u32 },
}
/// A `check-cast` printed at the first use of its value instead of as its own
/// statement. The check and its operation keep their provenance there.
#[derive(Clone, Copy, Debug, Serialize)]
pub struct InlineCheck {
    pub check: StatementId,
    pub operation: OpId,
    pub pc: crate::CodeUnitOffset,
    pub value: StorageId,
    pub reference: ReferenceId,
    pub carrier: CheckCarrier,
    /// The one printed expression occurrence that reads `value` for this
    /// carrier: the statement's first evaluated read, or the copy step's
    /// right-hand side. P9 matches it exactly, so another read of the same
    /// storage never prints the check again.
    pub occurrence: ExprId,
}
/// A `check-cast` whose own display action cannot add behavior: P4 established
/// from pre-check facts that every reaching transfer already proves the exact
/// membership this check would resolve. The statement, operation, route and any
/// copy keep their original facts and positions; only its text disappears, and
/// its origins stay as zero-length anchors at that position.
#[derive(Clone, Debug, Serialize)]
pub struct OmittedCheck {
    pub check: StatementId,
    pub operation: OpId,
    pub pc: crate::CodeUnitOffset,
    pub value: StorageId,
    pub reference: ReferenceId,
    /// The original establishing events P4 relied on, for this product's reader.
    pub sources: Vec<crate::p4::CheckSource>,
}
/// One original definition evaluated at its unique, adjacent terminal use.
/// A payload printed as the allocation's own array literal. The allocation, the
/// fill statement and the payload keep their original facts; only the spelling
/// and the omitted fill statement change.
#[derive(Clone, Copy, Debug, Serialize)]
pub struct ArrayLiteral {
    pub payload: PayloadId,
    /// The fill statement whose text the literal states; it prints nothing.
    pub fill: StatementId,
    pub operation: OpId,
    pub pc: crate::CodeUnitOffset,
}
#[derive(Clone, Copy, Debug, Serialize)]
pub struct ComposedExpression {
    pub expression: ExprId,
    pub operation: OpId,
    pub statement: StatementId,
    pub pc: crate::CodeUnitOffset,
    pub result_pc: Option<crate::CodeUnitOffset>,
    pub site: Option<SiteId>,
    pub boolean: bool,
    /// A cross-node composition closes the source control at this use.
    pub source_control: Option<ControlId>,
}
#[derive(Debug, Serialize)]
pub struct EmissionBlock {
    /// An unreferenced entry remains a zero-length source anchor.
    pub show_label: bool,
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
        compact: bool,
    },
    DoWhile {
        predicate: ControlId,
        test: PredicateEmission,
        compact: bool,
    },
}
#[derive(Clone, Copy, Debug, Serialize)]
pub enum IfShape {
    IfElse,
    ElseIf,
    /// The false edge and its original body become the sole printed arm. Any
    /// true-edge copy bundles have no steps or diagnostics, only source anchors.
    Inverted,
    /// Only the header remains; evaluating the predicate has no displayed work.
    Empty,
    /// The true arm always leaves the branch, so the false arm prints after the
    /// closed `if` instead of behind a redundant `else`.
    NoElse,
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
        shape: IfShape,
    },
    /// Several original predicates print as one short-circuit condition. The
    /// shared arm prints on `select`; the optional other arm prints otherwise.
    Condition {
        header: ControlId,
        tests: Vec<PredicateEmission>,
        yes: ControlId,
        no: Option<ControlId>,
        fall: TransferId,
    },
    Loop {
        body: ControlId,
        placement: LoopPlacement,
    },
    Switch {
        header: ControlId,
        bodies: Vec<ControlId>,
        private_cases: bool,
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
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub enum ControlCompletion {
    Goto(LabelId),
    Break(ControlId),
    Continue(ControlId),
    BreakHere(ControlId),
    ContinueHere(ControlId),
}

#[derive(Debug, Serialize)]
pub struct EmissionControl {
    pub label: String,
    /// Retain a labeled scope only when another emitted transfer names it.
    pub label_required: bool,
    /// Display-only omission; the original control remains in the product.
    pub omitted: bool,
    /// A labelled scope whose own braces print nothing but one unnamed printed
    /// child. P9 prints that child directly; the control, its labels and its
    /// transfers stay in the product.
    pub transparent: bool,
    pub continuation: Option<ControlCompletion>,
    /// Selected P6 explicit order; the retained control still owns its original
    /// tests/shared/other roles. Other layouts use the source child list.
    pub explicit_children: Option<Vec<ControlId>>,
    pub layout: EmissionLayout,
}
impl EmissionControl {
    pub fn children<'a>(&'a self, source: &'a Control) -> &'a [ControlId] {
        self.explicit_children
            .as_deref()
            .unwrap_or(&source.children)
    }
}
#[derive(Clone, Copy, Debug, Serialize)]
pub enum TransferSpelling {
    Goto(LabelId),
    Fallthrough,
    Break(ControlId),
    Continue(ControlId),
    /// The target is the innermost enclosing scope, so no label is named.
    BreakHere(ControlId),
    ContinueHere(ControlId),
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
    pub(super) show_chains: Vec<bool>,
    /// Whether a route annotation still adds information beyond the printed
    /// chains and catch scopes.
    pub(super) site_annotations: Vec<bool>,
    /// A field reference prints its declared type only when the delivered
    /// references would otherwise be indistinguishable.
    pub(super) field_qualifications: Vec<bool>,
    /// A store prints as an ordinary assignment only when reordering its
    /// operands to Java's left-to-right order cannot be observed.
    pub(super) natural_stores: Vec<bool>,
    pub(super) inline_checks: Vec<InlineCheck>,
    /// Indices into `inline_checks`, by the statement that carries the check.
    pub(super) check_uses: Vec<Vec<u32>>,
    /// Indices into `inline_checks`, by the copy bundle that carries the check.
    pub(super) copy_check_uses: Vec<Vec<u32>>,
    pub(super) composed_uses: Vec<Option<ComposedExpression>>,
    pub(super) composed_operations: Vec<bool>,
    /// By the statement whose allocation prints the payload.
    pub(super) array_literals: Vec<Option<ArrayLiteral>>,
    /// By the statement whose check prints nothing because P4 established it
    /// cannot add behavior at that position.
    pub(super) omitted_checks: Vec<Option<OmittedCheck>>,
    /// By the statement that allocates an object without constructing it yet.
    /// It prints the allocated type and its local where that allocation happens.
    pub(super) allocation_declarations: Vec<bool>,
    /// By the local whose declaration is that allocation statement.
    pub(super) allocation_locals: Vec<bool>,
    pub(super) hidden_locals: Vec<bool>,
    pub(super) deferred_controls: Vec<bool>,
    pub(super) operations: Vec<EvaluationPhases>,
    pub(super) terminals: Vec<EvaluationPhases>,
    /// Display-only omission of a copy-free fallthrough false arm.
    pub(super) empty_else: Vec<bool>,
    /// Display-only omission of a method body's closing void return, whose
    /// closing brace performs the same exit.
    pub(super) omitted_returns: Vec<bool>,
    /// Both explicit If arms are fallthrough with only nonprinting copies;
    /// the predicate has no evaluation or phase that needs to print.
    pub(super) empty_if: Vec<bool>,
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
            show_chains: Vec::new(),
            site_annotations: Vec::new(),
            field_qualifications: Vec::new(),
            natural_stores: Vec::new(),
            inline_checks: Vec::new(),
            check_uses: Vec::new(),
            copy_check_uses: Vec::new(),
            composed_uses: Vec::new(),
            composed_operations: Vec::new(),
            array_literals: Vec::new(),
            omitted_checks: Vec::new(),
            allocation_declarations: Vec::new(),
            allocation_locals: Vec::new(),
            hidden_locals: Vec::new(),
            deferred_controls: Vec::new(),
            operations: Vec::new(),
            terminals: Vec::new(),
            empty_else: Vec::new(),
            omitted_returns: Vec::new(),
            empty_if: Vec::new(),
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
    pub fn omitted(&self, id: ControlId) -> bool {
        self.controls
            .get(id.0 as usize)
            .is_some_and(|control| control.omitted)
    }
    pub fn blocks(&self) -> &[EmissionBlock] {
        &self.blocks
    }
    pub fn show_chains(&self) -> &[bool] {
        &self.show_chains
    }
    /// Every check-cast this plan prints at a use instead of its own statement.
    pub fn inline_checks(&self) -> &[InlineCheck] {
        &self.inline_checks
    }
    /// The check-cast carried by this exact printed occurrence for a read of
    /// `value`. A statement carrier also requires the expression occurrence P7
    /// selected, so another read of the same storage never prints the check.
    pub fn inline_check(
        &self,
        carrier: CheckCarrier,
        occurrence: ExprId,
        value: StorageId,
    ) -> Option<InlineCheck> {
        let uses = match carrier {
            CheckCarrier::Statement(statement) => self.check_uses.get(statement.0 as usize)?,
            CheckCarrier::Copy { bundle, .. } => self.copy_check_uses.get(bundle.0 as usize)?,
        };
        for &index in uses {
            let check = self.inline_checks.get(index as usize)?;
            if check.carrier == carrier && check.occurrence == occurrence && check.value == value {
                return Some(*check);
            }
        }
        None
    }
    pub fn natural_store(&self, id: StatementId) -> bool {
        self.natural_stores
            .get(id.0 as usize)
            .copied()
            .unwrap_or(false)
    }
    pub fn field_qualification(&self, id: ReferenceId) -> bool {
        self.field_qualifications
            .get(id.0 as usize)
            .copied()
            .unwrap_or(true)
    }
    pub fn site_annotation(&self, id: SiteId) -> bool {
        self.site_annotations
            .get(id.0 as usize)
            .copied()
            .unwrap_or(true)
    }
    pub fn composed_use(&self, id: ExprId) -> Option<ComposedExpression> {
        self.composed_uses.get(id.0 as usize).copied().flatten()
    }
    pub fn composed_operation(&self, id: OpId) -> bool {
        self.composed_operations
            .get(id.0 as usize)
            .copied()
            .unwrap_or(false)
    }
    pub fn array_literal(&self, id: StatementId) -> Option<ArrayLiteral> {
        self.array_literals.get(id.0 as usize).copied().flatten()
    }
    /// The original check this statement's own display omits, if P4 established
    /// it cannot add behavior at this position.
    pub fn omitted_check(&self, id: StatementId) -> Option<&OmittedCheck> {
        self.omitted_checks.get(id.0 as usize)?.as_ref()
    }
    pub fn hidden_local(&self, id: StorageId) -> bool {
        self.hidden_locals
            .get(id.0 as usize)
            .copied()
            .unwrap_or(false)
    }
    pub fn allocation_declaration(&self, id: StatementId) -> bool {
        self.allocation_declarations
            .get(id.0 as usize)
            .copied()
            .unwrap_or(false)
    }
    pub fn allocation_local(&self, id: StorageId) -> bool {
        self.allocation_locals
            .get(id.0 as usize)
            .copied()
            .unwrap_or(false)
    }
    pub fn deferred_control(&self, id: ControlId) -> bool {
        self.deferred_controls
            .get(id.0 as usize)
            .copied()
            .unwrap_or(false)
    }
    pub fn operations(&self) -> &[EvaluationPhases] {
        &self.operations
    }
    pub fn empty_else(&self, id: BlockId) -> bool {
        self.empty_else.get(id.0 as usize).copied().unwrap_or(false)
    }
    pub fn omitted_return(&self, id: BlockId) -> bool {
        self.omitted_returns
            .get(id.0 as usize)
            .copied()
            .unwrap_or(false)
    }
    pub fn empty_if(&self, id: BlockId) -> bool {
        self.empty_if.get(id.0 as usize).copied().unwrap_or(false)
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
/// A test prints the condition that selects the shared arm. `select` is an
/// original conditional edge, so negation only inverts the delivered role.
fn chained_predicate(
    code: &MethodCode,
    test: &super::ConditionTest,
) -> Result<PredicateEmission, ExpressionError> {
    let block = code
        .blocks
        .get(test.predicate.0 as usize)
        .ok_or(bug("condition test lost its block"))?
        .original_block;
    let TerminalKind::If {
        when_true,
        when_false,
        ..
    } = code.terminals[block.0 as usize].kind
    else {
        return Err(bug("condition test lost its original branch"));
    };
    if test.select != when_true && test.select != when_false {
        return Err(bug("condition select is not an original branch edge"));
    }
    Ok(PredicateEmission {
        terminal: block,
        when_true,
        when_false,
        negate: test.select == when_false,
    })
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
    plan.show_chains = filled(code.chains.len(), true)?;
    plan.site_annotations = filled(code.sites.len(), true)?;
    plan.field_qualifications = filled(code.references.len(), true)?;
    plan.natural_stores = filled(code.statements.len(), false)?;
    plan.check_uses = filled(code.statements.len(), Vec::new())?;
    plan.copy_check_uses = filled(code.copies.len(), Vec::new())?;
    plan.composed_uses = filled(code.expressions.len(), None)?;
    plan.composed_operations = filled(code.operations.len(), false)?;
    plan.array_literals = filled(code.statements.len(), None)?;
    plan.omitted_checks = filled(code.statements.len(), None)?;
    plan.allocation_declarations = filled(code.statements.len(), false)?;
    plan.allocation_locals = filled(code.locals.len(), false)?;
    plan.hidden_locals = filled(code.locals.len(), false)?;
    plan.deferred_controls = filled(code.controls.len(), false)?;
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
    plan.empty_else = filled(code.terminals.len(), false)?;
    plan.omitted_returns = filled(code.terminals.len(), false)?;
    plan.empty_if = filled(code.terminals.len(), false)?;
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
                Emission::Construction(g)
                    if code.constructions[g.0 as usize].initialization == op =>
                {
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
        let end = match b.completion {
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
        };
        push(
            &mut plan.blocks,
            EmissionBlock {
                show_label: true,
                actions,
                end,
            },
        )?;
    }
    for c in &code.controls {
        let layout = match &c.kind {
            ControlKind::Block { node } => EmissionLayout::Block(*node),
            ControlKind::Sequence => EmissionLayout::Sequence,
            ControlKind::Condition {
                tests, else_arm, ..
            } => {
                let runs = tests.len();
                let mut emitted = Vec::new();
                let mut last = None;
                for test in tests {
                    last = Some(test.other);
                    push(&mut emitted, chained_predicate(code, test)?)?;
                }
                EmissionLayout::Condition {
                    header: child(c, 0)?,
                    tests: emitted,
                    yes: child(c, runs)?,
                    no: if *else_arm {
                        Some(child(c, runs + 1)?)
                    } else {
                        None
                    },
                    fall: last.ok_or(bug("condition control has no test"))?,
                }
            }
            ControlKind::If { terminal, .. } => EmissionLayout::If {
                header: child(c, 0)?,
                yes: child(c, 1)?,
                no: child(c, 2)?,
                test: predicate(code, *terminal, None)?,
                shape: IfShape::IfElse,
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
                        compact: false,
                    },
                    LoopSyntax::DoWhile {
                        terminal, repeat, ..
                    } => LoopPlacement::DoWhile {
                        predicate: child(c, 1)?,
                        test: predicate(code, *terminal, Some(*repeat))?,
                        compact: false,
                    },
                },
            },
            ControlKind::Switch {
                terminal,
                arms,
                private_cases,
            } => {
                if *private_cases {
                    if c.children.len() != arms.len() + 1 {
                        return Err(bug("private switch lost its case children"));
                    }
                    for &arm in arms {
                        plan.transfers[arm.0 as usize].spelling = TransferSpelling::Fallthrough;
                    }
                }
                EmissionLayout::Switch {
                    header: child(c, 0)?,
                    bodies: copy(c.children.get(1..).ok_or(bug("switch lost its body"))?)?,
                    private_cases: *private_cases,
                    terminal: *terminal,
                }
            }
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
                label_required: true,
                omitted: false,
                transparent: false,
                explicit_children: None,
                continuation: match c.continuation {
                    Continuation::Goto(label) => Some(ControlCompletion::Goto(label)),
                    Continuation::Break(scope) => Some(ControlCompletion::Break(scope)),
                    Continuation::Continue(scope) => Some(ControlCompletion::Continue(scope)),
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
                BlockEnd::None => true,
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
