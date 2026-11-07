use super::{Origin, RenderError, TextOffset, expressions::Expressions};
use crate::{
    p2::{BlockId, BranchOp, OpId},
    p5::Materialization,
    p7::*,
};

impl Expressions<'_, '_> {
    /// A trailing note for a statement printed in Diana syntax rather than Java.
    /// It names the original instruction and the one fact the form hides.
    fn generated_note(&self, s: &Statement) -> Option<&'static str> {
        match &s.kind {
            StatementKind::CheckCast { .. } => Some("// Diana: check-cast, value not reassigned"),
            StatementKind::CaptureException => Some("// Diana: move-exception, caught throwable"),
            StatementKind::Initialize {
                syntax: InitializationSyntax::Explicit,
                ..
            } => None,
            StatementKind::Move { expression, .. } | StatementKind::Evaluate { expression } => {
                match &self.code.expressions().get(expression.0 as usize)?.kind {
                    ExprKind::AllocateInstance { .. } => {
                        Some("// Diana: new-instance, object exists before its constructor")
                    }
                    _ => None,
                }
            }
            _ => None,
        }
    }
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
        if release.is_none() && self.code.emission().omitted_literal(id) {
            // Every use prints the literal itself, so this statement states
            // nothing; its original facts keep zero-length anchors here.
            let position = self.out.position();
            if let StatementKind::Move { expression, .. } | StatementKind::Evaluate { expression } =
                s.kind
            {
                self.out.span(
                    position,
                    Origin::Expression {
                        method: self.method,
                        expression,
                    },
                )?;
            }
            self.out.span(
                position,
                Origin::Statement {
                    method: self.method,
                    statement: id,
                },
            )?;
            self.out.span(
                position,
                Origin::Operation {
                    method: self.method,
                    operation: op,
                    pc: original.pc,
                },
            )?;
            if let Some(pc) = original.result_pc {
                self.out.span(
                    position,
                    Origin::ResultCommit {
                        method: self.method,
                        operation: op,
                        pc,
                    },
                )?;
            }
            return Ok(());
        }
        if release.is_none()
            && let Some(omitted) = self.code.emission().omitted_check(id)
        {
            // P4 established from pre-check facts that this check cannot add
            // behavior at this position, so the plan prints no text for it. Its
            // statement, operation, routes, checked value and resolved target
            // name keep zero-length anchors here; its phases and definitions
            // were empty by admission.
            let position = self.out.position();
            let StatementKind::CheckCast { value, reference } = s.kind else {
                return Err(RenderError::Invariant(
                    "omitted check lost its original check-cast kind",
                ));
            };
            self.out.span(
                position,
                Origin::Expression {
                    method: self.method,
                    expression: value,
                },
            )?;
            self.out.span(
                position,
                Origin::Reference {
                    method: self.method,
                    reference,
                },
            )?;
            // The printed check's delivered type name keeps its original Type
            // origin. P8 already maps this reference's method-scoped type use to
            // the named use it printed before omission; this adds no type fact.
            let declaration =
                self.code
                    .references()
                    .get(reference.0 as usize)
                    .ok_or(RenderError::Invariant(
                        "omitted check lost its reference declaration",
                    ))?;
            if let Ok(ReferenceDeclaration::Type { type_use }) = &declaration.declaration
                && let Some(name) = self.names.body.get(type_use.0 as usize).copied().flatten()
            {
                self.out.span(position, Origin::Type(name))?;
            }
            for &site in &s.sites {
                self.out.span(
                    position,
                    Origin::Site {
                        method: self.method,
                        site,
                    },
                )?;
            }
            self.out.span(
                position,
                Origin::Statement {
                    method: self.method,
                    statement: id,
                },
            )?;
            self.out.span(
                position,
                Origin::Operation {
                    method: self.method,
                    operation: omitted.operation,
                    pc: omitted.pc,
                },
            )?;
            if let Some(pc) = original.result_pc {
                self.out.span(
                    position,
                    Origin::ResultCommit {
                        method: self.method,
                        operation: omitted.operation,
                        pc,
                    },
                )?;
            }
            return Ok(());
        }
        self.out.indent()?;
        self.copies(&phases.before)?;
        let start = self.out.position();
        if matches!(s.kind, StatementKind::Construction(_)) {
            self.copies(&phases.snapshots)?;
        }
        if let Some(scope) = release {
            let control = &self.code.emission().controls()[scope.0 as usize];
            self.out.put("release synchronized ")?;
            // A release names its control only when that control prints the
            // label; otherwise the enclosing synchronized header identifies it.
            if control.label_required {
                self.out.put(&control.label)?;
            }
            self.out.put("(")?;
            let StatementKind::Monitor { lock, .. } = s.kind else {
                return Err(RenderError::Invariant("release syntax lost its monitor"));
            };
            self.operand(lock)?;
            self.out.put(")")?;
        } else {
            // Only the statement's own operands may carry an inlined check: the
            // copies printed before it and any snapshot value after it are not
            // the check's first use.
            let previous = self.check_carrier.replace(CheckCarrier::Statement(id));
            let statement = self.statement.replace(id);
            let rendered = if self.code.emission().allocation_declaration(id) {
                self.allocation_declaration(s)
            } else {
                self.assignment(s).and_then(|()| self.semantic(s))
            };
            self.check_carrier = previous;
            self.statement = statement;
            rendered?;
        }
        if !matches!(s.kind, StatementKind::Construction(_)) {
            self.phases(&phases.snapshots, "after_operands_before_effect")?;
        }
        self.sites(&s.sites, start)?;
        self.out.put(";")?;
        if let Some(note) = self.generated_note(s) {
            self.out.put(" ")?;
            self.out.put(note)?;
        }
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
                if !matches!(source.emission, Emission::Construction(_)) {
                    continue;
                }
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
    /// A new-instance statement states the allocated type and its local where the
    /// original allocation runs. The constructor call stays at its own position,
    /// so the object is never claimed to be initialized here.
    fn allocation_declaration(&mut self, s: &Statement) -> Result<(), RenderError> {
        let StatementKind::Evaluate { expression } = s.kind else {
            return Err(RenderError::Invariant(
                "allocation declaration lost its evaluate statement",
            ));
        };
        let ExprKind::AllocateInstance { reference } = self
            .code
            .expressions()
            .get(expression.0 as usize)
            .ok_or(RenderError::Invariant(
                "allocation declaration lost its expression",
            ))?
            .kind
        else {
            return Err(RenderError::Invariant(
                "allocation declaration lost its new-instance",
            ));
        };
        let [output] = s.outputs.as_slice() else {
            return Err(RenderError::Invariant(
                "allocation declaration lost its publication",
            ));
        };
        let local = match output.materialization() {
            crate::p5::Materialization::Store(local)
            | crate::p5::Materialization::AllocationCarrier(local) => StorageId(local.0),
            _ => {
                return Err(RenderError::Invariant(
                    "allocation declaration lost its local",
                ));
            }
        };
        let start = self.out.position();
        self.reference(reference)?;
        self.out.put(" ")?;
        self.local(local)?;
        self.out.put(" = new ")?;
        // No argument list: the constructor has not run at this position, so a
        // `()` would claim a default constructor that this allocation never ran.
        self.reference(reference)?;
        self.out.span(
            start,
            Origin::Expression {
                method: self.method,
                expression,
            },
        )
    }
    fn sites(&mut self, sites: &[SiteId], statement: TextOffset) -> Result<(), RenderError> {
        // One statement can own several original sites from one chain, as a
        // contracted construction does. The marker names only that chain, so its
        // text prints once; every site keeps its own zero-length anchor.
        let mut printed: Vec<u32> = Vec::new();
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
            if printed.contains(&s.chain.0) {
                self.out.span(statement, origin)?;
                continue;
            }
            printed.try_reserve(1).map_err(RenderError::MethodScratch)?;
            printed.push(s.chain.0);
            let start = self.out.position();
            // The printed chain line carries clause order, catch types and
            // handler targets; the site only names the chain it selects.
            self.out.fmt(format_args!(" [exn c{}]", s.chain.0))?;
            self.out.span(start, origin)?;
        }
        Ok(())
    }
    fn condition(&mut self, c: &Condition, negate: bool) -> Result<(), RenderError> {
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
    fn selected(&mut self, id: TransferId, jump: bool) -> Result<(), RenderError> {
        let start = self.out.position();
        let copies = &self.code.emission().transfers()[id.0 as usize].copies;
        self.copies(copies)?;
        if jump {
            if !copies.is_empty() {
                self.out.put(" ")?;
            }
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
    /// A fallthrough with no copies keeps its original route without printing
    /// words, so its span stays zero-width where the route was taken.
    fn transfer_prints_nothing(&self, id: TransferId) -> bool {
        let p = &self.code.emission().transfers()[id.0 as usize];
        matches!(p.spelling, TransferSpelling::Fallthrough) && p.copies.is_empty()
    }
    /// A fallthrough whose bundles only carry source facts prints no words.
    /// Their Copy and Transfer origins are emitted as zero-width anchors.
    fn transfer_prints_no_words(&self, id: TransferId) -> bool {
        let p = &self.code.emission().transfers()[id.0 as usize];
        matches!(p.spelling, TransferSpelling::Fallthrough)
            && p.copies.iter().all(|&copy| {
                let bundle = &self.code.copies()[copy.0 as usize];
                bundle.steps.is_empty() && bundle.assignments.iter().all(|c| c.issue().is_none())
            })
    }
    fn transfer_with_indent(&mut self, id: TransferId, indent: bool) -> Result<(), RenderError> {
        let p = &self.code.emission().transfers()[id.0 as usize];
        let start = self.out.position();
        if self.transfer_prints_nothing(id) {
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
        if !p.copies.is_empty() && !matches!(p.spelling, TransferSpelling::Fallthrough) {
            self.out.put(" ")?;
        }
        if matches!(
            p.spelling,
            TransferSpelling::BreakHere(_) | TransferSpelling::ContinueHere(_)
        ) {
            self.out
                .put(if matches!(p.spelling, TransferSpelling::BreakHere(_)) {
                    "break;"
                } else {
                    "continue;"
                })?;
            self.out.span(
                start,
                Origin::Transfer {
                    method: self.method,
                    transfer: id,
                },
            )?;
            return self.out.put("\n");
        }
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
            TransferSpelling::BreakHere(_) | TransferSpelling::ContinueHere(_) => {}
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
        // The closing brace of the method body performs a void return that
        // prints nothing else, so P7 omits that redundant `return;`.
        if self.code.emission().omitted_return(id) {
            return Ok(());
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
    /// Renders one terminal operand under this terminal's check carrier, so a
    /// check-cast prints at its exact read and the previous carrier is restored
    /// even when rendering fails.
    fn carried(
        &mut self,
        terminal: BlockId,
        render: impl FnOnce(&mut Self) -> Result<(), RenderError>,
    ) -> Result<(), RenderError> {
        let previous = self.check_carrier.replace(CheckCarrier::Terminal(terminal));
        let rendered = render(self);
        self.check_carrier = previous;
        rendered
    }
    fn branch_transfer(&mut self, id: TransferId, indent: bool) -> Result<(), RenderError> {
        if matches!(
            self.code.emission().transfers()[id.0 as usize].spelling,
            TransferSpelling::Selected
        ) {
            if indent {
                self.out.indent()?;
            }
            self.selected(id, true)?;
            self.out.put("\n")
        } else {
            self.transfer_with_indent(id, indent)
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
    fn predicate_expression(
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
    fn predicate(&mut self, test: PredicateEmission) -> Result<(), RenderError> {
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
    fn compact_predicate(
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
                                IfShape::NoElse => {
                                    // The true arm never falls out of the branch,
                                    // so the false arm prints after the closed if.
                                    push(&mut work, Event::Control(*no))?;
                                    push(&mut work, Event::Close)?;
                                    push(&mut work, Event::Control(*yes))?;
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
    fn locals(&mut self) -> Result<(), RenderError> {
        for l in self.code.locals() {
            if self.code.emission().hidden_local(l.id)
                || self.code.emission().allocation_local(l.id)
                || matches!(l.original.kind(), crate::p5::LocalKind::Parameter { .. })
            {
                // An allocated object's declaration stands at its allocation.
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
    /// The caught type of one clause, without its route target.
    fn clause_type(&mut self, c: &Clause) -> Result<(), RenderError> {
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
/// The exact complement of a delivered branch operator. A negation is printed by
/// naming the opposite test instead of wrapping the whole condition, which keeps
/// the same integer comparison and the same selected edge.
fn complement(op: BranchOp) -> BranchOp {
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
fn push<T>(work: &mut Vec<T>, event: T) -> Result<(), RenderError> {
    work.try_reserve(1).map_err(RenderError::MethodScratch)?;
    work.push(event);
    Ok(())
}
