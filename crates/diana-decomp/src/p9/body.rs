use super::{Origin, RenderError, TextOffset, expressions::Expressions};
use crate::{
    p2::{BlockId, BranchOp, OpId},
    p5::Materialization,
    p7::*,
};

impl Expressions<'_, '_> {
    pub fn statement(&mut self, op: OpId, release: Option<ControlId>) -> Result<(), RenderError> {
        if self.code.emission().composed_operation(op) {
            return Ok(());
        }
        let original = &self.code.operations()[op.0 as usize];
        let id = original.statement.ok_or(RenderError::Invariant(
            "emission operation has no statement",
        ))?;
        let s = &self.code.statements()[id.0 as usize];
        let phases = &self.code.emission().operations()[op.0 as usize];
        self.out.indent()?;
        self.copies(&phases.before)?;
        let start = self.out.position();
        if matches!(s.kind, StatementKind::Construction(_)) {
            self.copies(&phases.snapshots)?;
        }
        if let Some(scope) = release {
            self.out.put("release synchronized ")?;
            self.out
                .put(&self.code.emission().controls()[scope.0 as usize].label)?;
            self.out.put("(")?;
            let StatementKind::Monitor { lock, .. } = s.kind else {
                return Err(RenderError::Invariant("release syntax lost its monitor"));
            };
            self.operand(lock)?;
            self.out.put(")")?;
        } else {
            self.assignment(s)?;
            self.semantic(s)?;
        }
        if !matches!(s.kind, StatementKind::Construction(_)) {
            self.phases(&phases.snapshots, "after_operands_before_effect")?;
        }
        self.sites(&s.sites, start)?;
        self.out.put(";")?;
        for definition in &s.outputs {
            if let Materialization::Unavailable(gap) = definition.materialization() {
                self.out
                    .fmt(format_args!(" unavailable_definition<{gap:?}>;"))?;
            }
        }
        self.out.span(
            start,
            Origin::Statement {
                method: self.method,
                statement: id,
            },
        )?;
        if let StatementKind::Construction(g) = s.kind {
            for &member in &self.code.constructions()[g.0 as usize].members {
                let source = &self.code.operations()[member.0 as usize];
                self.out.span(
                    start,
                    Origin::Operation {
                        method: self.method,
                        operation: member,
                        pc: source.pc,
                    },
                )?;
            }
        } else {
            self.out.span(
                start,
                Origin::Operation {
                    method: self.method,
                    operation: op,
                    pc: original.pc,
                },
            )?;
        }
        if let Some(pc) = original.result_pc {
            self.out.span(
                start,
                Origin::ResultCommit {
                    method: self.method,
                    operation: op,
                    pc,
                },
            )?;
        }
        if !phases.after.is_empty() {
            self.out.put(" ")?;
            self.copies(&phases.after)?;
        }
        self.out.put("\n")
    }
    fn sites(&mut self, sites: &[SiteId], statement: TextOffset) -> Result<(), RenderError> {
        for &id in sites {
            let s = &self.code.sites()[id.0 as usize];
            let origin = Origin::Site {
                method: self.method,
                site: id,
            };
            if !self.code.emission().site_annotation(id) {
                // The route is already printed as the enclosing catch scope.
                self.out.span(statement, origin)?;
                continue;
            }
            let start = self.out.position();
            // The printed chain line carries clause order, catch types and
            // handler targets; the site only names the chain it selects.
            self.out.fmt(format_args!(" [exn c{}]", s.chain.0))?;
            self.out.span(start, origin)?;
        }
        Ok(())
    }
    fn condition(&mut self, c: &Condition) -> Result<(), RenderError> {
        match c {
            Condition::Pair { op, left, right } => {
                self.operand(*left)?;
                self.out.put(branch(*op))?;
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
                    match op {
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
                self.out.put(branch(*op))?;
                self.out.put(match meaning {
                    ZeroMeaning::Null => "null",
                    ZeroMeaning::IntegralZero => "0",
                    ZeroMeaning::BitsZero => "zero_bits",
                    ZeroMeaning::Unavailable => "unavailable_zero_interpretation",
                })
            }
        }
    }
    fn selected(&mut self, id: TransferId, jump: bool) -> Result<(), RenderError> {
        let start = self.out.position();
        self.copies(&self.code.emission().transfers()[id.0 as usize].copies)?;
        if jump {
            self.out.put("goto ")?;
            self.label_name(self.code.transfers()[id.0 as usize].destination)?;
            self.out.put("; ")?;
        }
        self.out.span(
            start,
            Origin::Transfer {
                method: self.method,
                transfer: id,
            },
        )
    }
    fn transfer(&mut self, id: TransferId) -> Result<(), RenderError> {
        self.transfer_with_indent(id, true)
    }
    fn transfer_with_indent(&mut self, id: TransferId, indent: bool) -> Result<(), RenderError> {
        let p = &self.code.emission().transfers()[id.0 as usize];
        let start = self.out.position();
        if matches!(p.spelling, TransferSpelling::Fallthrough) && p.copies.is_empty() {
            return self.out.span(
                start,
                Origin::Transfer {
                    method: self.method,
                    transfer: id,
                },
            );
        }
        if indent {
            self.out.indent()?;
        }
        self.copies(&p.copies)?;
        match p.spelling {
            TransferSpelling::Goto(label) => {
                self.out.put("goto ")?;
                self.label_name(label)?;
                self.out.put(";")?;
            }
            TransferSpelling::Break(c) | TransferSpelling::Continue(c) => {
                self.out
                    .put(if matches!(p.spelling, TransferSpelling::Break(_)) {
                        "break "
                    } else {
                        "continue "
                    })?;
                self.out
                    .put(&self.code.emission().controls()[c.0 as usize].label)?;
                self.out.put(";")?;
            }
            TransferSpelling::Fallthrough => {}
            TransferSpelling::Selected => {
                return Err(RenderError::Invariant(
                    "selected edge reached an ordinary transfer position",
                ));
            }
        }
        self.out.span(
            start,
            Origin::Transfer {
                method: self.method,
                transfer: id,
            },
        )?;
        self.out.put("\n")
    }
    fn label_name(&mut self, id: LabelId) -> Result<(), RenderError> {
        self.out.put(&self.code.labels()[id.0 as usize].name)
    }
    fn label(&mut self, id: LabelId) -> Result<(), RenderError> {
        let start = self.out.position();
        if self.code.emission().blocks()[id.0 as usize].show_label {
            self.out.indent()?;
            self.label_name(id)?;
            self.out.put(":\n")?;
        }
        self.out.span(
            start,
            Origin::Label {
                method: self.method,
                label: id,
            },
        )
    }
    fn terminal(&mut self, id: BlockId) -> Result<(), RenderError> {
        let t = &self.code.terminals()[id.0 as usize];
        let p = &self.code.emission().terminals()[id.0 as usize];
        self.out.indent()?;
        self.copies(&p.before)?;
        let start = self.out.position();
        match &t.kind {
            TerminalKind::None => {}
            TerminalKind::Goto(t) => self.transfer_with_indent(*t, false)?,
            TerminalKind::Return(v) => {
                self.out.put("return")?;
                if let Some(v) = v {
                    self.out.put(" ")?;
                    self.operand(*v)?;
                }
                self.phases(&p.snapshots, "after_operands_before_effect")?;
                if let Some(s) = t.site {
                    self.sites(&[s], start)?;
                }
                self.out.put(";\n")?;
            }
            TerminalKind::Throw(v) => {
                self.out.put("throw ")?;
                self.operand(*v)?;
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
                self.out.put("if (")?;
                self.condition(condition)?;
                self.phases(&p.snapshots, "after_operands_before_effect")?;
                self.out.put(")")?;
                self.out.open()?;
                self.branch_transfer(*when_true)?;
                self.out.close()?;
                self.out.indent()?;
                self.out.put("else")?;
                self.out.open()?;
                self.branch_transfer(*when_false)?;
                self.out.close()?;
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
    fn branch_transfer(&mut self, id: TransferId) -> Result<(), RenderError> {
        if matches!(
            self.code.emission().transfers()[id.0 as usize].spelling,
            TransferSpelling::Selected
        ) {
            self.out.indent()?;
            self.selected(id, true)?;
            self.out.put("\n")
        } else {
            self.transfer(id)
        }
    }
    fn switch(
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
    fn switch_arms(&mut self, arms: &[TransferId]) -> Result<(), RenderError> {
        for &arm in arms {
            self.out.indent()?;
            match self.code.transfers()[arm.0 as usize].role {
                Some(crate::p2::NormalEdge::SwitchCase { key, .. }) => {
                    self.out.fmt(format_args!("case {key}: "))?
                }
                Some(crate::p2::NormalEdge::SwitchDefault) => self.out.put("default: ")?,
                _ => return Err(RenderError::Invariant("switch arm lost its original role")),
            }
            self.branch_transfer(arm)?;
        }
        Ok(())
    }
    fn predicate(&mut self, test: PredicateEmission) -> Result<(), RenderError> {
        let t = &self.code.terminals()[test.terminal.0 as usize];
        let p = &self.code.emission().terminals()[test.terminal.0 as usize];
        let TerminalKind::If { condition, .. } = &t.kind else {
            return Err(RenderError::Invariant(
                "predicate lost its original condition",
            ));
        };
        let start = self.out.position();
        self.copies(&p.before)?;
        if test.negate {
            self.out.put("!(")?;
        }
        self.condition(condition)?;
        self.phases(&p.snapshots, "after_operands_before_effect")?;
        if test.edge_copies {
            self.out.put(" [on_original_true {")?;
            self.selected(test.when_true, false)?;
            self.out.put("} on_original_false {")?;
            self.selected(test.when_false, false)?;
            self.out.put("}]")?;
        } else {
            self.selected(test.when_true, false)?;
            self.selected(test.when_false, false)?;
        }
        if test.negate {
            self.out.put(")")?;
        }
        self.out.span(
            start,
            Origin::Terminal {
                method: self.method,
                block: test.terminal,
            },
        )
    }
    fn compact_predicate(
        &mut self,
        control: ControlId,
        test: PredicateEmission,
    ) -> Result<(), RenderError> {
        let start = self.out.position();
        let EmissionLayout::Block(node) =
            self.code.emission().controls()[control.0 as usize].layout
        else {
            return Err(RenderError::Invariant("compact predicate lost its block"));
        };
        self.label(LabelId(node.0))?;
        self.predicate(test)?;
        self.out.span(
            start,
            Origin::Control {
                method: self.method,
                control,
            },
        )
    }
    fn block(&mut self, node: crate::p6::NodeId) -> Result<(), RenderError> {
        self.label(LabelId(node.0))?;
        let b = &self.code.emission().blocks()[node.0 as usize];
        for action in &b.actions {
            match *action {
                BodyAction::Statement { operation, .. } => self.statement(operation, None)?,
                BodyAction::Release {
                    operation, scope, ..
                } => self.statement(operation, Some(scope))?,
                BodyAction::Receive { handler } => {
                    let h = &self.code.handlers()[handler as usize];
                    if !h.receives.is_empty() {
                        self.out.indent()?;
                        self.out.put("on_exception_entry {")?;
                        self.copies(&h.receives)?;
                        self.out.put("}\n")?;
                    }
                }
            }
        }
        match &b.end {
            BlockEnd::None => Ok(()),
            BlockEnd::ConstructionFollow { group, target } => {
                self.out.indent()?;
                self.out
                    .fmt(format_args!("resume construction g{} at ", group.0))?;
                self.label_name(*target)?;
                self.out.put(";\n")
            }
            BlockEnd::Terminal(t) => self.terminal(*t),
            BlockEnd::Transfers(ts) => {
                for &t in ts {
                    self.transfer(t)?;
                }
                Ok(())
            }
        }
    }
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
            match event {
                Event::Control(id) => {
                    let c = &self.code.controls()[id.0 as usize];
                    let p = &self.code.emission().controls()[id.0 as usize];
                    let start = self.out.position();
                    push(&mut work, Event::End(id, start))?;
                    match &p.layout {
                        EmissionLayout::Block(node) => self.block(*node)?,
                        EmissionLayout::Sequence if !p.label_required => {
                            for &child in c.children.iter().rev() {
                                push(&mut work, Event::Control(child))?;
                            }
                        }
                        EmissionLayout::Sequence
                        | EmissionLayout::Labelled
                        | EmissionLayout::Graph => {
                            self.out.indent()?;
                            self.out.put(&p.label)?;
                            self.out.put(": ")?;
                            if matches!(p.layout, EmissionLayout::Graph) {
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
                            self.out.open()?;
                            push(&mut work, Event::Close)?;
                            for &child in c.children.iter().rev() {
                                push(&mut work, Event::Control(child))?;
                            }
                        }
                        EmissionLayout::If {
                            header,
                            yes,
                            no,
                            test,
                        } => {
                            if p.label_required {
                                self.out.indent()?;
                                self.out.put(&p.label)?;
                                self.out.put(":")?;
                                self.out.open()?;
                                push(&mut work, Event::Close)?;
                            }
                            push(&mut work, Event::Close)?;
                            push(&mut work, Event::Control(*no))?;
                            push(&mut work, Event::Else(test.when_false))?;
                            push(&mut work, Event::Control(*yes))?;
                            push(&mut work, Event::If(*test))?;
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
                            body,
                            terminal,
                        } => {
                            push(&mut work, Event::Close)?;
                            push(&mut work, Event::Control(*body))?;
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
                    if let Some(label) = self.code.emission().controls()[id.0 as usize].continuation
                    {
                        self.out.indent()?;
                        self.out.put("goto ")?;
                        self.label_name(label)?;
                        self.out.put("; // planned continuation\n")?;
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
                Event::If(test) => {
                    let t = &self.code.terminals()[test.terminal.0 as usize];
                    let TerminalKind::If { condition, .. } = &t.kind else {
                        return Err(RenderError::Invariant("if header lost its predicate"));
                    };
                    self.out.indent()?;
                    self.copies(
                        &self.code.emission().terminals()[test.terminal.0 as usize].before,
                    )?;
                    let a = self.out.position();
                    self.out.put("if (")?;
                    self.condition(condition)?;
                    self.phases(
                        &self.code.emission().terminals()[test.terminal.0 as usize].snapshots,
                        "after_operands_before_effect",
                    )?;
                    self.out.put(")")?;
                    self.out.span(
                        a,
                        Origin::Terminal {
                            method: self.method,
                            block: test.terminal,
                        },
                    )?;
                    self.out.open()?;
                    if !self.code.emission().transfers()[test.when_true.0 as usize]
                        .copies
                        .is_empty()
                    {
                        self.out.indent()?;
                        self.selected(test.when_true, false)?;
                        self.out.put("\n")?;
                    } else {
                        self.selected(test.when_true, false)?;
                    }
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
                    self.switch_arms(arms)?;
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
                    self.clause(clause)?;
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
                    self.out.indent()?;
                    self.copies(&phase.before)?;
                    let a = self.out.position();
                    self.out.put("synchronized ")?;
                    self.out
                        .put(&self.code.emission().controls()[id.0 as usize].label)?;
                    self.out.put("(")?;
                    self.operand(*lock)?;
                    self.out.put(") with_explicit_exits")?;
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
                    self.out.open()?;
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
    fn locals(&mut self) -> Result<(), RenderError> {
        for l in self.code.locals() {
            if self.code.emission().hidden_local(l.id)
                || matches!(l.original.kind(), crate::p5::LocalKind::Parameter { .. })
            {
                continue;
            }
            self.out.indent()?;
            match self.code.emission().local_types()[l.id.0 as usize] {
                LocalSpelling::Named(t) => self.ty(t)?,
                LocalSpelling::Machine(t) => super::expressions::machine_type(self.out, t)?,
            }
            self.out.put(" ")?;
            self.local(l.id)?;
            self.out.put(";")?;
            if let Some(gap) = l.gap {
                self.out
                    .fmt(format_args!(" unavailable_storage<{gap:?}>;"))?;
            }
            self.out.put("\n")?;
        }
        for issue in self.code.issues() {
            self.out.indent()?;
            self.out.fmt(format_args!("diagnostic {issue:?};\n"))?;
        }
        Ok(())
    }
    fn clause(&mut self, c: &Clause) -> Result<(), RenderError> {
        if let Some(catch) = &c.catch {
            match &catch.descriptor {
                Ok(t) => self.ty(*t)?,
                Err(e) => {
                    self.out
                        .fmt(format_args!("unavailable_catch<{}; ", catch.original.0))?;
                    self.out.quote(&e.detail)?;
                    self.out.put(">")?;
                }
            }
            if let Some(e) = &catch.use_issue {
                self.out.put(" [")?;
                self.out.quote(&e.detail)?;
                self.out.put("]")?;
            }
        } else {
            self.out.put("all")?;
        }
        self.out.put(" -> ")?;
        if let Some(target) = c.target {
            self.label_name(target)?;
        } else {
            self.out.put("propagate")?;
        }
        Ok(())
    }
    fn exception_chains(&mut self) -> Result<(), RenderError> {
        for chain in self.code.chains() {
            if chain.clauses.is_empty() || !self.code.emission().show_chains()[chain.id.0 as usize]
            {
                continue;
            }
            self.out.indent()?;
            self.out
                .fmt(format_args!("exception_chain c{} {{ ", chain.id.0))?;
            for c in &chain.clauses {
                self.out.fmt(format_args!("{}: ", c.order))?;
                self.clause(c)?;
                self.out.put("; ")?;
            }
            self.out.put("}\n")?;
        }
        Ok(())
    }
    fn bootstrap_definitions(&mut self) -> Result<(), RenderError> {
        for r in self.code.references() {
            if let Ok(ReferenceDeclaration::CallSite {
                name,
                prototype,
                bootstrap_member,
                arguments,
                handle_id,
                handle_kind,
                prototype_id,
                name_id,
                argument_count,
                ..
            }) = &r.declaration
            {
                self.out.indent()?;
                let a = self.out.position();
                self.out.fmt(format_args!(
                    "call_site r{} {:?} {{ name: ",
                    r.id.0, r.position.reference
                ))?;
                self.out.name(&name.units, name.form)?;
                self.out.put("; prototype: ")?;
                self.signature(prototype)?;
                self.out.fmt(format_args!("; name_id={}; prototype_id={}; bootstrap_handle={} kind={}; argument_count={}; bootstrap: ",name_id.0,prototype_id.0,handle_id.0,handle_kind,argument_count))?;
                self.reference(*bootstrap_member)?;
                self.out.put("; encoded_arguments: ")?;
                for value in arguments {
                    match value {
                        Bootstrap::EndContainer => self.out.put("end_container; ")?,
                        Bootstrap::Value {
                            name,
                            value,
                            tag,
                            value_arg,
                            ..
                        } => {
                            if let Some(name) = name {
                                self.out.units(name, true)?;
                                self.out.put(" = ")?;
                            }
                            match value {
                                BootstrapLiteral::Reference { declaration, .. } => {
                                    self.reference(*declaration)?
                                }
                                BootstrapLiteral::Annotation { type_use, count } => {
                                    self.out.put("annotation_begin<")?;
                                    self.ty(*type_use)?;
                                    self.out.fmt(format_args!("; count={count}>"))?;
                                }
                                _ => self.out.fmt(format_args!("{value:?}"))?,
                            }
                            self.out
                                .fmt(format_args!(" [tag=0x{tag:02x}; arg={value_arg}]; "))?;
                        }
                    }
                }
                self.out.put("; }\n")?;
                self.out.span(
                    a,
                    Origin::Reference {
                        method: self.method,
                        reference: r.id,
                    },
                )?;
            }
        }
        Ok(())
    }
}
fn branch(op: BranchOp) -> &'static str {
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
    End(ControlId, TextOffset),
    Close,
    If(PredicateEmission),
    Else(TransferId),
    LoopHead(PredicateEmission),
    CompactLoopTail(ControlId, PredicateEmission),
    LoopTailStart,
    LoopTailEnd(PredicateEmission),
    Switch(ControlId, BlockId),
    Catch(ControlId, usize),
    Acquire(ControlId, OpId),
}
fn push(work: &mut Vec<Event>, event: Event) -> Result<(), RenderError> {
    work.try_reserve(1).map_err(RenderError::MethodScratch)?;
    work.push(event);
    Ok(())
}
