//! Terminals: returns, throws, branch predicates and switches.

use super::super::expressions::Expressions;
use super::super::{Origin, RenderError};
use super::{branch, complement, push};
use crate::p2::{BlockId, BranchOp};
use crate::p7::*;

impl Expressions<'_, '_> {
    pub(super) fn terminal(&mut self, id: BlockId) -> Result<(), RenderError> {
        // The closing brace of the method body performs a void return that
        // prints nothing else, so P7 omits that redundant `return;`. The original
        // terminal keeps a zero-length anchor where its text would have stood.
        if self.code.emission().omitted_return(id) {
            let position = self.out.position();
            return self.out.span(
                position,
                Origin::Terminal {
                    method: self.method,
                    block: id,
                },
            );
        }
        if self.code.emission().empty_if(id) {
            let TerminalKind::If {
                when_true,
                when_false,
                ..
            } = self.code.terminals()[id.0 as usize].kind
            else {
                return Err(RenderError::Invariant("empty If lost its original edges"));
            };
            let start = self.out.position();
            self.selected(when_true, false)?;
            self.selected(when_false, false)?;
            return self.out.span(
                start,
                Origin::Terminal {
                    method: self.method,
                    block: id,
                },
            );
        }
        let t = &self.code.terminals()[id.0 as usize];
        let p = &self.code.emission().terminals()[id.0 as usize];
        // A terminal that prints no words keeps zero-width anchors for itself
        // and for its delivered route, including empty copy bundles. It must
        // not leave indentation behind for the next statement.
        let silent = p.before.is_empty()
            && p.snapshots.is_empty()
            && match &t.kind {
                TerminalKind::None => true,
                TerminalKind::Goto(transfer) => self.transfer_prints_no_words(*transfer),
                _ => false,
            };
        if silent {
            let start = self.out.position();
            if let TerminalKind::Goto(transfer) = &t.kind {
                let copies = self.code.emission().transfers()[transfer.0 as usize]
                    .copies
                    .len();
                for index in 0..copies {
                    let copy = self.code.emission().transfers()[transfer.0 as usize].copies[index];
                    self.copies(std::slice::from_ref(&copy))?;
                }
                self.out.span(
                    start,
                    Origin::Transfer {
                        method: self.method,
                        transfer: *transfer,
                    },
                )?;
            }
            return self.out.span(
                start,
                Origin::Terminal {
                    method: self.method,
                    block: id,
                },
            );
        }
        self.out.indent()?;
        self.copies(&p.before)?;
        if !p.before.is_empty() {
            self.out.put(" ")?;
        }
        let start = self.out.position();
        match &t.kind {
            TerminalKind::None => {}
            TerminalKind::Goto(t) => self.transfer_with_indent(*t, false)?,
            TerminalKind::Return(v) => {
                self.out.put("return")?;
                if let Some(v) = v {
                    self.out.put(" ")?;
                    self.carried(id, |render| render.operand(*v))?;
                }
                self.phases(&p.snapshots, "after_operands_before_effect")?;
                if let Some(s) = t.site {
                    self.sites(&[s], start)?;
                }
                self.out.put(";\n")?;
            }
            TerminalKind::Throw(v) => {
                self.out.put("throw ")?;
                self.carried(id, |render| render.operand(*v))?;
                self.phases(&p.snapshots, "after_operands_before_effect")?;
                if let Some(s) = t.site {
                    self.sites(&[s], start)?;
                }
                self.out.put(";\n")?;
            }
            TerminalKind::If {
                condition,
                when_true,
                when_false,
            } => {
                // The predicate evaluates its operands before selection, so the
                // terminal's own carrier wraps only that evaluation.
                let previous = self.check_carrier.replace(CheckCarrier::Terminal(id));
                self.out.put("if (")?;
                let rendered = self.condition(condition, false);
                let rendered = rendered
                    .and_then(|()| self.phases(&p.snapshots, "after_operands_before_effect"));
                self.check_carrier = previous;
                rendered?;
                self.out.put(")")?;
                self.out.open()?;
                self.branch_transfer(*when_true, true)?;
                self.out.close()?;
                if !self.code.emission().empty_else(id) {
                    self.out.indent()?;
                    self.out.put("else")?;
                    self.out.open()?;
                    self.branch_transfer(*when_false, true)?;
                    self.out.close()?;
                } else {
                    self.selected(*when_false, false)?;
                }
            }
            TerminalKind::Switch { selector, arms } => {
                self.switch(*selector, arms, &p.snapshots)?
            }
        }
        self.out.span(
            start,
            Origin::Terminal {
                method: self.method,
                block: id,
            },
        )
    }

    pub(super) fn condition(&mut self, c: &Condition, negate: bool) -> Result<(), RenderError> {
        let op_of = |op: BranchOp| if negate { complement(op) } else { op };
        match c {
            Condition::Pair { op, left, right } => {
                self.operand(*left)?;
                self.out.put(branch(op_of(*op)))?;
                self.operand(*right)
            }
            Condition::Zero { op, value, meaning } => {
                if self
                    .code
                    .emission()
                    .composed_use(*value)
                    .is_some_and(|value| value.boolean)
                {
                    if !matches!(meaning, ZeroMeaning::IntegralZero) {
                        return Err(RenderError::Invariant(
                            "composed boolean lost its zero interpretation",
                        ));
                    }
                    match op_of(*op) {
                        BranchOp::Eqz => self.out.put("!")?,
                        BranchOp::Nez => {}
                        _ => {
                            return Err(RenderError::Invariant(
                                "composed boolean lost its branch role",
                            ));
                        }
                    }
                    return self.operand(*value);
                }
                self.operand(*value)?;
                self.out.put(branch(op_of(*op)))?;
                self.out.put(match meaning {
                    ZeroMeaning::Null => "null",
                    ZeroMeaning::IntegralZero => "0",
                    ZeroMeaning::BitsZero => "zero_bits",
                    ZeroMeaning::Unavailable => "unavailable_zero_interpretation",
                })
            }
        }
    }

    pub(super) fn predicate_expression(
        &mut self,
        test: PredicateEmission,
        loop_edges: bool,
    ) -> Result<(), RenderError> {
        let t = &self.code.terminals()[test.terminal.0 as usize];
        let p = &self.code.emission().terminals()[test.terminal.0 as usize];
        let TerminalKind::If { condition, .. } = &t.kind else {
            return Err(RenderError::Invariant(
                "predicate lost its original condition",
            ));
        };
        // Negating the delivered boolean zero-test cancels its exact Eqz
        // negation; it does not change branch identities or selected-edge phases.
        let positive_boolean = match condition {
            Condition::Zero {
                op: BranchOp::Eqz,
                value,
                meaning: ZeroMeaning::IntegralZero,
            } if test.negate
                && self
                    .code
                    .emission()
                    .composed_use(*value)
                    .is_some_and(|value| value.boolean) =>
            {
                Some(*value)
            }
            _ => None,
        };
        if let Some(value) = positive_boolean {
            self.operand(value)?;
        } else {
            self.condition(condition, test.negate)?;
        }
        self.phases(&p.snapshots, "after_operands_before_effect")?;
        // Copies belong to one selected original edge. Naming the looping role
        // is shorter than the truth value and keeps the same ownership.
        let (repeat, exit) = if test.negate {
            (test.when_false, test.when_true)
        } else {
            (test.when_true, test.when_false)
        };
        if loop_edges {
            // Each role is one selected original edge, and only its copies
            // print. `selected` keeps the transfer origin on those copies and
            // anchors an edge with nothing to print where it was selected.
            for (role, edge) in [("repeat", repeat), ("exit", exit)] {
                let prints = !self.code.emission().transfers()[edge.0 as usize]
                    .copies
                    .is_empty();
                if prints {
                    self.out.fmt(format_args!(" [{role}: "))?;
                }
                self.selected(edge, false)?;
                if prints {
                    self.out.put("]")?;
                }
            }
        }
        Ok(())
    }

    pub(super) fn predicate(&mut self, test: PredicateEmission) -> Result<(), RenderError> {
        let start = self.out.position();
        self.copies(&self.code.emission().terminals()[test.terminal.0 as usize].before)?;
        let previous = self
            .check_carrier
            .replace(CheckCarrier::Terminal(test.terminal));
        let rendered = self.predicate_expression(test, true);
        self.check_carrier = previous;
        rendered?;
        self.out.span(
            start,
            Origin::Terminal {
                method: self.method,
                block: test.terminal,
            },
        )
    }

    pub(super) fn compact_predicate(
        &mut self,
        control: ControlId,
        test: PredicateEmission,
    ) -> Result<(), RenderError> {
        let start = self.out.position();
        let mut stack = Vec::new();
        let mut controls = Vec::new();
        push(&mut stack, control)?;
        while let Some(id) = stack.pop() {
            push(&mut controls, id)?;
            match self.code.emission().controls()[id.0 as usize].layout {
                EmissionLayout::Block(node) => self.block(node)?,
                EmissionLayout::Sequence => {
                    for &child in self.code.emission().controls()[id.0 as usize]
                        .children(&self.code.controls()[id.0 as usize])
                        .iter()
                        .rev()
                    {
                        push(&mut stack, child)?;
                    }
                }
                _ => {
                    return Err(RenderError::Invariant(
                        "compact predicate lost its evaluation",
                    ));
                }
            }
            if self.code.emission().deferred_control(id) {
                let slot = self
                    .pending_controls
                    .as_deref_mut()
                    .and_then(|pending| pending.get_mut(id.0 as usize))
                    .ok_or(RenderError::Invariant(
                        "deferred predicate control is unavailable",
                    ))?;
                if slot.replace(start).is_some() {
                    return Err(RenderError::Invariant(
                        "predicate control was deferred twice",
                    ));
                }
            }
        }
        self.predicate(test)?;
        for id in controls {
            if !self.code.emission().deferred_control(id) {
                self.out.span(
                    start,
                    Origin::Control {
                        method: self.method,
                        control: id,
                    },
                )?;
            }
        }
        Ok(())
    }

    pub(super) fn switch(
        &mut self,
        selector: ExprId,
        arms: &[TransferId],
        snapshots: &[CopyPlanId],
    ) -> Result<(), RenderError> {
        self.out.put("switch (")?;
        self.operand(selector)?;
        self.phases(snapshots, "after_operands_before_effect")?;
        self.out.put(")")?;
        self.out.open()?;
        self.switch_arms(arms)?;
        self.out.close()
    }

    pub(super) fn switch_arms(&mut self, arms: &[TransferId]) -> Result<(), RenderError> {
        for &arm in arms {
            self.out.indent()?;
            match self.code.transfers()[arm.0 as usize].role {
                Some(crate::p2::NormalEdge::SwitchCase { key, .. }) => {
                    self.out.fmt(format_args!("case {key}:"))?
                }
                Some(crate::p2::NormalEdge::SwitchDefault) => self.out.put("default:")?,
                _ => return Err(RenderError::Invariant("switch arm lost its original role")),
            }
            let silent = self.transfer_prints_nothing(arm);
            if !silent {
                // The arm label already indented this line; only the transfer text follows.
                self.out.put(" ")?;
            }
            self.branch_transfer(arm, false)?;
            if silent {
                // A route that prints no words still ends its own line: the arm
                // falls through to the next case instead of continuing this one.
                self.out.put("\n")?;
            }
        }
        Ok(())
    }

    /// Renders one terminal operand under this terminal's check carrier, so a
    /// check-cast prints at its exact read and the previous carrier is restored
    /// even when rendering fails.
    pub(super) fn carried(
        &mut self,
        terminal: BlockId,
        render: impl FnOnce(&mut Self) -> Result<(), RenderError>,
    ) -> Result<(), RenderError> {
        let previous = self.check_carrier.replace(CheckCarrier::Terminal(terminal));
        let rendered = render(self);
        self.check_carrier = previous;
        rendered
    }
}
