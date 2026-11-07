//! Renders one method body from the final emission plan.
//!
//! The event loop below is the only traversal: it walks the selected controls
//! and prints each action at its place. The renderers it calls live in the
//! sibling modules, one per kind of printed element, and every one of them only
//! reads the delivered plan.

mod control;
mod header;
mod statement;
mod terminal;
mod transfer;

use super::expressions::Expressions;
use super::*;
use crate::p2::{BlockId, BranchOp};
use crate::{
    p2::OpId,
    p7::{
        BlockEnd, CheckCarrier, ControlCompletion, ControlId, ControlKind, EmissionLayout, IfShape,
        LabelId, LoopPlacement, PredicateEmission, TerminalKind, TransferId,
    },
};

impl Expressions<'_, '_> {
    /// An explicit work stack, bounded by the delivered tree, prevents hostile
    /// region depth from consuming the native call stack. It does not plan CFGs.
    pub fn body(&mut self) -> Result<(), RenderError> {
        self.locals()?;
        self.exception_chains()?;
        self.bootstrap_definitions()?;
        if !self.code.emission().initial().is_empty() {
            self.out.indent()?;
            self.copies(self.code.emission().initial())?;
            self.out.put("\n")?;
        }
        let mut work = Vec::new();
        if let Some(root) = self.code.root() {
            push(&mut work, Event::Control(root))?;
        }
        while let Some(event) = work.pop() {
            let chained = matches!(event, Event::ChainedControl(_));
            match event {
                Event::Control(id) | Event::ChainedControl(id) => {
                    if self.code.emission().omitted(id) {
                        continue;
                    }
                    let c = &self.code.controls()[id.0 as usize];
                    let p = &self.code.emission().controls()[id.0 as usize];
                    let start = self.out.position();
                    push(&mut work, Event::End(id, start))?;
                    match &p.layout {
                        EmissionLayout::Block(node) => self.block(*node)?,
                        EmissionLayout::Sequence if !p.label_required => {
                            for &child in p.children(c).iter().rev() {
                                push(
                                    &mut work,
                                    if chained {
                                        Event::ChainedControl(child)
                                    } else {
                                        Event::Control(child)
                                    },
                                )?;
                            }
                        }
                        EmissionLayout::Labelled if p.transparent => {
                            for &child in p.children(c).iter().rev() {
                                push(&mut work, Event::Control(child))?;
                            }
                        }
                        EmissionLayout::Sequence
                        | EmissionLayout::Labelled
                        | EmissionLayout::Graph => {
                            let graph = matches!(p.layout, EmissionLayout::Graph);
                            self.out.indent()?;
                            // A control name is printed only when a transfer still
                            // names it; Sequence reaches here only when required.
                            if p.label_required {
                                self.out.put(&p.label)?;
                                // The graph header continues on this line; a plain
                                // block opens immediately after the colon.
                                self.out.put(if graph { ": " } else { ":" })?;
                            }
                            if graph {
                                if let ControlKind::Graph {
                                    reason, entries, ..
                                } = &c.kind
                                {
                                    self.out.fmt(format_args!("graph<{reason:?}> entries ["))?;
                                    for (i, &entry) in entries.iter().enumerate() {
                                        if i > 0 {
                                            self.out.put(", ")?;
                                        }
                                        self.label_name(entry)?;
                                    }
                                    self.out.put("]")?;
                                }
                                if let Some(entry) = c.entry {
                                    self.out.put(" entry ")?;
                                    self.label_name(entry)?;
                                } else {
                                    self.out.put(" no_implicit_entry")?;
                                }
                            }
                            if graph || p.label_required {
                                self.out.open()?;
                            } else {
                                // A bare break still names the innermost printed
                                // scope, so only the unused name is omitted.
                                self.out.open_plain()?;
                            }
                            push(&mut work, Event::Close)?;
                            for &child in p.children(c).iter().rev() {
                                push(&mut work, Event::Control(child))?;
                            }
                        }
                        EmissionLayout::If {
                            header,
                            yes,
                            no,
                            test,
                            shape,
                        } => {
                            if p.label_required {
                                self.out.indent()?;
                                self.out.put(&p.label)?;
                                self.out.put(":")?;
                                self.out.open()?;
                                push(&mut work, Event::Close)?;
                            }
                            match shape {
                                IfShape::Empty => push(&mut work, Event::EmptyIf(*test))?,
                                IfShape::Inverted => {
                                    push(&mut work, Event::Close)?;
                                    push(&mut work, Event::Control(*no))?;
                                    push(
                                        &mut work,
                                        Event::If {
                                            test: *test,
                                            chained,
                                            omitted_false: false,
                                        },
                                    )?;
                                }
                                IfShape::IfElse | IfShape::ElseIf => {
                                    if matches!(shape, IfShape::ElseIf) {
                                        push(&mut work, Event::ChainedControl(*no))?;
                                        push(&mut work, Event::ElseIf(test.when_false))?;
                                    } else {
                                        push(&mut work, Event::Close)?;
                                        if !self.code.emission().omitted(*no) {
                                            push(&mut work, Event::Control(*no))?;
                                            push(&mut work, Event::Else(test.when_false))?;
                                        }
                                    }
                                    push(&mut work, Event::Control(*yes))?;
                                    push(
                                        &mut work,
                                        Event::If {
                                            test: *test,
                                            chained,
                                            omitted_false: self.code.emission().omitted(*no),
                                        },
                                    )?;
                                }
                            }
                            push(&mut work, Event::Control(*header))?;
                        }
                        EmissionLayout::Condition { header, .. } => {
                            push(
                                &mut work,
                                Event::Condition {
                                    control: id,
                                    chained,
                                },
                            )?;
                            push(&mut work, Event::Control(*header))?;
                        }
                        EmissionLayout::Loop { body, placement } => {
                            self.out.indent()?;
                            if p.label_required {
                                self.out.put(&p.label)?;
                                self.out.put(": ")?;
                            }
                            match placement {
                                LoopPlacement::DoWhile {
                                    predicate,
                                    test,
                                    compact,
                                } => {
                                    self.out.put("do")?;
                                    self.out.open()?;
                                    if *compact {
                                        push(&mut work, Event::CompactLoopTail(*predicate, *test))?;
                                    } else {
                                        push(&mut work, Event::LoopTailEnd(*test))?;
                                        push(&mut work, Event::Control(*predicate))?;
                                        push(&mut work, Event::LoopTailStart)?;
                                    }
                                    push(&mut work, Event::Control(*body))?;
                                }
                                LoopPlacement::While {
                                    predicate,
                                    test,
                                    compact,
                                } => {
                                    if *compact {
                                        self.out.put("while (")?;
                                        self.compact_predicate(*predicate, *test)?;
                                        self.out.put(")")?;
                                        self.out.open()?;
                                        push(&mut work, Event::Close)?;
                                        push(&mut work, Event::Control(*body))?;
                                    } else {
                                        self.out.put("while (test")?;
                                        self.out.open()?;
                                        push(&mut work, Event::Close)?;
                                        push(&mut work, Event::Control(*body))?;
                                        push(&mut work, Event::LoopHead(*test))?;
                                        push(&mut work, Event::Control(*predicate))?;
                                    }
                                }
                                LoopPlacement::Infinite => {
                                    self.out.put("while (true)")?;
                                    self.out.open()?;
                                    push(&mut work, Event::Close)?;
                                    push(&mut work, Event::Control(*body))?;
                                }
                            }
                        }
                        EmissionLayout::Switch {
                            header,
                            bodies,
                            private_cases,
                            terminal,
                        } => {
                            push(&mut work, Event::Close)?;
                            if *private_cases {
                                let TerminalKind::Switch { arms, .. } =
                                    &self.code.terminals()[terminal.0 as usize].kind
                                else {
                                    return Err(RenderError::Invariant("switch lost its arms"));
                                };
                                for (&body, &arm) in bodies.iter().zip(arms).rev() {
                                    push(&mut work, Event::Control(body))?;
                                    push(&mut work, Event::Case(arm))?;
                                }
                            } else {
                                for &body in bodies.iter().rev() {
                                    push(&mut work, Event::Control(body))?;
                                }
                            }
                            push(&mut work, Event::Switch(id, *terminal))?;
                            push(&mut work, Event::Control(*header))?;
                        }
                        EmissionLayout::Try { body, catches } => {
                            self.out.indent()?;
                            if p.label_required {
                                self.out.put(&p.label)?;
                                self.out.put(": ")?;
                            }
                            self.out.put("try")?;
                            self.out.open()?;
                            push(&mut work, Event::Close)?;
                            for (i, &handler) in catches.iter().enumerate().rev() {
                                push(&mut work, Event::Control(handler))?;
                                push(&mut work, Event::Catch(id, i))?;
                            }
                            push(&mut work, Event::Control(*body))?;
                        }
                        EmissionLayout::Synchronized {
                            entry,
                            body,
                            acquire,
                        } => {
                            push(&mut work, Event::Close)?;
                            push(&mut work, Event::Control(*body))?;
                            push(&mut work, Event::Acquire(id, *acquire))?;
                            push(&mut work, Event::Control(*entry))?;
                        }
                    }
                }
                Event::End(id, start) => {
                    if let Some(completion) =
                        self.code.emission().controls()[id.0 as usize].continuation
                    {
                        self.out.indent()?;
                        match completion {
                            ControlCompletion::Goto(label) => {
                                self.out.put("goto ")?;
                                self.label_name(label)?;
                                self.out.put("; // planned continuation\n")?;
                            }
                            ControlCompletion::Break(scope)
                            | ControlCompletion::Continue(scope) => {
                                self.out.put(
                                    if matches!(completion, ControlCompletion::Break(_)) {
                                        "break "
                                    } else {
                                        "continue "
                                    },
                                )?;
                                self.out.put(
                                    &self.code.emission().controls()[scope.0 as usize].label,
                                )?;
                                self.out.put(";\n")?;
                            }
                            ControlCompletion::BreakHere(_) => self.out.put("break;\n")?,
                            ControlCompletion::ContinueHere(_) => self.out.put("continue;\n")?,
                        }
                    }
                    if self.code.emission().deferred_control(id) {
                        let slot = self
                            .pending_controls
                            .as_deref_mut()
                            .and_then(|pending| pending.get_mut(id.0 as usize))
                            .ok_or(RenderError::Invariant("deferred control is unavailable"))?;
                        if slot.replace(start).is_some() {
                            return Err(RenderError::Invariant("control was deferred twice"));
                        }
                    } else {
                        self.out.span(
                            start,
                            Origin::Control {
                                method: self.method,
                                control: id,
                            },
                        )?;
                    }
                }
                Event::Close => self.out.close()?,
                Event::Condition { control, chained } => {
                    let (count, yes, no, fall) =
                        match &self.code.emission().controls()[control.0 as usize].layout {
                            EmissionLayout::Condition {
                                tests,
                                yes,
                                no,
                                fall,
                                ..
                            } => (tests.len(), *yes, *no, *fall),
                            _ => {
                                return Err(RenderError::Invariant(
                                    "condition control lost its delivered layout",
                                ));
                            }
                        };
                    if !chained {
                        self.out.indent()?;
                    }
                    self.out.put("if (")?;
                    for i in 0..count {
                        if i > 0 {
                            self.out.put(" || ")?;
                        }
                        let test = match &self.code.emission().controls()[control.0 as usize].layout
                        {
                            EmissionLayout::Condition { tests, .. } => tests[i],
                            _ => {
                                return Err(RenderError::Invariant(
                                    "condition control lost its delivered layout",
                                ));
                            }
                        };
                        let start = self.out.position();
                        self.predicate_expression(test, false)?;
                        self.out.span(
                            start,
                            Origin::Terminal {
                                method: self.method,
                                block: test.terminal,
                            },
                        )?;
                        // Both original test edges keep their identity at the
                        // test's own evaluation position. The final other edge
                        // is already the printed `else` route when that arm
                        // exists; anchoring it twice would duplicate a source
                        // span for one original transfer.
                        let (select, other) = if test.negate {
                            (test.when_false, test.when_true)
                        } else {
                            (test.when_true, test.when_false)
                        };
                        self.selected(select, false)?;
                        if i + 1 < count || no.is_none() {
                            self.selected(other, false)?;
                        }
                    }
                    self.out.put(")")?;
                    self.out.open()?;
                    push(&mut work, Event::Close)?;
                    // Later test runs print no words of their own. Their
                    // delivered controls, labels, routes and copies keep
                    // zero-width anchors at the shared arm.
                    let run_count =
                        count.min(self.code.controls()[control.0 as usize].children.len());
                    for i in (1..run_count).rev() {
                        let run = self.code.controls()[control.0 as usize].children[i];
                        push(&mut work, Event::Anchors(run))?;
                    }
                    if let Some(no) = no {
                        push(&mut work, Event::Control(no))?;
                        push(&mut work, Event::Else(fall))?;
                    }
                    push(&mut work, Event::Control(yes))?;
                }
                Event::Anchors(control) => {
                    let start = self.out.position();
                    self.out.span(
                        start,
                        Origin::Control {
                            method: self.method,
                            control,
                        },
                    )?;
                    let node = match self.code.emission().controls()[control.0 as usize].layout {
                        EmissionLayout::Block(node) => Some(node),
                        _ => None,
                    };
                    if let Some(node) = node {
                        let start = self.out.position();
                        self.out.span(
                            start,
                            Origin::Label {
                                method: self.method,
                                label: LabelId(node.0),
                            },
                        )?;
                        let transfers = match &self.code.emission().blocks()[node.0 as usize].end {
                            BlockEnd::Transfers(links) => links.len(),
                            _ => 0,
                        };
                        for at in 0..transfers {
                            let link = match &self.code.emission().blocks()[node.0 as usize].end {
                                BlockEnd::Transfers(links) => links[at],
                                _ => {
                                    return Err(RenderError::Invariant(
                                        "anchored block lost its transfer",
                                    ));
                                }
                            };
                            let start = self.out.position();
                            let copies = self.code.emission().transfers()[link.0 as usize]
                                .copies
                                .len();
                            for index in 0..copies {
                                let copy =
                                    self.code.emission().transfers()[link.0 as usize].copies[index];
                                self.copies(std::slice::from_ref(&copy))?;
                            }
                            self.out.span(
                                start,
                                Origin::Transfer {
                                    method: self.method,
                                    transfer: link,
                                },
                            )?;
                        }
                        let terminal = match &self.code.emission().blocks()[node.0 as usize].end {
                            BlockEnd::Terminal(terminal) => Some(*terminal),
                            _ => None,
                        };
                        if let Some(terminal) = terminal {
                            let start = self.out.position();
                            self.out.span(
                                start,
                                Origin::Terminal {
                                    method: self.method,
                                    block: terminal,
                                },
                            )?;
                        }
                    } else {
                        let children = self.code.emission().controls()[control.0 as usize]
                            .children(&self.code.controls()[control.0 as usize]);
                        for &child in children.iter().rev() {
                            push(&mut work, Event::Anchors(child))?;
                        }
                    }
                }
                Event::EmptyIf(test) => {
                    let start = self.out.position();
                    self.selected(test.when_true, false)?;
                    self.selected(test.when_false, false)?;
                    self.out.span(
                        start,
                        Origin::Terminal {
                            method: self.method,
                            block: test.terminal,
                        },
                    )?;
                }
                Event::If {
                    test,
                    chained,
                    omitted_false,
                } => {
                    if !chained {
                        self.out.indent()?;
                    }
                    self.copies(
                        &self.code.emission().terminals()[test.terminal.0 as usize].before,
                    )?;
                    let a = self.out.position();
                    self.out.put("if (")?;
                    let previous = self
                        .check_carrier
                        .replace(CheckCarrier::Terminal(test.terminal));
                    let rendered = self.predicate_expression(test, false);
                    self.check_carrier = previous;
                    rendered?;
                    self.out.put(")")?;
                    self.out.span(
                        a,
                        Origin::Terminal {
                            method: self.method,
                            block: test.terminal,
                        },
                    )?;
                    if omitted_false {
                        self.selected(test.when_false, false)?;
                    }
                    if test.negate {
                        // P7 admits only nonprinting true-edge bundles here;
                        // their plan IDs retain zero-length original origins.
                        self.selected(test.when_true, false)?;
                    }
                    self.out.open()?;
                    let edge = if test.negate {
                        test.when_false
                    } else {
                        test.when_true
                    };
                    if !self.code.emission().transfers()[edge.0 as usize]
                        .copies
                        .is_empty()
                    {
                        self.out.indent()?;
                        self.selected(edge, false)?;
                        self.out.put("\n")?;
                    } else {
                        self.selected(edge, false)?;
                    }
                }
                Event::ElseIf(edge) => {
                    self.out.close()?;
                    self.out.indent()?;
                    self.out.put("else ")?;
                    self.selected(edge, false)?;
                }
                Event::Else(edge) => {
                    self.out.close()?;
                    self.out.indent()?;
                    self.out.put("else")?;
                    self.out.open()?;
                    if !self.code.emission().transfers()[edge.0 as usize]
                        .copies
                        .is_empty()
                    {
                        self.out.indent()?;
                        self.selected(edge, false)?;
                        self.out.put("\n")?;
                    } else {
                        self.selected(edge, false)?;
                    }
                }
                Event::LoopHead(test) => {
                    self.out.indent()?;
                    self.out.put("yield ")?;
                    self.predicate(test)?;
                    self.out.put(";\n")?;
                    self.out.close()?;
                    self.out.indent()?;
                    self.out.put(")")?;
                    self.out.open()?;
                }
                Event::CompactLoopTail(predicate, test) => {
                    self.out.close()?;
                    self.out.indent()?;
                    self.out.put("while (")?;
                    self.compact_predicate(predicate, test)?;
                    self.out.put(");\n")?;
                }
                Event::LoopTailStart => {
                    self.out.close()?;
                    self.out.indent()?;
                    self.out.put("while (test")?;
                    self.out.open()?;
                }
                Event::LoopTailEnd(test) => {
                    self.out.indent()?;
                    self.out.put("yield ")?;
                    self.predicate(test)?;
                    self.out.put(";\n")?;
                    self.out.close()?;
                    self.out.line(");")?;
                }
                Event::Switch(id, terminal) => {
                    let t = &self.code.terminals()[terminal.0 as usize];
                    let TerminalKind::Switch { selector, arms } = &t.kind else {
                        return Err(RenderError::Invariant("switch lost its selector"));
                    };
                    self.out.indent()?;
                    self.copies(&self.code.emission().terminals()[terminal.0 as usize].before)?;
                    let a = self.out.position();
                    if self.code.emission().controls()[id.0 as usize].label_required {
                        self.out
                            .put(&self.code.emission().controls()[id.0 as usize].label)?;
                        self.out.put(": ")?;
                    }
                    self.out.put("switch (")?;
                    self.operand(*selector)?;
                    self.phases(
                        &self.code.emission().terminals()[terminal.0 as usize].snapshots,
                        "after_operands_before_effect",
                    )?;
                    self.out.put(")")?;
                    self.out.span(
                        a,
                        Origin::Terminal {
                            method: self.method,
                            block: terminal,
                        },
                    )?;
                    self.out.open()?;
                    if !matches!(
                        self.code.emission().controls()[id.0 as usize].layout,
                        EmissionLayout::Switch {
                            private_cases: true,
                            ..
                        }
                    ) {
                        self.switch_arms(arms)?;
                    }
                }
                Event::Case(arm) => {
                    self.out.indent()?;
                    match self.code.transfers()[arm.0 as usize].role {
                        Some(crate::p2::NormalEdge::SwitchCase { key, .. }) => {
                            self.out.fmt(format_args!("case {key}:"))?
                        }
                        Some(crate::p2::NormalEdge::SwitchDefault) => self.out.put("default:")?,
                        _ => {
                            return Err(RenderError::Invariant(
                                "switch arm lost its original role",
                            ));
                        }
                    }
                    if !self.code.emission().transfers()[arm.0 as usize]
                        .copies
                        .is_empty()
                    {
                        self.out.put(" ")?;
                    }
                    self.selected(arm, false)?;
                    self.out.put("\n")?;
                }
                Event::Catch(id, index) => {
                    self.out.close()?;
                    self.out.indent()?;
                    self.out.put("catch (")?;
                    let ControlKind::Try { chain, clauses, .. } =
                        &self.code.controls()[id.0 as usize].kind
                    else {
                        return Err(RenderError::Invariant("catch lost its try"));
                    };
                    let clause = &self.code.chains()[chain.0 as usize].clauses
                        [clauses.start as usize + index];
                    // The handler block follows immediately, so the clause
                    // prints only the type it catches; the route target is the
                    // enclosing catch itself and needs no second mention.
                    self.clause_type(clause)?;
                    self.out.put(")")?;
                    self.out.open()?;
                }
                Event::Acquire(id, operation) => {
                    let source = &self.code.operations()[operation.0 as usize];
                    let statement = source
                        .statement
                        .ok_or(RenderError::Invariant("acquire lost its statement"))?;
                    let ControlKind::Synchronized { lock, enter, .. } =
                        &self.code.controls()[id.0 as usize].kind
                    else {
                        return Err(RenderError::Invariant("acquire lost its scope"));
                    };
                    let phase = &self.code.emission().operations()[operation.0 as usize];
                    let control = &self.code.emission().controls()[id.0 as usize];
                    self.out.indent()?;
                    self.copies(&phase.before)?;
                    let a = self.out.position();
                    if control.label_required {
                        self.out.put(&control.label)?;
                        self.out.put(": ")?;
                    }
                    self.out.put("synchronized (")?;
                    self.operand(*lock)?;
                    self.out.put(")")?;
                    self.phases(&phase.snapshots, "after_operands_before_effect")?;
                    self.sites(&self.code.statements()[statement.0 as usize].sites, a)?;
                    self.out.span(
                        a,
                        Origin::Operation {
                            method: self.method,
                            operation,
                            pc: source.pc,
                        },
                    )?;
                    self.out.span(
                        a,
                        Origin::Statement {
                            method: self.method,
                            statement,
                        },
                    )?;
                    // The closing brace is not a release: the delivered
                    // `release synchronized` statements are the only releases.
                    self.out.open_note("// Diana: releases are explicit")?;
                    if !phase.after.is_empty()
                        || !self.code.emission().transfers()[enter.0 as usize]
                            .copies
                            .is_empty()
                    {
                        self.out.indent()?;
                        self.copies(&phase.after)?;
                        self.selected(*enter, false)?;
                        self.out.put("\n")?;
                    } else {
                        self.selected(*enter, false)?;
                    }
                }
            }
        }
        Ok(())
    }
}

pub(super) fn complement(op: BranchOp) -> BranchOp {
    match op {
        BranchOp::Eq => BranchOp::Ne,
        BranchOp::Ne => BranchOp::Eq,
        BranchOp::Lt => BranchOp::Ge,
        BranchOp::Ge => BranchOp::Lt,
        BranchOp::Gt => BranchOp::Le,
        BranchOp::Le => BranchOp::Gt,
        BranchOp::Eqz => BranchOp::Nez,
        BranchOp::Nez => BranchOp::Eqz,
        BranchOp::Ltz => BranchOp::Gez,
        BranchOp::Gez => BranchOp::Ltz,
        BranchOp::Gtz => BranchOp::Lez,
        BranchOp::Lez => BranchOp::Gtz,
    }
}
pub(super) fn branch(op: BranchOp) -> &'static str {
    match op {
        BranchOp::Eq | BranchOp::Eqz => " == ",
        BranchOp::Ne | BranchOp::Nez => " != ",
        BranchOp::Lt | BranchOp::Ltz => " < ",
        BranchOp::Ge | BranchOp::Gez => " >= ",
        BranchOp::Gt | BranchOp::Gtz => " > ",
        BranchOp::Le | BranchOp::Lez => " <= ",
    }
}
enum Event {
    Control(ControlId),
    ChainedControl(ControlId),
    End(ControlId, TextOffset),
    Close,
    If {
        test: PredicateEmission,
        chained: bool,
        omitted_false: bool,
    },
    EmptyIf(PredicateEmission),
    Condition {
        control: ControlId,
        chained: bool,
    },
    /// A delivered control subtree that prints no text still anchors its
    /// original controls, labels, transfers and copies.
    Anchors(ControlId),
    Else(TransferId),
    ElseIf(TransferId),
    LoopHead(PredicateEmission),
    CompactLoopTail(ControlId, PredicateEmission),
    LoopTailStart,
    LoopTailEnd(PredicateEmission),
    Switch(ControlId, BlockId),
    Case(TransferId),
    Catch(ControlId, usize),
    Acquire(ControlId, OpId),
}
pub(super) fn push<T>(work: &mut Vec<T>, event: T) -> Result<(), RenderError> {
    work.try_reserve(1).map_err(RenderError::MethodScratch)?;
    work.push(event);
    Ok(())
}
