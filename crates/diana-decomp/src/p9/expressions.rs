use super::{
    Origin, RenderError,
    buffer::Buffer,
    names::{Lookup, MethodNames},
};
use crate::{
    MethodId,
    p4::{Bits, Family, Interpretation},
    p5,
    p7::*,
};

pub(super) trait ExpressionSource {
    fn payloads(&self) -> &[ArrayPayload];
    fn types(&self) -> &[TypeUse];
    fn references(&self) -> &[Reference];
    fn locals(&self) -> &[LocalDeclaration];
    fn expressions(&self) -> &[Expression];
    fn constructions(&self) -> &[Construction];
    fn copies(&self) -> &[CopyBundle];
    fn composed_use(&self, _id: ExprId) -> Option<ComposedExpression> {
        None
    }
    /// A partial product has no selected display plan, so it keeps the type.
    fn field_qualification(&self, _reference: ReferenceId) -> bool {
        true
    }
    fn inline_check(&self, _statement: StatementId, _value: StorageId) -> Option<InlineCheck> {
        None
    }
    fn statement_sites(&self, _statement: StatementId) -> &[SiteId] {
        &[]
    }
    /// A partial product has no selected store form.
    fn natural_store(&self, _statement: StatementId) -> bool {
        false
    }
    /// A partial product has no selected allocation spelling.
    fn array_literal(&self, _statement: StatementId) -> Option<ArrayLiteral> {
        None
    }
}
impl ExpressionSource for MethodCode {
    fn payloads(&self) -> &[ArrayPayload] {
        self.payloads()
    }
    fn types(&self) -> &[TypeUse] {
        self.types()
    }
    fn references(&self) -> &[Reference] {
        self.references()
    }
    fn locals(&self) -> &[LocalDeclaration] {
        self.locals()
    }
    fn expressions(&self) -> &[Expression] {
        self.expressions()
    }
    fn constructions(&self) -> &[Construction] {
        self.constructions()
    }
    fn copies(&self) -> &[CopyBundle] {
        self.copies()
    }
    fn composed_use(&self, id: ExprId) -> Option<ComposedExpression> {
        self.emission().composed_use(id)
    }
    fn field_qualification(&self, reference: ReferenceId) -> bool {
        self.emission().field_qualification(reference)
    }
    fn inline_check(&self, statement: StatementId, value: StorageId) -> Option<InlineCheck> {
        self.emission().inline_check(statement, value)
    }
    fn statement_sites(&self, statement: StatementId) -> &[SiteId] {
        self.statements()
            .get(statement.0 as usize)
            .map_or(&[], |statement| statement.sites.as_slice())
    }
    fn natural_store(&self, statement: StatementId) -> bool {
        self.emission().natural_store(statement)
    }
    fn array_literal(&self, statement: StatementId) -> Option<ArrayLiteral> {
        self.emission().array_literal(statement)
    }
}
impl ExpressionSource for ExpressionPartial {
    fn payloads(&self) -> &[ArrayPayload] {
        self.payloads()
    }
    fn types(&self) -> &[TypeUse] {
        self.type_uses()
    }
    fn references(&self) -> &[Reference] {
        self.reference_declarations()
    }
    fn locals(&self) -> &[LocalDeclaration] {
        self.local_declarations()
    }
    fn expressions(&self) -> &[Expression] {
        self.expressions()
    }
    fn constructions(&self) -> &[Construction] {
        self.constructions()
    }
    fn copies(&self) -> &[CopyBundle] {
        self.copies()
    }
}
pub(super) struct Expressions<'a, 'b, S: ExpressionSource = MethodCode> {
    pub out: &'b mut Buffer,
    pub method: MethodId,
    pub code: &'a S,
    pub names: &'a MethodNames,
    pub lookup: &'a Lookup<'a>,
    pub pending_controls: Option<&'b mut [Option<super::TextOffset>]>,
    /// The statement currently printing, so an inlined check-cast can wrap the
    /// read that carries it.
    pub statement: Option<StatementId>,
}
impl<S: ExpressionSource> Expressions<'_, '_, S> {
    /// A construction whose only publication is its own deferred carrier is an
    /// ordinary assignment: Java assigns the local only after `new` completes, so
    /// no separate success marker is needed.
    fn deferred_publication(&self, s: &Statement) -> Option<StorageId> {
        let StatementKind::Construction(group) = s.kind else {
            return None;
        };
        let group = self.code.constructions().get(group.0 as usize)?;
        let [publication] = group.publications.as_slice() else {
            return None;
        };
        let mut carrier = None;
        for definition in &s.outputs {
            if let p5::Materialization::Deferred { local, .. } = definition.materialization() {
                carrier = Some(StorageId(local.0));
            }
        }
        (carrier == Some(publication.destination)).then_some(publication.destination)
    }
    pub fn assignment(&mut self, s: &Statement) -> Result<(), RenderError> {
        let deferred = self.deferred_publication(s);
        let mut first = true;
        for definition in &s.outputs {
            let local = match definition.materialization() {
                p5::Materialization::Store(local)
                | p5::Materialization::AllocationCarrier(local)
                | p5::Materialization::ExceptionCapture(local) => Some(local),
                p5::Materialization::Deferred { local, .. }
                    if deferred == Some(StorageId(local.0)) =>
                {
                    Some(local)
                }
                _ => None,
            };
            if let Some(local) = local {
                if !first {
                    self.out.put(", ")?;
                }
                self.local(StorageId(local.0))?;
                first = false;
            }
        }
        if !first {
            self.out.put(" = ")?;
        }
        Ok(())
    }
    /// The payload the allocation prints, between its delivered array type and
    /// the closing brace: every element in original order and exact bits.
    fn array_literal(&mut self, literal: &ArrayLiteral) -> Result<(), RenderError> {
        let payload = self
            .code
            .payloads()
            .get(literal.payload.0 as usize)
            .ok_or(RenderError::Invariant("array literal lost its payload"))?;
        let Ok(element) = payload.element else {
            return Err(RenderError::Invariant(
                "array literal lost its element interpretation",
            ));
        };
        self.out.put("{")?;
        for (index, &bits) in payload.bits.iter().enumerate() {
            if index > 0 {
                self.out.put(", ")?;
            }
            element_literal(self.out, element, bits)?;
        }
        self.out.put("}")
    }
    pub fn semantic(&mut self, s: &Statement) -> Result<(), RenderError> {
        match &s.kind {
            StatementKind::Nop => self.out.put("nop")?,
            StatementKind::Move { expression, .. } | StatementKind::Evaluate { expression } => {
                self.expr(*expression)?
            }
            StatementKind::FieldStore {
                reference,
                receiver,
                value,
            } => {
                let qualified = self.code.field_qualification(*reference);
                if self.code.natural_store(s.id) {
                    self.member(*reference, *receiver, qualified)?;
                    self.out.put(" = ")?;
                    self.operand(*value)?;
                } else {
                    self.out.put("store ")?;
                    self.operand(*value)?;
                    self.out.put(" -> ")?;
                    self.member(*reference, *receiver, qualified)?;
                }
            }
            StatementKind::ArrayStore {
                array,
                index,
                value,
                kind,
            } => {
                if self.code.natural_store(s.id) {
                    self.operand(*array)?;
                    self.out.put("[")?;
                    self.operand(*index)?;
                    self.out.put("] = ")?;
                    self.operand(*value)?;
                    // Only a narrowing store needs its original width kept.
                    if matches!(
                        kind,
                        crate::p2::ValueKind::Byte
                            | crate::p2::ValueKind::Char
                            | crate::p2::ValueKind::Short
                    ) {
                        self.out.fmt(format_args!(" /* {kind:?} store */"))?;
                    }
                } else {
                    self.out.put("store ")?;
                    self.operand(*value)?;
                    self.out.put(" -> ")?;
                    self.operand(*array)?;
                    self.out.put("[")?;
                    self.operand(*index)?;
                    self.out.put("]")?;
                    self.out.fmt(format_args!(" /* {kind:?} store */"))?;
                }
            }
            StatementKind::CheckCast { value, reference } => {
                // A cast statement keeps the check at its original position and
                // writes no register; the value keeps its own local.
                self.out.put("cast ")?;
                self.operand(*value)?;
                self.out.put(" to ")?;
                self.reference(*reference)?;
            }
            StatementKind::FillArray { array, payload } => {
                self.out.put("fill_array ")?;
                self.operand(*array)?;
                let p = &self.code.payloads()[payload.0 as usize];
                self.out.fmt(format_args!(
                    " from payload<width={}; count={}; element={:?}>[",
                    p.width, p.count, p.element
                ))?;
                for (i, bits) in p.bits.iter().enumerate() {
                    if i > 0 {
                        self.out.put(", ")?;
                    }
                    self.out.fmt(format_args!("0x{bits:x}"))?;
                }
                self.out.put("]")?;
            }
            StatementKind::Monitor { enter, lock } => {
                self.out.put(if *enter {
                    "monitor_enter "
                } else {
                    "monitor_exit "
                })?;
                self.operand(*lock)?;
            }
            StatementKind::CaptureException => self.out.put("capture_exception")?,
            StatementKind::Initialize {
                expression,
                syntax,
                relation,
            } => {
                let ExprKind::Call(c) = &self.code.expressions()[expression.0 as usize].kind else {
                    return Err(RenderError::Invariant("initialization lost its invocation"));
                };
                let a = self.out.position();
                self.call(c, Some(*syntax), Some(*relation))?;
                self.out.span(
                    a,
                    Origin::Expression {
                        method: self.method,
                        expression: *expression,
                    },
                )?;
            }
            StatementKind::UnresolvedResult { width } => self
                .out
                .fmt(format_args!("unavailable_result<{width:?}>"))?,
            StatementKind::Construction(group) => {
                let g = &self.code.constructions()[group.0 as usize];
                self.expr(g.expression)?;
                if self.deferred_publication(s).is_none() {
                    self.out.put(" [on_success publish (")?;
                    for (i, p) in g.publications.iter().enumerate() {
                        if i > 0 {
                            self.out.put(", ")?;
                        }
                        self.local(p.destination)?;
                    }
                    self.out.put(")]")?;
                }
            }
        }
        Ok(())
    }
    pub fn local(&mut self, id: StorageId) -> Result<(), RenderError> {
        let l = self
            .code
            .locals()
            .get(id.0 as usize)
            .ok_or(RenderError::Invariant(
                "expression lost its local declaration",
            ))?;
        self.out.put(&l.name)
    }
    pub fn ty(&mut self, id: TypeUseId) -> Result<(), RenderError> {
        self.names
            .body(self.out, self.lookup, self.code.types(), id)
    }
    pub fn operand(&mut self, id: ExprId) -> Result<(), RenderError> {
        if !matches!(
            self.code.expressions().get(id.0 as usize).map(|e| &e.kind),
            Some(
                ExprKind::Read { .. }
                    | ExprKind::Snapshot { .. }
                    | ExprKind::Literal { .. }
                    | ExprKind::Unavailable { .. }
            )
        ) {
            return Err(RenderError::Invariant(
                "operand is not a delivered leaf read",
            ));
        }
        self.expr(id)
    }
    pub fn args(&mut self, args: &Arguments) -> Result<(), RenderError> {
        match args {
            Arguments::Known(args) => self.args_known(args),
            Arguments::Unavailable { gap, words } => self.out.fmt(format_args!(
                "unavailable_arguments<{gap:?}; original_words={words:?}>"
            )),
        }
    }
    pub fn args_known(&mut self, args: &[ExprId]) -> Result<(), RenderError> {
        for (i, &arg) in args.iter().enumerate() {
            if i != 0 {
                self.out.put(", ")?;
            }
            self.operand(arg)?;
        }
        Ok(())
    }
    pub fn expr(&mut self, id: ExprId) -> Result<(), RenderError> {
        let e = self
            .code
            .expressions()
            .get(id.0 as usize)
            .ok_or(RenderError::Invariant(
                "expression reference is outside its arena",
            ))?;
        let start = self.out.position();
        match &e.kind {
            ExprKind::Read { storage, .. } => {
                if let Some(check) = self
                    .statement
                    .and_then(|statement| self.code.inline_check(statement, *storage))
                {
                    // The check prints at its first use and keeps its own origin.
                    self.out.put("((")?;
                    self.reference(check.reference)?;
                    self.out.put(") ")?;
                    self.local(*storage)?;
                    self.out.put(")")?;
                    self.out.span(
                        start,
                        Origin::Statement {
                            method: self.method,
                            statement: check.check,
                        },
                    )?;
                    self.out.span(
                        start,
                        Origin::Operation {
                            method: self.method,
                            operation: check.operation,
                            pc: check.pc,
                        },
                    )?;
                    // A hidden route still keeps an anchor at the check's text.
                    for &site in self.code.statement_sites(check.check) {
                        self.out.span(
                            start,
                            Origin::Site {
                                method: self.method,
                                site,
                            },
                        )?;
                    }
                } else if let Some(composed) = self.code.composed_use(id) {
                    // Inline checks belong to the producer statement even when
                    // its expression prints at the adjacent consuming branch.
                    let previous = self.statement.replace(composed.statement);
                    let rendered = self.expr(composed.expression);
                    self.statement = previous;
                    rendered?;
                    self.out.span(
                        start,
                        Origin::Statement {
                            method: self.method,
                            statement: composed.statement,
                        },
                    )?;
                    self.out.span(
                        start,
                        Origin::Operation {
                            method: self.method,
                            operation: composed.operation,
                            pc: composed.pc,
                        },
                    )?;
                    if let Some(control) = composed.source_control {
                        let start = self
                            .pending_controls
                            .as_deref_mut()
                            .and_then(|pending| pending.get_mut(control.0 as usize))
                            .and_then(Option::take)
                            .ok_or(RenderError::Invariant("composed control was not opened"))?;
                        self.out.span(
                            start,
                            Origin::Control {
                                method: self.method,
                                control,
                            },
                        )?;
                    }
                    if let Some(site) = composed.site {
                        let position = self.out.position();
                        self.out.span(
                            position,
                            Origin::Site {
                                method: self.method,
                                site,
                            },
                        )?;
                    }
                    if let Some(pc) = composed.result_pc {
                        self.out.span(
                            start,
                            Origin::ResultCommit {
                                method: self.method,
                                operation: composed.operation,
                                pc,
                            },
                        )?;
                    }
                } else {
                    self.local(*storage)?;
                }
            }
            ExprKind::Snapshot { storage } => self.local(*storage)?,
            ExprKind::Literal { bits, view } => literal(self.out, *bits, *view)?,
            ExprKind::Unavailable { gap } => self.out.fmt(format_args!("unavailable<{gap:?}>"))?,
            ExprKind::Unary { op, value } => {
                self.out.put("(")?;
                self.out.put(unary(*op))?;
                self.operand(*value)?;
                self.out.put(")")?;
            }
            ExprKind::Binary { op, left, right } => {
                self.out.put("(")?;
                self.operand(*left)?;
                self.out.put(binary(*op))?;
                self.operand(*right)?;
                self.out.put(")")?;
            }
            ExprKind::Compare { op, left, right } => {
                self.out.fmt(format_args!("compare<{op:?}>("))?;
                self.operand(*left)?;
                self.out.put(", ")?;
                self.operand(*right)?;
                self.out.put(")")?;
            }
            ExprKind::String(r) => self.reference(*r)?,
            ExprKind::Class(r) => {
                self.reference(*r)?;
                self.out.put(".class")?;
            }
            ExprKind::MethodHandle(r) | ExprKind::MethodType(r) => self.reference(*r)?,
            ExprKind::Field {
                reference,
                receiver,
            } => {
                let qualified = self.code.field_qualification(*reference);
                self.member(*reference, *receiver, qualified)?;
            }
            ExprKind::Array { array, index, kind } => {
                self.operand(*array)?;
                self.out.put("[")?;
                self.operand(*index)?;
                self.out.fmt(format_args!("] /* {kind:?} load */"))?;
            }
            ExprKind::ArrayLength(a) => {
                self.operand(*a)?;
                self.out.put(".length")?;
            }
            ExprKind::InstanceOf { value, reference } => {
                self.out.put("(")?;
                self.operand(*value)?;
                self.out.put(" instanceof ")?;
                self.reference(*reference)?;
                self.out.put(")")?;
            }
            ExprKind::AllocateInstance { reference } => {
                self.out.put("allocate_uninitialized ")?;
                self.reference(*reference)?;
            }
            ExprKind::NewArray { reference, length } => {
                self.out.put("new ")?;
                let literal = self
                    .statement
                    .and_then(|statement| self.code.array_literal(statement));
                if let Some(literal) = literal {
                    // The delivered array type already carries its one dimension;
                    // the payload states every element, in original order and bits.
                    self.reference(*reference)?;
                    self.array_literal(&literal)?;
                    // The fill statement prints no text of its own, so its
                    // statement, operation and hidden route origins stay at the
                    // literal that states its effect.
                    self.out.span(
                        start,
                        Origin::Statement {
                            method: self.method,
                            statement: literal.fill,
                        },
                    )?;
                    self.out.span(
                        start,
                        Origin::Operation {
                            method: self.method,
                            operation: literal.operation,
                            pc: literal.pc,
                        },
                    )?;
                    for &site in self.code.statement_sites(literal.fill) {
                        self.out.span(
                            start,
                            Origin::Site {
                                method: self.method,
                                site,
                            },
                        )?;
                    }
                } else {
                    // The delivered type is the array type; the instruction supplies
                    // the first length and every remaining dimension is empty.
                    let element = match &self.code.references()[reference.0 as usize].declaration {
                        Ok(ReferenceDeclaration::Type { type_use }) => self
                            .names
                            .body
                            .get(type_use.0 as usize)
                            .copied()
                            .flatten()
                            .and_then(|id| self.lookup.dimensions(id).map(|d| (id, d))),
                        _ => None,
                    };
                    match element {
                        Some((type_use, dimensions)) if dimensions > 0 => {
                            self.lookup.print_element(self.out, type_use)?;
                            self.out.put("[")?;
                            self.operand(*length)?;
                            self.out.put("]")?;
                            for _ in 1..dimensions {
                                self.out.put("[]")?;
                            }
                        }
                        _ => {
                            self.reference(*reference)?;
                            self.out.put("[")?;
                            self.operand(*length)?;
                            self.out.put("]")?;
                        }
                    }
                }
            }
            ExprKind::FilledArray {
                reference,
                elements,
            } => {
                self.out.put("new ")?;
                self.reference(*reference)?;
                self.out.put(" {")?;
                self.args(elements)?;
                self.out.put("}")?;
            }
            ExprKind::Call(call) => self.call(call, None, None)?,
            ExprKind::New {
                reference,
                constructor,
                arguments,
            } => {
                self.out.put("new ")?;
                self.reference(*reference)?;
                self.out.put("(")?;
                self.args_known(arguments)?;
                self.out.put(")")?;
                // `new T(...)` already states T's constructor. The suffix stays
                // when the delivered initializer is a different declaration.
                if !self.redundant_initializer(*reference, *constructor) {
                    self.out.put(" [initializer: ")?;
                    self.member(*constructor, None, true)?;
                    self.out.put("]")?;
                }
            }
        }
        self.out.span(
            start,
            Origin::Expression {
                method: self.method,
                expression: id,
            },
        )
    }
    fn redundant_initializer(&self, reference: ReferenceId, constructor: ReferenceId) -> bool {
        let (
            Ok(ReferenceDeclaration::Type { type_use }),
            Ok(ReferenceDeclaration::Member(MemberDeclaration::Method { owner, name, .. })),
        ) = (
            &self.code.references()[reference.0 as usize].declaration,
            &self.code.references()[constructor.0 as usize].declaration,
        )
        else {
            return false;
        };
        let initializer = name.units.iter().copied().eq("<init>".encode_utf16());
        initializer
            && self.code.types()[owner.0 as usize].units
                == self.code.types()[type_use.0 as usize].units
    }
    pub fn signature(&mut self, p: &Prototype) -> Result<(), RenderError> {
        self.out.put("<(")?;
        for (i, &ty) in p.parameters.iter().enumerate() {
            if i > 0 {
                self.out.put(", ")?;
            }
            self.out
                .units(&self.code.types()[ty.0 as usize].units, true)?;
        }
        self.out.put(") -> ")?;
        self.out
            .units(&self.code.types()[p.returns.0 as usize].units, true)?;
        self.out.put(">")
    }
    pub fn member(
        &mut self,
        r: ReferenceId,
        receiver: Option<ExprId>,
        signature: bool,
    ) -> Result<(), RenderError> {
        let reference = &self.code.references()[r.0 as usize];
        let start = self.out.position();
        if let Some(recv) = receiver {
            self.operand(recv)?;
            self.out.put(".")?;
        }
        match &reference.declaration {
            Ok(ReferenceDeclaration::Member(m)) => {
                let (owner, name, initializer) = match m {
                    MemberDeclaration::Field { owner, name, .. } => (*owner, name, false),
                    MemberDeclaration::Method { owner, name, .. } => (
                        *owner,
                        name,
                        name.units.iter().copied().eq("<init>".encode_utf16()),
                    ),
                };
                if initializer {
                    // A constructor reference names the type it constructs.
                    // `name<"<init>">` would hide the owner behind a name that
                    // is not an identifier, and repeating the owner before it
                    // would print the type twice.
                    self.ty(owner)?;
                } else {
                    if receiver.is_none() {
                        self.ty(owner)?;
                        self.out.put(".")?;
                    }
                    if signature && receiver.is_some() {
                        self.out.put("binding<")?;
                        self.ty(owner)?;
                        self.out.put(">::")?;
                    }
                    self.out.name(&name.units, name.form)?;
                }
                if signature {
                    match m {
                        MemberDeclaration::Field { field_type, .. } => {
                            self.out.put("<")?;
                            self.names.body(
                                self.out,
                                self.lookup,
                                self.code.types(),
                                *field_type,
                            )?;
                            self.out.put(">")?;
                        }
                        MemberDeclaration::Method { prototype, .. } => self.signature(prototype)?,
                    }
                }
            }
            _ => self.reference(r)?,
        }
        if matches!(reference.declaration, Ok(ReferenceDeclaration::Member(_)))
            && let Some(issue) = &reference.use_issue
        {
            self.out.put(" [reference_use_unavailable: ")?;
            self.out.quote(&issue.detail)?;
            self.out.put("]")?;
        }
        self.out.span(
            start,
            Origin::Reference {
                method: self.method,
                reference: r,
            },
        )
    }
    pub fn call(
        &mut self,
        c: &Call,
        initialization: Option<InitializationSyntax>,
        relation: Option<crate::p4::ConstructorRelation>,
    ) -> Result<(), RenderError> {
        if let Some(InitializationSyntax::This | InitializationSyntax::Super) = initialization {
            let start = self.out.position();
            self.out.put(
                if matches!(initialization, Some(InitializationSyntax::This)) {
                    "this"
                } else {
                    "super"
                },
            )?;
            if let Some(receiver) = c.receiver {
                self.out.span(
                    start,
                    Origin::Expression {
                        method: self.method,
                        expression: receiver,
                    },
                )?;
            }
        } else {
            if initialization.is_some() {
                self.out.put("initialize ")?;
            } else if matches!(
                c.kind,
                crate::p2::InvokeKind::Custom | crate::p2::InvokeKind::Polymorphic
            ) {
                self.out.fmt(format_args!("invoke<{:?}> ", c.kind))?;
            }
            // A constructor delegation names the original constructor's owner,
            // never the object under construction: `this`/`super` already say
            // the receiver, so repeating it would claim a `this.<init>` target
            // for a superclass call.
            let receiver = match relation {
                Some(
                    crate::p4::ConstructorRelation::This | crate::p4::ConstructorRelation::Super,
                ) => None,
                _ => c.receiver,
            };
            self.member(c.target, receiver, false)?;
            if receiver.is_none()
                && let Some(receiver) = c.receiver
            {
                let start = self.out.position();
                self.out.span(
                    start,
                    Origin::Expression {
                        method: self.method,
                        expression: receiver,
                    },
                )?;
            }
        }
        if let Some(proto) = c.invocation_prototype {
            self.out.put(" [invocation prototype: ")?;
            self.reference(proto)?;
            self.out.put("]")?;
        }
        self.out.put("(")?;
        self.args(&c.arguments)?;
        self.out.put(")")?;
        // Only a selected explicit signature stays in the text; the original
        // target id, dispatch kind and prototype remain owned reference facts
        // and keep their source span without repeating after every call.
        if matches!(c.qualification, Qualification::FullSignature) {
            self.out.put(" [binding ")?;
            self.member(c.target, None, true)?;
            self.out.put("]")?;
        }
        Ok(())
    }
    pub fn reference(&mut self, r: ReferenceId) -> Result<(), RenderError> {
        let reference = self
            .code
            .references()
            .get(r.0 as usize)
            .ok_or(RenderError::Invariant("reference exceeds the owned arena"))?;
        let start = self.out.position();
        match &reference.declaration {
            Err(error) => {
                self.out.fmt(format_args!(
                    "unavailable_reference<{:?}; ",
                    reference.position.reference
                ))?;
                self.out.quote(&error.detail)?;
                self.out.put(">")?;
            }
            Ok(ReferenceDeclaration::String { units }) => self.out.units(units, true)?,
            Ok(ReferenceDeclaration::Type { type_use }) => self.ty(*type_use)?,
            Ok(ReferenceDeclaration::Member(_)) => self.member(r, None, true)?,
            Ok(ReferenceDeclaration::Prototype(p)) => {
                self.out.fmt(format_args!(
                    "method_type<{:?}>",
                    reference.position.reference
                ))?;
                self.signature(p)?;
            }
            Ok(ReferenceDeclaration::MethodHandle {
                kind,
                member,
                declaration,
            }) => {
                self.out.fmt(format_args!(
                    "method_handle<{:?}; kind={kind}; {member:?}; ",
                    reference.position.reference
                ))?;
                match declaration {
                    MemberDeclaration::Field {
                        owner,
                        name,
                        field_type,
                    } => {
                        self.ty(*owner)?;
                        self.out.put(".")?;
                        self.out.name(&name.units, name.form)?;
                        self.out.put(": ")?;
                        self.ty(*field_type)?;
                    }
                    MemberDeclaration::Method {
                        owner,
                        name,
                        prototype,
                    } => {
                        // A constructor handle names the type it constructs,
                        // the same way an ordinary constructor reference does.
                        self.ty(*owner)?;
                        if !name.units.iter().copied().eq("<init>".encode_utf16()) {
                            self.out.put(".")?;
                            self.out.name(&name.units, name.form)?;
                        }
                        self.signature(prototype)?;
                    }
                }
                self.out.put(">")?;
            }
            Ok(ReferenceDeclaration::CallSite { .. }) => self.out.fmt(format_args!(
                "call_site<r{}; {:?}>",
                r.0, reference.position.reference
            ))?,
        }
        if let Some(issue) = &reference.use_issue {
            self.out.put(" [reference_use_unavailable: ")?;
            self.out.quote(&issue.detail)?;
            self.out.put("]")?;
        }
        self.out.span(
            start,
            Origin::Reference {
                method: self.method,
                reference: r,
            },
        )
    }
    pub fn copy(&mut self, id: CopyPlanId) -> Result<(), RenderError> {
        let bundle = &self.code.copies()[id.0 as usize];
        let start = self.out.position();
        for (index, step) in bundle.steps.iter().enumerate() {
            if index > 0 {
                self.out.put(" ")?;
            }
            match step {
                CopyStatement::Assign {
                    destination,
                    expression,
                    ..
                } => {
                    self.local(*destination)?;
                    self.out.put(" = ")?;
                    self.operand(*expression)?;
                }
                CopyStatement::Save {
                    destination,
                    source,
                } => {
                    self.local(*destination)?;
                    self.out.put(" = ")?;
                    self.local(*source)?;
                }
            }
            self.out.put(";")?;
        }
        for (index, req) in bundle.assignments.iter().enumerate() {
            if let Some(gap) = req.issue() {
                if !bundle.steps.is_empty() || index > 0 {
                    self.out.put(" ")?;
                }
                self.out.fmt(format_args!("unavailable_copy<{gap:?}>;"))?;
            }
        }
        self.out.span(
            start,
            Origin::Copy {
                method: self.method,
                copy: id,
            },
        )
    }
    pub fn copies(&mut self, ids: &[CopyPlanId]) -> Result<(), RenderError> {
        for &id in ids {
            self.copy(id)?;
        }
        Ok(())
    }
    pub fn phases(&mut self, ids: &[CopyPlanId], tag: &str) -> Result<(), RenderError> {
        if !ids.is_empty() {
            self.out.put(" [")?;
            self.out.put(tag)?;
            self.out.put(" {")?;
            self.copies(ids)?;
            self.out.put("}]")?;
        }
        Ok(())
    }
}
pub(super) fn machine_type(out: &mut Buffer, ty: p5::StorageType) -> Result<(), RenderError> {
    match ty {
        p5::StorageType::Family { family, .. } => out.put(match family {
            Family::Integral => "int",
            Family::Long => "long",
            Family::Float => "float",
            Family::Double => "double",
            Family::Reference => "reference",
        }),
        p5::StorageType::NumericBits { width, families } => {
            out.fmt(format_args!("bits<{width:?}; {families:?}>"))
        }
    }
}
pub(super) fn literal(
    out: &mut Buffer,
    bits: Bits,
    view: Interpretation,
) -> Result<(), RenderError> {
    let family = match view {
        Interpretation::Null => return out.put("null"),
        Interpretation::Literal { family, .. } | Interpretation::Direct(family) => Some(family),
        Interpretation::IntegralView { .. } => Some(Family::Integral),
        _ => None,
    };
    let word =
        match family {
            Some(Family::Integral | Family::Float) => {
                if bits.width != crate::SlotWidth::One {
                    return Err(RenderError::Invariant(
                        "word literal lost its delivered width",
                    ));
                }
                Some(u32::try_from(bits.value).map_err(|_| {
                    RenderError::Invariant("word literal exceeds its delivered width")
                })?)
            }
            _ => None,
        };
    if matches!(family, Some(Family::Long | Family::Double)) && bits.width != crate::SlotWidth::Two
    {
        return Err(RenderError::Invariant(
            "wide literal lost its delivered width",
        ));
    }
    match (family, word) {
        (Some(Family::Integral), Some(word)) => {
            out.fmt(format_args!("{}", i32::from_le_bytes(word.to_le_bytes())))
        }
        (Some(Family::Long), _) => out.fmt(format_args!(
            "{}L",
            i64::from_le_bytes(bits.value.to_le_bytes())
        )),
        (Some(Family::Float), Some(word)) if f32::from_bits(word).is_finite() => {
            out.fmt(format_args!("{:?}f", f32::from_bits(word)))
        }
        (Some(Family::Double), _) if f64::from_bits(bits.value).is_finite() => {
            out.fmt(format_args!("{:?}d", f64::from_bits(bits.value)))
        }
        _ => out.fmt(format_args!(
            "bits<{:?}; 0x{:x}; {view:?}>",
            bits.width, bits.value
        )),
    }
}
/// One payload element as its exact Java literal. The original bits are never
/// changed: a bit pattern these forms cannot state keeps the payload statement,
/// which P7 selects only for elements that print exactly.
fn element_literal(
    out: &mut Buffer,
    element: crate::p4::ArrayElementType,
    bits: u64,
) -> Result<(), RenderError> {
    use crate::p4::ArrayElementType::*;
    match element {
        Boolean => match bits {
            0 => out.put("false")?,
            1 => out.put("true")?,
            _ => {
                return Err(RenderError::Invariant("boolean element lost its 0/1 value"));
            }
        },
        Byte => out.fmt(format_args!("{}", bits as u8 as i8))?,
        Short => out.fmt(format_args!("{}", bits as u16 as i16))?,
        Char => char_literal(out, bits as u16)?,
        Int => out.fmt(format_args!("{}", bits as u32 as i32))?,
        Long => out.fmt(format_args!("{}L", bits as i64))?,
        Float => {
            let value = f32::from_bits(bits as u32);
            if !value.is_finite() {
                return Err(RenderError::Invariant(
                    "float element has no exact decimal spelling",
                ));
            }
            // The shortest decimal for a finite float round-trips to these bits.
            out.fmt(format_args!("{value:?}f"))?;
        }
        Double => {
            let value = f64::from_bits(bits);
            if !value.is_finite() {
                return Err(RenderError::Invariant(
                    "double element has no exact decimal spelling",
                ));
            }
            out.fmt(format_args!("{value:?}d"))?;
        }
    }
    Ok(())
}

/// One original `char` unit as a single-quoted literal with the project's
/// existing escapes; an isolated surrogate or control unit keeps `\uXXXX`.
fn char_literal(out: &mut Buffer, unit: u16) -> Result<(), RenderError> {
    out.put("'")?;
    if unit == u16::from(b'\'') {
        out.put("\\'")?;
    } else {
        out.units(std::slice::from_ref(&unit), false)?;
    }
    out.put("'")
}
fn unary(op: crate::p2::UnaryOp) -> &'static str {
    use crate::p2::UnaryOp::*;
    match op {
        NegInt | NegLong | NegFloat | NegDouble => "-",
        NotInt | NotLong => "~",
        IntToLong | FloatToLong | DoubleToLong => "(long) ",
        IntToFloat | LongToFloat | DoubleToFloat => "(float) ",
        IntToDouble | LongToDouble | FloatToDouble => "(double) ",
        LongToInt | FloatToInt | DoubleToInt => "(int) ",
        IntToByte => "(byte) ",
        IntToChar => "(char) ",
        IntToShort => "(short) ",
    }
}
fn binary(op: crate::p2::BinaryOp) -> &'static str {
    use crate::p2::BinaryOp::*;
    match op {
        AddInt | AddLong | AddFloat | AddDouble => " + ",
        SubInt | RsubInt | SubLong | SubFloat | SubDouble => " - ",
        MulInt | MulLong | MulFloat | MulDouble => " * ",
        DivInt | DivLong | DivFloat | DivDouble => " / ",
        RemInt | RemLong | RemFloat | RemDouble => " % ",
        AndInt | AndLong => " & ",
        OrInt | OrLong => " | ",
        XorInt | XorLong => " ^ ",
        ShlInt | ShlLong => " << ",
        ShrInt | ShrLong => " >> ",
        UshrInt | UshrLong => " >>> ",
    }
}
