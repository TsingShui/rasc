use super::build::{bug, push};
use super::values::Context;
use super::*;
use crate::p2::{EdgeRole, NormalEdge, OpId};
use crate::p5::ThrowSite;
use crate::p6::{self, LinkOrigin, RegionKind, SyncExitKind, SyncLoopTransfer, TransferAction};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct Label {
    pub id: LabelId,
    pub name: String,
    pub node: p6::NodeId,
    pub block: crate::p2::BlockId,
    pub owner: ControlId,
    #[serde(serialize_with = "super::debug")]
    pub kind: p6::LabelKind,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TransferKind {
    Goto(LabelId),
    Next(LabelId),
    Break(ControlId),
    Continue(ControlId),
    IfArm {
        control: ControlId,
        taken: bool,
    },
    SwitchArm(ControlId),
    LoopCondition {
        control: ControlId,
        repeat: bool,
    },
    EnterSync(ControlId),
    LeaveSync {
        control: ControlId,
        release: StatementId,
    },
    ReleasedSync {
        control: ControlId,
        release: StatementId,
    },
}
#[derive(Debug, Serialize)]
pub struct Transfer {
    pub id: TransferId,
    pub from: p6::NodeId,
    pub destination: LabelId,
    pub edge: Option<crate::p2::EdgeId>,
    #[serde(serialize_with = "super::debug")]
    pub role: Option<NormalEdge>,
    pub kind: TransferKind,
    pub copies: Vec<CopyPlanId>,
    pub construction: Option<ConstructionId>,
}
#[derive(Debug, Serialize)]
pub struct CatchType {
    pub original: crate::TypeId,
    pub descriptor: Result<TypeUseId, SourceFailure>,
    pub use_issue: Option<SourceFailure>,
}
#[derive(Debug, Serialize)]
pub struct Clause {
    pub order: u32,
    pub catch: Option<CatchType>,
    pub target: Option<LabelId>,
}
#[derive(Debug, Serialize)]
pub struct ExceptionChain {
    pub id: p6::ChainId,
    pub clauses: Vec<Clause>,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Delivery {
    Explicit,
    Catch(ControlId),
    ImpossibleRelease(StatementId),
}
#[derive(Debug, Serialize)]
pub struct ExceptionRoute {
    pub edge: crate::p2::EdgeId,
    pub order: u32,
    pub delivery: Delivery,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExceptionSyntax {
    Natural,
    Explicit,
    Propagate,
    ImpossibleRelease,
}
#[derive(Debug, Serialize)]
pub struct ExceptionSite {
    pub id: SiteId,
    #[serde(serialize_with = "super::debug")]
    pub original: ThrowSite,
    #[serde(serialize_with = "super::debug")]
    pub completion: crate::p2::ExceptionCompletion,
    pub chain: p6::ChainId,
    pub routes: Vec<ExceptionRoute>,
    pub syntax: ExceptionSyntax,
    pub construction: Option<ConstructionId>,
    pub source_snapshots: Vec<CopyPlanId>,
}
#[derive(Debug, Serialize)]
pub struct Handler {
    pub node: p6::NodeId,
    pub capture: HandlerCapture,
    /// Exception arrivals only, after the original capture when present.
    pub receives: Vec<CopyPlanId>,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HandlerCapture {
    None,
    /// The explicit body retains the original capture statement. The handler
    /// label does not perform a second local binding on exceptional arrival.
    Body(StatementId),
    Catch {
        statement: StatementId,
        control: ControlId,
    },
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LoopSyntax {
    Infinite,
    While {
        terminal: crate::p2::BlockId,
        repeat: TransferId,
        exit: TransferId,
    },
    DoWhile {
        terminal: crate::p2::BlockId,
        repeat: TransferId,
        exit: TransferId,
    },
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Resume {
    Follow(LabelId),
    Break {
        control: ControlId,
        target: LabelId,
    },
    Continue {
        control: ControlId,
        target: LabelId,
    },
    ReturnVoid {
        terminal: crate::p2::BlockId,
    },
    Rethrow {
        terminal: crate::p2::BlockId,
        exception: ExprId,
        capture: OpId,
    },
}
#[derive(Debug, Serialize)]
pub struct SyncOutlet {
    pub entry: LabelId,
    pub release: StatementId,
    pub transfer: TransferId,
    pub destination: LabelId,
    pub resume: Resume,
    pub routing: Vec<p6::NodeId>,
    pub before_operand: Vec<CopyPlanId>,
    pub snapshots: Vec<CopyPlanId>,
    /// Original obligations, referenced by `completion`; do not emit twice.
    pub after_release: Vec<CopyPlanId>,
    pub completion: Vec<OutletStep>,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OutletStep {
    Copies(Vec<CopyPlanId>),
    /// Select the original link, then execute only this selected link's copies.
    Transfer {
        transfer: TransferId,
        copies: Vec<CopyPlanId>,
    },
    /// Before-terminal copies precede operand reads; snapshots follow those
    /// reads and precede the terminal event. `Resume` is not a second event.
    Terminal {
        terminal: crate::p2::BlockId,
        before_operand: Vec<CopyPlanId>,
        snapshots: Vec<CopyPlanId>,
    },
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ControlKind {
    Block {
        node: p6::NodeId,
    },
    Sequence,
    If {
        terminal: crate::p2::BlockId,
        when_true: TransferId,
        when_false: TransferId,
    },
    /// Ordered short-circuit tests delivered by one P6 condition region. The
    /// children are the test runs, then the shared arm, then the optional other
    /// arm; each test keeps its original terminal, links and evaluation order.
    Condition {
        tests: Vec<ConditionTest>,
        else_arm: bool,
        explicit: ConditionSchedule,
    },
    Loop {
        syntax: LoopSyntax,
        continue_target: LabelId,
    },
    Switch {
        terminal: crate::p2::BlockId,
        arms: Vec<TransferId>,
        private_cases: bool,
    },
    LabelledBlock {
        target: LabelId,
    },
    Try {
        chain: p6::ChainId,
        clauses: std::ops::Range<u32>,
        handlers: Vec<p6::NodeId>,
        planning_scope: Option<p6::SyncId>,
    },
    Synchronized {
        acquire: StatementId,
        lock: ExprId,
        enter: TransferId,
        held: Vec<p6::NodeId>,
        outlets: Vec<SyncOutlet>,
    },
    Graph {
        #[serde(serialize_with = "super::debug")]
        reason: p6::RecoveryReason,
        entries: Vec<LabelId>,
        exits: Vec<TransferId>,
    },
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Continuation {
    None,
    Parent,
    Next(LabelId),
    Goto(LabelId),
    Break(ControlId),
    Continue(ControlId),
}
/// One delivered predicate of a short-circuit condition. `select` is the
/// original edge to the shared arm, so it also fixes the printed polarity.
#[derive(Debug, Serialize)]
pub struct ConditionTest {
    pub predicate: p6::NodeId,
    pub select: TransferId,
    pub other: TransferId,
}
/// P6's explicit alternative, lowered without changing source child roles.
#[derive(Debug, Serialize)]
pub struct ConditionSchedule {
    pub children: Vec<ControlId>,
    pub completions: Vec<ConditionCompletion>,
    pub gotos: Vec<TransferId>,
}
#[derive(Debug, Serialize)]
pub struct ConditionCompletion {
    pub child: ControlId,
    pub continuation: Continuation,
}
/// P6's selected integer decision chain, retaining the original If tree.
#[derive(Debug, Serialize)]
pub struct Decision {
    pub arms: Vec<DecisionArm>,
    pub default: ControlId,
}
#[derive(Clone, Copy, Debug, Serialize)]
pub struct DecisionArm {
    pub source: ControlId,
    pub header: ControlId,
    pub terminal: crate::p2::BlockId,
    pub predicate: p6::NodeId,
    pub selector: ExprId,
    pub key_expression: Option<ExprId>,
    pub key: i32,
    pub selected: TransferId,
    pub remaining: TransferId,
    pub body: ControlId,
}
#[derive(Debug, Serialize)]
pub struct Control {
    pub decision: Option<Decision>,
    pub id: ControlId,
    pub kind: ControlKind,
    pub children: Vec<ControlId>,
    pub entry: Option<LabelId>,
    pub follow: Option<LabelId>,
    pub continuation: Continuation,
}

pub(super) fn label(c: &Context<'_, '_>, node: p6::NodeId) -> Result<LabelId, ExpressionError> {
    let p = c
        .planned
        .facts()
        .labels()
        .get(node.0 as usize)
        .ok_or(bug("control target lost its P6 label"))?;
    if p.node() != node {
        return Err(bug("control label no longer binds its source node"));
    }
    Ok(LabelId(p.id().0))
}
pub(super) fn statement(c: &Context<'_, '_>, op: OpId) -> Result<StatementId, ExpressionError> {
    c.code
        .operations
        .get(op.0 as usize)
        .and_then(|o| o.statement)
        .ok_or(bug("syntax-owned operation lost its statement"))
}
fn terminal(c: &Context<'_, '_>, node: p6::NodeId) -> crate::p2::BlockId {
    c.planned.facts().nodes()[node.0 as usize].block()
}
fn ids<T: Copy, U>(source: &[T], map: impl Fn(T) -> U) -> Result<Vec<U>, ExpressionError> {
    let mut out = Vec::new();
    for &v in source {
        push(&mut out, map(v))?;
    }
    Ok(out)
}
fn control(value: Option<ControlId>) -> Result<ControlId, ExpressionError> {
    value.ok_or(bug("transfer names an unselected control region"))
}
fn continuation(
    c: &Context<'_, '_>,
    source: p6::Continuation,
) -> Result<Continuation, ExpressionError> {
    Ok(match source {
        p6::Continuation::None => Continuation::None,
        p6::Continuation::Parent => Continuation::Parent,
        p6::Continuation::Next(n) => Continuation::Next(label(c, n)?),
        p6::Continuation::Goto(l) => Continuation::Goto(LabelId(l.0)),
        p6::Continuation::Break(scope) => Continuation::Break(ControlId(scope.0)),
        p6::Continuation::Continue(id) => Continuation::Continue(control(c.loops[id.0 as usize])?),
    })
}

fn outlet_copies(
    c: &Context<'_, '_>,
    exit: &p6::SyncExit,
    at: impl Fn(crate::p5::CopyLocation) -> bool,
) -> Result<Vec<CopyPlanId>, ExpressionError> {
    let mut out = Vec::new();
    for p in exit.after_release() {
        if at(c.planned.storage_facts().copy_plans()[p.0 as usize].location()) {
            push(&mut out, CopyPlanId(p.0))?;
        }
    }
    Ok(out)
}
fn outlet_transfer(
    c: &Context<'_, '_>,
    exit: &p6::SyncExit,
    link: p6::LinkId,
) -> Result<OutletStep, ExpressionError> {
    let copies = if let LinkOrigin::Edge(edge) = c.planned.facts().links()[link.0 as usize].origin()
    {
        outlet_copies(
            c,
            exit,
            |l| matches!(l,crate::p5::CopyLocation::Transfer{site:crate::p5::TransferSite::NormalEdge(id),..}if id==edge),
        )?
    } else {
        Vec::new()
    };
    Ok(OutletStep::Transfer {
        transfer: TransferId(link.0),
        copies,
    })
}
fn outlet_completion(
    c: &Context<'_, '_>,
    exit: &p6::SyncExit,
) -> Result<Vec<OutletStep>, ExpressionError> {
    use crate::p5::{CopyLocation, TransferSite};
    let mut steps = Vec::new();
    let commits = outlet_copies(
        c,
        exit,
        |l| matches!(l,CopyLocation::Transfer{site:TransferSite::AfterCommit(op),..}if op==exit.release()),
    )?;
    if !commits.is_empty() {
        push(&mut steps, OutletStep::Copies(commits))?;
    }
    push(&mut steps, outlet_transfer(c, exit, exit.release_link())?)?;
    for node in exit.routing() {
        let n = &c.planned.facts().nodes()[node.0 as usize];
        if n.has_terminal() {
            let before_operand = outlet_copies(
                c,
                exit,
                |l| matches!(l,CopyLocation::Transfer{site:TransferSite::BeforeTerminal(b),..}if b==n.block()),
            )?;
            let snapshots = outlet_copies(c, exit, |l| {
                l == CopyLocation::BeforeThrow(ThrowSite::Terminal(n.block()))
            })?;
            push(
                &mut steps,
                OutletStep::Terminal {
                    terminal: n.block(),
                    before_operand,
                    snapshots,
                },
            )?;
        }
        for link in &c.code.blocks[node.0 as usize].transfers {
            push(&mut steps, outlet_transfer(c, exit, p6::LinkId(link.0))?)?;
        }
    }
    Ok(steps)
}
pub(super) fn construct(c: &mut Context<'_, '_>) -> Result<(), ExpressionError> {
    let f = c.planned.facts();
    let flow = c.planned.execution();
    for p in f.labels() {
        let prefix = if p.kind() == p6::LabelKind::Handler {
            "H"
        } else {
            "L"
        };
        push(
            &mut c.code.labels,
            Label {
                id: LabelId(p.id().0),
                name: super::references::text(format_args!("{prefix}{}", p.id().0))?,
                node: p.node(),
                block: p.block(),
                owner: ControlId(p.owner().0),
                kind: p.kind(),
            },
        )?;
    }
    for (link, plan) in f.links().iter().zip(f.link_plans()) {
        let kind = match plan.action() {
            TransferAction::Goto(id) => TransferKind::Goto(LabelId(id.0)),
            TransferAction::Next(n) => TransferKind::Next(label(c, n)?),
            TransferAction::Break(id) => TransferKind::Break(ControlId(id.0)),
            TransferAction::Continue(id) => {
                TransferKind::Continue(control(c.loops[id.0 as usize])?)
            }
            TransferAction::IfArm { region, taken } => TransferKind::IfArm {
                control: ControlId(region.0),
                taken,
            },
            TransferAction::SwitchArm { region } => TransferKind::SwitchArm(ControlId(region.0)),
            TransferAction::LoopCondition { region, repeat } => TransferKind::LoopCondition {
                control: ControlId(region.0),
                repeat,
            },
            TransferAction::EnterSynchronized(id) => TransferKind::EnterSync(ControlId(id.0)),
            TransferAction::LeaveSynchronized { region, release } => TransferKind::LeaveSync {
                control: ControlId(region.0),
                release: statement(c, release)?,
            },
            TransferAction::SynchronizedReleased { region, release } => {
                TransferKind::ReleasedSync {
                    control: ControlId(region.0),
                    release: statement(c, release)?,
                }
            }
        };
        let (edge, role, construction) = match link.origin() {
            LinkOrigin::WithinBlock(_) => (None, None, None),
            LinkOrigin::Edge(id) => {
                let EdgeRole::Normal(role) = flow.edges()[id.0 as usize].role() else {
                    return Err(bug("normal expression link lost its original role"));
                };
                (Some(id), Some(role), c.edge_groups[id.0 as usize])
            }
        };
        let destination = label(c, link.to())?;
        push(
            &mut c.code.transfers,
            Transfer {
                id: TransferId(link.id().0),
                from: link.from(),
                destination,
                edge,
                role,
                kind,
                copies: ids(plan.copies(), |p| CopyPlanId(p.0))?,
                construction,
            },
        )?;
    }
    for chain in f.handler_chains() {
        let mut clauses = Vec::new();
        for clause in chain.clauses() {
            let catch = if let Some(type_id) = clause.catch {
                let fact = flow
                    .catch_type(type_id)
                    .ok_or(bug("catch clause lost its original descriptor result"))?;
                let descriptor = match fact.descriptor() {
                    Ok(name) => Ok(super::references::type_use(
                        &mut c.code.types,
                        crate::substrate::owned::copy_units(name)?,
                        TypeSource::Catch { original: type_id },
                    )?),
                    Err(error) => Err(super::references::failure(
                        error.kind(),
                        format_args!("{error}"),
                    )?),
                };
                Some(CatchType {
                    original: type_id,
                    descriptor,
                    use_issue: fact
                        .use_issue()
                        .map(|e| super::references::failure(e.kind(), format_args!("{e}")))
                        .transpose()?,
                })
            } else {
                None
            };
            let target = match clause.target {
                p6::CatchTarget::Handler(n) => Some(label(c, n)?),
                p6::CatchTarget::Uncaught => None,
            };
            push(
                &mut clauses,
                Clause {
                    order: clause.order,
                    catch,
                    target,
                },
            )?;
        }
        push(
            &mut c.code.chains,
            ExceptionChain {
                id: chain.id(),
                clauses,
            },
        )?;
    }
    for site in f.throws() {
        let mut routes = Vec::new();
        let mut impossible = true;
        for &edge in site.edges() {
            let p6::EdgeRoute::Exception {
                order, delivery, ..
            } = f.edges()[edge.0 as usize].route()
            else {
                return Err(bug("throw site lost its exceptional route"));
            };
            let delivery = match delivery {
                p6::ExceptionDelivery::Explicit => {
                    impossible = false;
                    Delivery::Explicit
                }
                p6::ExceptionDelivery::Catch(t) => {
                    impossible = false;
                    Delivery::Catch(control(c.tries[t.0 as usize])?)
                }
                p6::ExceptionDelivery::ImpossibleRelease(op) => {
                    Delivery::ImpossibleRelease(statement(c, op)?)
                }
            };
            push(
                &mut routes,
                ExceptionRoute {
                    edge,
                    order,
                    delivery,
                },
            )?;
        }
        let syntax = if impossible {
            ExceptionSyntax::ImpossibleRelease
        } else if site.protection() == p6::SiteProtection::Natural {
            ExceptionSyntax::Natural
        } else if f.handler_chains()[site.chain().0 as usize]
            .clauses()
            .iter()
            .all(|x| x.target == p6::CatchTarget::Uncaught)
        {
            ExceptionSyntax::Propagate
        } else {
            ExceptionSyntax::Explicit
        };
        push(
            &mut c.code.sites,
            ExceptionSite {
                id: SiteId(site.id().0),
                original: site.site(),
                completion: site.completion(),
                chain: site.chain(),
                routes,
                syntax,
                construction: site.construction().map(|g| ConstructionId(g.0)),
                source_snapshots: ids(site.snapshots(), |p| CopyPlanId(p.0))?,
            },
        )?;
    }
    for h in f.handler_entries() {
        let capture = if let Some(op) = h.capture() {
            let statement = statement(c, op)?;
            match c.code.operations[op.0 as usize].emission {
                Emission::Catch(control) => HandlerCapture::Catch { statement, control },
                Emission::Block { .. } => HandlerCapture::Body(statement),
                _ => return Err(bug("handler capture has an incompatible expression owner")),
            }
        } else {
            HandlerCapture::None
        };
        push(
            &mut c.code.handlers,
            Handler {
                node: h.node(),
                capture,
                receives: ids(h.receives(), |p| CopyPlanId(p.0))?,
            },
        )?;
    }
    for r in f.regions() {
        let kind = match r.kind() {
            RegionKind::Block(n) => ControlKind::Block { node: *n },
            RegionKind::Sequence => ControlKind::Sequence,
            RegionKind::If {
                true_link,
                false_link,
            } => ControlKind::If {
                terminal: terminal(c, r.entry().ok_or(bug("if has no predicate node"))?),
                when_true: TransferId(true_link.0),
                when_false: TransferId(false_link.0),
            },
            RegionKind::Condition(id) => {
                let plan = &f.conditions()[id.0 as usize];
                let mut tests = Vec::new();
                for test in plan.tests() {
                    push(
                        &mut tests,
                        ConditionTest {
                            predicate: test.predicate(),
                            select: TransferId(test.select().0),
                            other: TransferId(test.other().0),
                        },
                    )?;
                }
                let explicit = plan
                    .explicit()
                    .ok_or(bug("selected condition has no explicit schedule"))?;
                let mut completions = Vec::new();
                for completion in explicit.completions() {
                    push(
                        &mut completions,
                        ConditionCompletion {
                            child: ControlId(completion.child().0),
                            continuation: continuation(c, completion.continuation())?,
                        },
                    )?;
                }
                ControlKind::Condition {
                    tests,
                    else_arm: plan.else_arm(),
                    explicit: ConditionSchedule {
                        children: ids(explicit.children(), |r| ControlId(r.0))?,
                        completions,
                        gotos: ids(explicit.gotos(), |l| TransferId(l.0))?,
                    },
                }
            }
            RegionKind::Loop(id) => {
                let p = &f.loops()[id.0 as usize];
                let syntax = match p.form() {
                    p6::LoopForm::Infinite => LoopSyntax::Infinite,
                    p6::LoopForm::While(test) => LoopSyntax::While {
                        terminal: terminal(c, test.node()),
                        repeat: TransferId(test.repeat().0),
                        exit: TransferId(test.exit().0),
                    },
                    p6::LoopForm::DoWhile(test) => LoopSyntax::DoWhile {
                        terminal: terminal(c, test.node()),
                        repeat: TransferId(test.repeat().0),
                        exit: TransferId(test.exit().0),
                    },
                };
                ControlKind::Loop {
                    syntax,
                    continue_target: label(c, p.continue_target())?,
                }
            }
            RegionKind::Switch {
                arms,
                private_cases,
            } => ControlKind::Switch {
                private_cases: *private_cases,
                terminal: terminal(c, r.entry().ok_or(bug("switch has no selector node"))?),
                arms: ids(arms, |a| TransferId(a.0))?,
            },
            RegionKind::LabelledBlock { target } => ControlKind::LabelledBlock {
                target: label(c, *target)?,
            },
            RegionKind::Try(id) => {
                let p = &f.tries()[id.0 as usize];
                ControlKind::Try {
                    chain: p.chain(),
                    clauses: p.clauses(),
                    handlers: ids(p.handlers(), |n| n)?,
                    planning_scope: p.outer_sync(),
                }
            }
            RegionKind::Synchronized(id) => {
                let p = &f.synchronized()[id.0 as usize];
                let acquire = statement(c, p.enter())?;
                let StatementKind::Monitor { enter: true, lock } =
                    c.code.statements[acquire.0 as usize].kind
                else {
                    return Err(bug("synchronized acquire lost its monitor expression"));
                };
                let mut outlets = Vec::new();
                for exit in p.exits() {
                    let release = statement(c, exit.release())?;
                    let resume = match exit.kind() {
                        SyncExitKind::Follow(n) => Resume::Follow(label(c, n)?),
                        SyncExitKind::Loop {
                            loop_id,
                            kind,
                            target,
                        } => {
                            let at = control(c.loops[loop_id.0 as usize])?;
                            let target = label(c, target)?;
                            match kind {
                                SyncLoopTransfer::Break => Resume::Break {
                                    control: at,
                                    target,
                                },
                                SyncLoopTransfer::Continue => Resume::Continue {
                                    control: at,
                                    target,
                                },
                            }
                        }
                        SyncExitKind::ReturnVoid { terminal: n } => Resume::ReturnVoid {
                            terminal: terminal(c, n),
                        },
                        SyncExitKind::Rethrow {
                            terminal: n,
                            capture,
                            ..
                        } => {
                            let block = terminal(c, n);
                            let TerminalKind::Throw(exception) =
                                c.code.terminals[block.0 as usize].kind
                            else {
                                return Err(bug(
                                    "synchronized rethrow lost its original expression",
                                ));
                            };
                            Resume::Rethrow {
                                terminal: block,
                                exception,
                                capture,
                            }
                        }
                    };
                    // These are the already selected outlet phases. The original
                    // operand read stays between before-operation and snapshot saves.
                    let completion = outlet_completion(c, exit)?;
                    let entry = label(c, c.code.operations[exit.release().0 as usize].node)?;
                    let s = &mut c.code.statements[release.0 as usize];
                    let before_operand = std::mem::take(&mut s.before);
                    let snapshots = std::mem::take(&mut s.snapshots);
                    s.after.clear();
                    for n in exit.routing() {
                        if f.nodes()[n.0 as usize].has_terminal() {
                            let t =
                                &mut c.code.terminals[f.nodes()[n.0 as usize].block().0 as usize];
                            t.before.clear();
                            t.snapshots.clear();
                        }
                    }
                    push(
                        &mut outlets,
                        SyncOutlet {
                            entry,
                            release,
                            transfer: TransferId(exit.release_link().0),
                            destination: label(c, exit.target())?,
                            resume,
                            routing: ids(exit.routing(), |n| n)?,
                            before_operand,
                            snapshots,
                            after_release: ids(exit.after_release(), |p| CopyPlanId(p.0))?,
                            completion,
                        },
                    )?;
                }
                ControlKind::Synchronized {
                    acquire,
                    lock,
                    enter: TransferId(p.enter_link().0),
                    held: ids(p.held_nodes(), |n| n)?,
                    outlets,
                }
            }
            RegionKind::Graph {
                reason,
                entries,
                exits,
            } => {
                let mut ent = Vec::new();
                for &n in entries {
                    push(&mut ent, label(c, n)?)?;
                }
                ControlKind::Graph {
                    reason: *reason,
                    entries: ent,
                    exits: ids(exits, |l| TransferId(l.0))?,
                }
            }
        };
        let decision = if let Some(plan) = r.decision() {
            let mut arms = Vec::new();
            for test in plan.tests() {
                let block = terminal(c, test.predicate());
                let terminal = &c.code.terminals[block.0 as usize];
                let (selector, key_expression) = match terminal.kind {
                    TerminalKind::If {
                        condition: Condition::Zero { value, .. },
                        ..
                    } => (value, None),
                    TerminalKind::If {
                        condition: Condition::Pair { left, right, .. },
                        ..
                    } => {
                        if test.argument() == 0 {
                            (left, Some(right))
                        } else {
                            (right, Some(left))
                        }
                    }
                    _ => return Err(bug("decision test lost its original branch operands")),
                };
                push(
                    &mut arms,
                    DecisionArm {
                        source: ControlId(test.region().0),
                        header: ControlId(test.header().0),
                        terminal: block,
                        predicate: test.predicate(),
                        selector,
                        key_expression,
                        key: test.key(),
                        selected: TransferId(test.selected().0),
                        remaining: TransferId(test.remaining().0),
                        body: ControlId(test.body().0),
                    },
                )?;
            }
            Some(Decision {
                arms,
                default: ControlId(plan.default().0),
            })
        } else {
            None
        };
        let continuation = continuation(c, r.continuation())?;
        let entry = r.entry().map(|n| label(c, n)).transpose()?;
        let follow = r.follow().map(|n| label(c, n)).transpose()?;
        push(
            &mut c.code.controls,
            Control {
                decision,
                id: ControlId(r.id().0),
                kind,
                children: ids(r.children(), |r| ControlId(r.0))?,
                entry,
                follow,
                continuation,
            },
        )?;
    }
    c.code.root = f.root().map(|r| ControlId(r.0));
    Ok(())
}
