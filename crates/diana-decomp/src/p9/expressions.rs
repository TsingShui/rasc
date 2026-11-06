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
}
impl<S: ExpressionSource> Expressions<'_, '_, S> {
    pub fn assignment(&mut self, s: &Statement) -> Result<(), RenderError> {
        let mut first = true;
        for definition in &s.outputs {
            if let p5::Materialization::Store(local)
            | p5::Materialization::AllocationCarrier(local)
            | p5::Materialization::ExceptionCapture(local) = definition.materialization()
            {
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
                self.out.put("store ")?;
                self.operand(*value)?;
                self.out.put(" -> ")?;
                self.member(*reference, *receiver, true)?;
            }
            StatementKind::ArrayStore {
                array,
                index,
                value,
                kind,
            } => {
                self.out.put("store ")?;
                self.operand(*value)?;
                self.out.put(" -> ")?;
                self.operand(*array)?;
                self.out.put("[")?;
                self.operand(*index)?;
                self.out.put("]")?;
                self.out.fmt(format_args!(" /* {kind:?} store */"))?;
            }
            StatementKind::CheckCast { value, reference } => {
                self.out.put("check_cast ")?;
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
                expression, syntax, ..
            } => {
                let ExprKind::Call(c) = &self.code.expressions()[expression.0 as usize].kind else {
                    return Err(RenderError::Invariant("initialization lost its invocation"));
                };
                let a = self.out.position();
                self.call(c, Some(*syntax))?;
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
                if let Some(composed) = self.code.composed_use(id) {
                    self.expr(composed.expression)?;
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
            } => self.member(*reference, *receiver, true)?,
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
                self.out.put("new_array ")?;
                self.reference(*reference)?;
                self.out.put("[length: ")?;
                self.operand(*length)?;
                self.out.put("]")?;
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
            ExprKind::Call(call) => self.call(call, None)?,
            ExprKind::New {
                reference,
                constructor,
                arguments,
            } => {
                self.out.put("new ")?;
                self.reference(*reference)?;
                self.out.put("(")?;
                self.args_known(arguments)?;
                self.out.put(") [initializer: ")?;
                self.member(*constructor, None, true)?;
                self.out.put("]")?;
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
                let (owner, name) = match m {
                    MemberDeclaration::Field { owner, name, .. }
                    | MemberDeclaration::Method { owner, name, .. } => (*owner, name),
                };
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
                if signature {
                    match m {
                        MemberDeclaration::Field { field_type, .. } => {
                            self.out.put("<")?;
                            self.out
                                .units(&self.code.types()[field_type.0 as usize].units, true)?;
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
            self.member(c.target, c.receiver, false)?;
        }
        if let Some(proto) = c.invocation_prototype {
            self.out.put(" [invocation prototype: ")?;
            self.reference(proto)?;
            self.out.put("]")?;
        }
        self.out.put("(")?;
        self.args(&c.arguments)?;
        self.out.put(")")?;
        let original = self.code.references()[c.target.0 as usize]
            .position
            .reference;
        self.out.fmt(format_args!(" [{:?} ", c.kind))?;
        match original {
            crate::raw::OperandReference::Method(id) => {
                self.out.fmt(format_args!("method#{}", id.0))?
            }
            crate::raw::OperandReference::CallSite(id) => {
                self.out.fmt(format_args!("call_site#{}", id.0))?
            }
            other => self.out.fmt(format_args!("{other:?}"))?,
        }
        if matches!(c.qualification, Qualification::FullSignature) {
            self.out.put("; target ")?;
            self.member(c.target, None, true)?;
        }
        self.out.put("]")
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
                        self.ty(*owner)?;
                        self.out.put(".")?;
                        self.out.name(&name.units, name.form)?;
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
        for step in &bundle.steps {
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
            self.out.put("; ")?;
        }
        for req in &bundle.assignments {
            if let Some(gap) = req.issue() {
                self.out.fmt(format_args!("unavailable_copy<{gap:?}>; "))?;
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
