use crate::p2_flow::Op;
use crate::p3_ssa::{Definition, SsaBody, Sym};
use crate::substrate::owned::push;
use crate::{RequestError, RunContext};

/// Widest source kind that can hold both views, or None if Java cannot.
fn widen(a: Kind, b: Kind) -> Option<Kind> {
    use Kind::*;
    if a == b {
        return Some(a);
    }
    let reference = |kind: Kind| {
        matches!(
            kind,
            Object
                | Throwable
                | Class
                | String
                | PrimitiveArray(_)
                | ObjectArray
                | Nominal(_)
                | ReferenceArray(_)
                | ProjectedNominal(_)
        )
    };
    match (a, b) {
        (Int, Boolean) | (Boolean, Int) => Some(Int),
        (Long, Int) | (Int, Long) => Some(Long),
        (Float, Int) | (Int, Float) => Some(Float),
        (Double, Int) | (Int, Double) => Some(Double),
        _ if reference(a) && reference(b) => Some(Object),
        _ => None,
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum Kind {
    Long,
    Double,
    Int,
    Boolean,
    Object,
    Throwable,
    Class,
    String,
    Float,
    PrimitiveArray(super::PrimitiveArray),
    ObjectArray,
    Nominal(crate::TypeId),
    ReferenceArray(crate::input::TypeProjection),
    ProjectedNominal(crate::input::TypeProjection),
}
impl Kind {
    pub(crate) fn builtin_spelling(self) -> Option<&'static str> {
        Some(match self {
            Self::Int => "int",
            Self::Boolean => "boolean",
            Self::Object => "java.lang.Object",
            Self::Throwable => "java.lang.Throwable",
            Self::Class => "java.lang.Class",
            Self::String => "java.lang.String",
            Self::Float => "float",
            Self::Long => "long",
            Self::Double => "double",
            Self::PrimitiveArray(kind) => kind.spelling(),
            Self::ObjectArray => "java.lang.Object[]",
            Self::Nominal(_) | Self::ReferenceArray(_) | Self::ProjectedNominal(_) => return None,
        })
    }
}
crate::substrate::checked::checked_product!(TypedBody, TypedBodyData);
pub(crate) struct TypedBodyData<'a> {
    _sealed: (),
    pub ssa: SsaBody<'a>,
    pub roots: super::reference_roots::ReferenceRoots,
    pub zeros: super::zero::ZeroFacts,
    pub kinds: Vec<Option<Kind>>,
    pub integral_bits: Vec<u8>,
    pub ranges: Vec<Option<crate::p4_types::IntRange>>,
    pub arrays: Vec<Option<ArrayEvidence>>,
    pub bindings: Vec<crate::p4_types::bindings::CallEvidence>,
    pub source_throws: Option<crate::p4_types::bindings::SourceThrows>,
    pub exceptions: Vec<crate::p4_types::declarations::exceptions::ExceptionBinding>,
    pub call_uses: Vec<crate::p4_types::calls::CallUses>,
    pub cast_uses: Vec<Option<crate::p4_types::calls::TypedUse>>,
    pub numeric_uses: Vec<Option<super::numeric::NumericUses>>,
    pub returned_uses: Vec<Option<UseConversion>>,
    pub thrown_uses: Vec<Option<UseConversion>>,
    pub initializations: super::initialization::Initializations,
    pub capture: Option<super::capture::CaptureUse>,
    pub empty_fills: Vec<PrimitiveFillUse>,
    pub array_lengths: Vec<crate::p4_types::calls::TypedUse>,
    pub filled_arrays: Vec<FilledArrayUse>,
    pub fresh_fills: Vec<PrimitiveFillUse>,

    pub field_declarations: Vec<Option<crate::p4_types::bindings::FieldEvidence>>,
    pub field_uses: Vec<Option<FieldUse>>,
}
pub(crate) struct FilledArrayUse {
    _sealed: (),
    pub kind: Kind,
    pub arguments: Vec<crate::p4_types::calls::TypedUse>,
}
pub(crate) struct PrimitiveFillUse {
    _sealed: (),
    pub array: crate::p3_ssa::ValueId,
    pub component: super::PrimitiveArray,
}
pub(crate) enum FieldUse {
    Read,
    Write(UseConversion),
    WriteThis(UseConversion),
}
impl TypedBody<'_> {
    pub(crate) fn contract(
        &self,
        id: crate::p2_flow::CallId,
    ) -> Result<&crate::p4_types::declarations::StaticContract, RequestError> {
        crate::p4_types::bindings::contract(&self.ssa, &self.bindings, id)
    }
}
#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum UseConversion {
    Identity,
    FloatLiteral(u32),
    DoubleLiteral(u64),
    BooleanWord,
    Boolean01,
    NullAssignment,
    UpcastObject,
    Narrow(crate::p4_types::Narrow),
}
#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum ArrayReceiver {
    Direct,
    ObjectView,
}
pub(crate) enum ArrayUse {
    Length,
    Load {
        index: UseConversion,
    },
    Store {
        index: UseConversion,
        value: UseConversion,
        receiver: ArrayReceiver,
    },
}
pub(crate) struct ArrayEvidence {
    _sealed: (),
    pub usage: ArrayUse,
}
fn ix(value: u32) -> Result<usize, RequestError> {
    usize::try_from(value).map_err(|_| RequestError::PlatformCapacity)
}
fn merge_range(
    target: &mut Option<crate::p4_types::IntRange>,
    source: Option<crate::p4_types::IntRange>,
) -> bool {
    let merged = match (*target, source) {
        (None, value) => value,
        (value, None) => value,
        (Some(a), Some(b)) => Some(a.hull(b)),
    };
    if merged == *target {
        false
    } else {
        *target = merged;
        true
    }
}
pub(crate) fn solve<'a>(
    schedule: &crate::engine::SchedulePermit,
    input: &crate::input::MethodSource<'_, '_>,
    context: &crate::context::Context,
    ssa: SsaBody<'a>,
    capture: Option<&crate::CaptureCertificate>,
    budget: &mut RunContext<'_>,
) -> Result<crate::substrate::outcome::Outcome<TypedBody<'a>>, RequestError> {
    let resolved = super::bindings::collect(schedule, input, context, ssa, capture, budget)?;
    solve_impl(input, resolved, capture, budget)
}

fn solve_impl<'a>(
    input: &crate::input::MethodSource<'_, '_>,
    resolved: crate::p4_types::bindings::ResolvedBody<'a>,
    capture: Option<&crate::CaptureCertificate>,
    budget: &mut RunContext<'_>,
) -> Result<crate::substrate::outcome::Outcome<TypedBody<'a>>, RequestError> {
    let resolved = resolved.into_data();
    let ssa = resolved.ssa;
    if let Some(certificate) = capture {
        certificate.check(input, budget)?;
    }
    let capture = super::capture::prove(&ssa, capture, budget)?;
    let bindings = resolved.calls;
    let source_throws = resolved.source_throws;
    let exceptions = resolved.exceptions;
    let field_declarations = resolved.fields;
    let array_components = super::array_components::collect(input, &ssa, budget)?;
    let zeros = super::zero::ZeroFacts::build(&ssa, budget)?;
    let mut integral_bits = Vec::new();
    let mut ranges = Vec::new();
    let mut kinds = Vec::new();
    for definition in &ssa.values {
        budget.checkpoint()?;
        let (bits, range, kind) = match *definition {
            Definition::Parameter(index) => {
                let declaration = ssa.parameters[ix(index)?];
                let kind = declaration.kind();
                let bits = match kind {
                    Kind::Boolean => 3,
                    Kind::Int => 7,
                    Kind::Object
                    | Kind::String
                    | Kind::Throwable
                    | Kind::Class
                    | Kind::Float
                    | Kind::Long
                    | Kind::Double
                    | Kind::PrimitiveArray(_)
                    | Kind::ObjectArray
                    | Kind::Nominal(_)
                    | Kind::ReferenceArray(_)
                    | Kind::ProjectedNominal(_) => 0,
                };
                (bits, declaration.range(), Some(kind))
            }
            Definition::OpResult(id) => match ssa.operation(id)?.op {
                Op::ConstantWide(..) | Op::LongBinary(..) => (0, None, Some(Kind::Long)),
                Op::DoubleBinary(..) => (0, None, Some(Kind::Double)),
                Op::FloatBinary(..) => (0, None, Some(Kind::Float)),
                Op::Unary(_, unary, _)
                    if unary.signature().1 != crate::p2_flow::primitives::ScalarType::Int =>
                {
                    (
                        0,
                        None,
                        Some(super::numeric::scalar_kind(unary.signature().1)),
                    )
                }
                Op::Unary(_, unary, _) => (
                    7,
                    Some(match unary {
                        crate::p2_flow::primitives::Unary::Byte => super::Narrow::Byte.range(),
                        crate::p2_flow::primitives::Unary::Char => super::Narrow::Char.range(),
                        crate::p2_flow::primitives::Unary::Short => super::Narrow::Short.range(),
                        _ => super::IntRange::ALL,
                    }),
                    Some(Kind::Int),
                ),
                Op::ReverseSubtract(..) => (7, Some(super::IntRange::ALL), Some(Kind::Int)),
                Op::NewInstance(_, id) => (
                    0,
                    None,
                    Some(Kind::Nominal(
                        ssa.object_creations
                            .get(ix(id.0)?)
                            .ok_or(RequestError::InternalInvariant(
                                "object allocation lost type",
                            ))?
                            .reference,
                    )),
                ),
                Op::NewArray(_, _, id) => (
                    0,
                    None,
                    Some(
                        ssa.array_creations
                            .get(ix(id.0)?)
                            .ok_or(RequestError::InternalInvariant(
                                "array allocation lost type",
                            ))?
                            .component
                            .kind(),
                    ),
                ),
                Op::Cast(_, id) => (
                    0,
                    None,
                    Some(
                        ssa.casts
                            .get(ix(id.0)?)
                            .ok_or(RequestError::InternalInvariant("cast lost target type"))?
                            .kind
                            .kind(),
                    ),
                ),
                Op::InstanceOf(..) => (3, Some(super::IntRange::BOOLEAN), Some(Kind::Boolean)),
                Op::FilledArray(id, _) => {
                    (0, None, Some(ssa.filled_arrays[ix(id.0)?].component.kind()))
                }
                Op::String(..) => (0, None, Some(Kind::String)),
                Op::Class(..) => (0, None, Some(Kind::Class)),
                Op::Constant(_, value) => (
                    match value {
                        0 => 1,
                        1 => 2,
                        _ => 4,
                    },
                    Some(crate::p4_types::IntRange::literal(value)),
                    Some(Kind::Int),
                ),
                Op::Binary(
                    _,
                    _,
                    crate::p2_flow::Arithmetic::And
                    | crate::p2_flow::Arithmetic::Or
                    | crate::p2_flow::Arithmetic::Xor,
                    _,
                ) => (0, None, Some(Kind::Int)),
                Op::FieldLoad(_, _, _, kind) | Op::StaticLoad(_, _, kind) => (
                    match kind.kind() {
                        Kind::Int => 7,
                        Kind::Boolean => 3,
                        _ => 0,
                    },
                    kind.range(),
                    Some(kind.kind()),
                ),
                Op::ArrayLength(..) | Op::Binary(_, _, _, _) | Op::Compare(_, _, _, _) => {
                    (7, Some(crate::p4_types::IntRange::ALL), Some(Kind::Int))
                }
                Op::Call(call, _) => {
                    let declaration = match ssa.calls[ix(call.0)?].result {
                        crate::p2_flow::InvokeResult::Word(kind)
                        | crate::p2_flow::InvokeResult::Wide(kind)
                        | crate::p2_flow::InvokeResult::Reference(Some(kind)) => kind,
                        _ => {
                            return Err(RequestError::InternalInvariant(
                                "unsupported raw call result entered normal typing",
                            ));
                        }
                    };
                    let kind = declaration.kind();
                    let bits = match kind {
                        Kind::Int => 7,
                        Kind::Boolean => 3,
                        _ => 0,
                    };
                    (bits, declaration.range(), Some(kind))
                }
                Op::ArrayLoad(_, _, _, element) => {
                    let operation = ssa.operation(id)?;
                    let Some(component) =
                        array_components.component(input, operation.input(0)?, budget)?
                    else {
                        return Ok(crate::substrate::outcome::Outcome::Declined(
                            crate::OutputReason::JavaCheckFailed {
                                code: crate::DiagnosticCode::JavaAnalysisUnavailable,
                                phase: "P4",
                                check: "array_component_kind",
                                pc: None,
                            },
                        ));
                    };
                    let kind = component.kind();
                    if !element.accepts(component) {
                        return Err(crate::input::MethodError::Malformed {
                            pc: operation.pc,
                            problem: crate::input::MethodProblem::InvalidArrayAccessType,
                        }
                        .into());
                    }
                    (
                        match kind {
                            Kind::Int => 7,
                            Kind::Boolean => 3,
                            _ => 0,
                        },
                        component.range(),
                        Some(kind),
                    )
                }
                Op::Move(_, _) | Op::MoveWide(_, _) => (0, None, None),
                Op::Exception(_) => {
                    return Ok(crate::substrate::outcome::Outcome::Declined(
                        crate::OutputReason::JavaCheckFailed {
                            code: crate::DiagnosticCode::JavaAnalysisUnavailable,
                            phase: "P4",
                            check: "exception_definition",
                            pc: None,
                        },
                    ));
                }
                Op::Monitor(..)
                | Op::Nop
                | Op::ArrayStore(..)
                | Op::FieldStore(..)
                | Op::StaticStore(..)
                | Op::EmptyFill(..)
                | Op::FreshFill(..)
                | Op::Initialize(..) => {
                    return Err(RequestError::InternalInvariant(
                        "NOP acquired a normal value definition",
                    ));
                }
            },
            Definition::Phi => (0, None, None),
            Definition::ExceptionAt(id) => {
                let mut kind = None;
                for binding in &exceptions {
                    budget.checkpoint()?;
                    if binding.operation == id {
                        kind = binding.kind.map(|kind| match kind {
                            crate::p4_types::declarations::exceptions::ExceptionType::Nominal(
                                id,
                            ) => Kind::Nominal(id),
                            crate::p4_types::declarations::exceptions::ExceptionType::Throwable => {
                                Kind::Throwable
                            }
                        });
                    }
                }
                let Some(kind) = kind else {
                    return Ok(crate::substrate::outcome::Outcome::Declined(
                        crate::OutputReason::JavaCheckFailed {
                            code: crate::DiagnosticCode::JavaAnalysisUnavailable,
                            phase: "P4",
                            check: "definition_kind",
                            pc: None,
                        },
                    ));
                };
                (0, None, Some(kind))
            }
        };
        push(&mut integral_bits, bits, budget)?;
        push(&mut ranges, range, budget)?;
        push(&mut kinds, kind, budget)?;
    }
    let numeric_valid =
        super::numeric::constrain(input, &ssa, &mut kinds, &array_components, budget)?;
    let mut type_conflict = false;
    loop {
        let mut changed = false;
        for op in &ssa.operations {
            budget.checkpoint()?;
            if matches!(op.op, Op::Move(..) | Op::MoveWide(..)) {
                let index = ix(op
                    .result
                    .ok_or(RequestError::InternalInvariant("move lost its result"))?
                    .0)?;
                let source = ix(op.input(0)?.0)?;
                let source_range =
                    if matches!(kinds[index], Some(Kind::Float | Kind::Long | Kind::Double)) {
                        None
                    } else {
                        ranges[source]
                    };
                changed |= merge_range(&mut ranges[index], source_range);
                let merged =
                    if matches!(kinds[index], Some(Kind::Float | Kind::Long | Kind::Double)) {
                        0
                    } else {
                        integral_bits[index] | integral_bits[source]
                    };
                if merged != integral_bits[index] {
                    integral_bits[index] = merged;
                    changed = true;
                }
                if kinds[index].is_none() && kinds[source].is_some() {
                    kinds[index] = kinds[source];
                    changed = true;
                }
            }
        }
        for op in &ssa.operations {
            budget.checkpoint()?;
            let Op::Binary(_, _, arithmetic, right) = op.op else {
                continue;
            };
            if !matches!(
                arithmetic,
                crate::p2_flow::Arithmetic::And
                    | crate::p2_flow::Arithmetic::Or
                    | crate::p2_flow::Arithmetic::Xor
            ) {
                continue;
            }
            let left = integral_bits[ix(op.input(0)?.0)?];
            let right = match right {
                crate::p2_flow::Operand::Register(_) => integral_bits[ix(op.input(1)?.0)?],
                crate::p2_flow::Operand::Literal(0) => 1,
                crate::p2_flow::Operand::Literal(1) => 2,
                crate::p2_flow::Operand::Literal(_) => 4,
            };
            let known = |bits: u8| bits != 0 && bits & 4 == 0;
            let fact = if matches!(arithmetic, crate::p2_flow::Arithmetic::And)
                && (known(left) || known(right))
                || known(left) && known(right)
            {
                3
            } else if left != 0 && right != 0 {
                7
            } else {
                0
            };
            let index = ix(op
                .result
                .ok_or(RequestError::InternalInvariant(
                    "bitwise operation lost result",
                ))?
                .0)?;
            let merged = integral_bits[index] | fact;
            if merged != integral_bits[index] {
                integral_bits[index] = merged;
                changed = true;
            }
            if fact != 0 {
                changed |= merge_range(
                    &mut ranges[index],
                    Some(if fact & 4 == 0 {
                        super::IntRange::BOOLEAN
                    } else {
                        super::IntRange::ALL
                    }),
                );
            }
        }
        for phi in &ssa.phis {
            budget.checkpoint()?;
            let index = ix(phi.result.0)?;
            for &(_, input) in &phi.inputs {
                budget.checkpoint()?;
                let Sym::Value(source) = input else {
                    return Err(RequestError::InternalInvariant(
                        "unavailable Phi entered type solving",
                    ));
                };
                // A proven word-zero has independent integer/null use views.
                // It must not constrain a reference Phi to an integer kind.
                if zeros.contains(source)? {
                    changed |= merge_range(&mut ranges[index], Some(super::IntRange::literal(0)));
                    if integral_bits[index] & 1 == 0 {
                        integral_bits[index] |= 1;
                        changed = true;
                    }
                    continue;
                }
                let source = ix(source.0)?;
                let source_range = ranges[source];
                changed |= merge_range(&mut ranges[index], source_range);
                let merged = integral_bits[index] | integral_bits[source];
                if merged != integral_bits[index] {
                    integral_bits[index] = merged;
                    changed = true;
                }
                if let Some(kind) = kinds[source] {
                    if let Some(old) = kinds[index] {
                        if old != kind {
                            // Proven zero inputs were handled separately above.
                            // Other inputs require a common storage representation.
                            let widened = widen(old, kind);
                            match widened {
                                Some(kind) if Some(kind) != kinds[index] => {
                                    kinds[index] = Some(kind);
                                    changed = true;
                                }
                                Some(_) => {}
                                None => type_conflict = true,
                            }
                        }
                    } else {
                        kinds[index] = Some(kind);
                        changed = true;
                    }
                }
            }
        }
        if !changed {
            break;
        }
    }
    for (index, kind) in kinds.iter_mut().enumerate() {
        budget.checkpoint()?;
        if kind.is_none()
            && zeros.contains(crate::p3_ssa::ValueId(
                u32::try_from(index).map_err(|_| RequestError::PlatformCapacity)?,
            ))?
        {
            *kind = Some(Kind::Int);
        }
    }
    if type_conflict {
        return Ok(crate::substrate::outcome::Outcome::Declined(
            crate::OutputReason::JavaCheckFailed {
                code: crate::DiagnosticCode::JavaAnalysisUnavailable,
                phase: "P4",
                check: "type_conflict",
                pc: None,
            },
        ));
    }
    let Some(arrays) = solve_arrays(input, &ssa, &zeros, &kinds, &integral_bits, budget)? else {
        return Ok(crate::substrate::outcome::Outcome::Declined(
            crate::OutputReason::JavaCheckFailed {
                code: crate::DiagnosticCode::JavaAnalysisUnavailable,
                phase: "P4",
                check: "array_solve",
                pc: None,
            },
        ));
    };
    if !numeric_valid {
        return Ok(crate::substrate::outcome::Outcome::Declined(
            crate::OutputReason::JavaCheckFailed {
                code: crate::DiagnosticCode::JavaAnalysisUnavailable,
                phase: "P4",
                check: "numeric_valid",
                pc: None,
            },
        ));
    }
    let Some(numeric_uses) = super::numeric::solve(&ssa, &kinds, budget)? else {
        return Ok(crate::substrate::outcome::Outcome::Declined(
            crate::OutputReason::JavaCheckFailed {
                code: crate::DiagnosticCode::JavaAnalysisUnavailable,
                phase: "P4",
                check: "numeric_uses",
                pc: None,
            },
        ));
    };
    let initializations = super::initialization::solve(&ssa, capture.as_ref(), budget)?;
    let Some(array_uses) = solve_new_arrays(&ssa, &kinds, budget)? else {
        return Ok(crate::substrate::outcome::Outcome::Declined(
            crate::OutputReason::JavaCheckFailed {
                code: crate::DiagnosticCode::JavaAnalysisUnavailable,
                phase: "P4",
                check: "new_array_solve",
                pc: None,
            },
        ));
    };
    let Some(filled_arrays) =
        solve_filled_arrays(input, &ssa, &zeros, &kinds, &integral_bits, budget)?
    else {
        return Ok(crate::substrate::outcome::Outcome::Declined(
            crate::OutputReason::JavaCheckFailed {
                code: crate::DiagnosticCode::JavaAnalysisUnavailable,
                phase: "P4",
                check: "filled_array_solve",
                pc: None,
            },
        ));
    };
    let Some(empty_fills) = solve_empty_fills(&ssa, &kinds, budget)? else {
        return Ok(crate::substrate::outcome::Outcome::Declined(
            crate::OutputReason::JavaCheckFailed {
                code: crate::DiagnosticCode::JavaAnalysisUnavailable,
                phase: "P4",
                check: "empty_fill_solve",
                pc: None,
            },
        ));
    };
    let Some(cast_uses) = solve_casts(&ssa, &zeros, &kinds, budget)? else {
        return Ok(crate::substrate::outcome::Outcome::Declined(
            crate::OutputReason::JavaCheckFailed {
                code: crate::DiagnosticCode::JavaAnalysisUnavailable,
                phase: "P4",
                check: "cast_uses",
                pc: None,
            },
        ));
    };
    let Some(field_uses) = solve_fields(
        &ssa,
        &zeros,
        &field_declarations,
        &kinds,
        &integral_bits,
        budget,
    )?
    else {
        return Ok(crate::substrate::outcome::Outcome::Declined(
            crate::OutputReason::JavaCheckFailed {
                code: crate::DiagnosticCode::JavaAnalysisUnavailable,
                phase: "P4",
                check: "field_uses",
                pc: None,
            },
        ));
    };
    let Some(call_uses) = super::calls::solve(
        &ssa,
        &zeros,
        &initializations,
        &kinds,
        &integral_bits,
        &ranges,
        budget,
    )?
    else {
        return Ok(crate::substrate::outcome::Outcome::Declined(
            crate::OutputReason::JavaCheckFailed {
                code: crate::DiagnosticCode::JavaAnalysisUnavailable,
                phase: "P4",
                check: "call_uses",
                pc: None,
            },
        ));
    };
    for (index, block) in ssa.blocks.iter().enumerate() {
        budget.checkpoint()?;
        if let crate::p2_flow::Terminal::Test { opcode, .. } = block.terminal
            && matches!(opcode, 0x34..=0x37 | 0x3a..=0x3d)
        {
            let value = ssa.bound[index]
                .terminal
                .ok_or(RequestError::InternalInvariant(
                    "typed condition lost operand",
                ))?;
            if kinds[ix(value.0)?].is_some_and(super::reference_roots::reference) {
                return Err(crate::input::MethodError::Malformed {
                    pc: block.terminal_pc,
                    problem: crate::input::MethodProblem::InvalidTypeOperand,
                }
                .into());
            }
        }
    }
    let roots = super::reference_roots::solve(&ssa, &kinds, budget)?;
    for op in &ssa.operations {
        budget.checkpoint()?;
        if matches!(op.op, Op::Monitor(..)) {
            let value = op.input(0)?;
            if !kinds[ix(value.0)?].is_some_and(super::reference_roots::reference)
                && roots.one(value)? != Some(super::reference_roots::Root::Null)
            {
                return Err(crate::input::MethodError::Malformed {
                    pc: op.pc,
                    problem: crate::input::MethodProblem::InvalidTypeOperand,
                }
                .into());
            }
        }
    }
    let mut typed = TypedBody(TypedBodyData {
        capture,
        source_throws,
        roots,
        zeros,
        _sealed: (),
        ssa,
        kinds,
        integral_bits,
        ranges,
        arrays,
        bindings,
        exceptions,
        call_uses,
        cast_uses,
        numeric_uses,
        returned_uses: Vec::new(),
        thrown_uses: Vec::new(),
        initializations,
        empty_fills,
        array_lengths: array_uses.lengths,
        filled_arrays,
        fresh_fills: array_uses.fills,
        field_declarations,
        field_uses,
    });
    for _ in &typed.0.ssa.values {
        budget.checkpoint()?;
        push(&mut typed.0.returned_uses, None, budget)?;
    }
    for (index, block) in typed.0.ssa.blocks.iter().enumerate() {
        budget.checkpoint()?;
        let thrown = if matches!(block.terminal, crate::p2_flow::Terminal::Throw(_)) {
            let value = typed.ssa.bound[index]
                .terminal
                .ok_or(RequestError::InternalInvariant("typed throw lost value"))?;
            Some(if typed.zeros.contains(value)? {
                UseConversion::NullAssignment
            } else if matches!(
                typed.kinds[ix(value.0)?],
                Some(Kind::Nominal(_) | Kind::Throwable)
            ) {
                UseConversion::Identity
            } else {
                return Ok(crate::substrate::outcome::Outcome::Declined(
                    crate::OutputReason::JavaCheckFailed {
                        code: crate::DiagnosticCode::JavaAnalysisUnavailable,
                        phase: "P4",
                        check: "thrown_use",
                        pc: None,
                    },
                ));
            })
        } else {
            None
        };
        push(&mut typed.0.thrown_uses, thrown, budget)?;
        if matches!(block.terminal, crate::p2_flow::Terminal::Return(_)) {
            let value = typed.ssa.bound[index]
                .terminal
                .ok_or(RequestError::InternalInvariant("typed return lost value"))?;
            let Some(conversion) = return_use(&typed, value)? else {
                return Ok(crate::substrate::outcome::Outcome::Declined(
                    crate::OutputReason::JavaCheckFailed {
                        code: crate::DiagnosticCode::JavaAnalysisUnavailable,
                        phase: "P4",
                        check: "return_use",
                        pc: None,
                    },
                ));
            };
            typed.0.returned_uses[ix(value.0)?] = Some(conversion);
        }
    }
    Ok(crate::substrate::outcome::Outcome::Ready(typed))
}
fn solve_arrays(
    input: &crate::input::MethodSource<'_, '_>,
    ssa: &SsaBody<'_>,
    zeros: &super::zero::ZeroFacts,
    kinds: &[Option<Kind>],
    bits: &[u8],
    budget: &mut RunContext<'_>,
) -> Result<Option<Vec<Option<ArrayEvidence>>>, RequestError> {
    let mut result = Vec::new();
    for op in &ssa.operations {
        budget.checkpoint()?;
        if !matches!(
            op.op,
            Op::ArrayLength(..) | Op::ArrayLoad(..) | Op::ArrayStore(..)
        ) {
            push(&mut result, None, budget)?;
            continue;
        }
        let bad = || {
            RequestError::from(crate::input::MethodError::Malformed {
                pc: op.pc,
                problem: crate::input::MethodProblem::InvalidArrayAccessType,
            })
        };
        let component = match kinds[ix(op.input(0)?.0)?] {
            Some(Kind::PrimitiveArray(kind)) => kind.value_type(),
            Some(Kind::ObjectArray) => crate::p2_flow::DeclarationType::Object,
            Some(Kind::ReferenceArray(key)) => {
                crate::p2_flow::ArrayKind::Reference(key).element_type(input, budget)?
            }
            _ => return Ok(None),
        };
        let usage = if matches!(op.op, Op::ArrayLength(..)) {
            ArrayUse::Length
        } else {
            let element = match op.op {
                Op::ArrayLoad(_, _, _, element) | Op::ArrayStore(_, _, _, element) => element,
                _ => {
                    return Err(RequestError::InternalInvariant(
                        "array access lost element kind",
                    ));
                }
            };
            if !element.accepts(component) {
                return Err(bad());
            }
            let index = match kinds[ix(op.input(1)?.0)?] {
                Some(Kind::Int) => UseConversion::Identity,
                Some(Kind::Boolean) => UseConversion::BooleanWord,
                None => return Ok(None),
                _ => return Err(bad()),
            };
            if matches!(op.op, Op::ArrayLoad(..)) {
                ArrayUse::Load { index }
            } else {
                let source = op.input(2)?;
                let receiver =
                    if matches!(kinds[ix(op.input(0)?.0)?], Some(Kind::ReferenceArray(_))) {
                        ArrayReceiver::ObjectView
                    } else {
                        ArrayReceiver::Direct
                    };
                let value =
                    if component.kind() == Kind::Object || receiver == ArrayReceiver::ObjectView {
                        let Some(conversion) =
                            super::references::view(kinds, zeros, source, Kind::Object)?
                        else {
                            return Ok(None);
                        };
                        conversion
                    } else {
                        let Some(view) =
                            super::primitive_stores::view(ssa, kinds, bits, source, component)?
                        else {
                            return Ok(None);
                        };
                        view
                    };
                ArrayUse::Store {
                    index,
                    value,
                    receiver,
                }
            }
        };
        push(
            &mut result,
            Some(ArrayEvidence { _sealed: (), usage }),
            budget,
        )?;
    }
    Ok(Some(result))
}

fn solve_fields(
    ssa: &SsaBody<'_>,
    zeros: &super::zero::ZeroFacts,
    declarations: &[Option<crate::p4_types::bindings::FieldEvidence>],
    kinds: &[Option<Kind>],
    bits: &[u8],
    budget: &mut RunContext<'_>,
) -> Result<Option<Vec<Option<FieldUse>>>, RequestError> {
    let mut uses = Vec::new();
    for op in &ssa.operations {
        budget.checkpoint()?;
        let reference = match op.op {
            Op::FieldLoad(_, _, id, _)
            | Op::FieldStore(_, _, id, _)
            | Op::StaticLoad(_, id, _)
            | Op::StaticStore(_, id, _) => id,
            _ => {
                push(&mut uses, None, budget)?;
                continue;
            }
        };
        let evidence = declarations
            .get(ix(op.id.0)?)
            .and_then(Option::as_ref)
            .ok_or(RequestError::InternalInvariant(
                "field operation lost original constraints",
            ))?;
        if evidence.reference != reference {
            return Err(RequestError::InternalInvariant(
                "field constraints changed reference",
            ));
        }
        let is_static = evidence.scope == crate::p4_types::bindings::FieldScope::Static;
        if !is_static && kinds[ix(op.input(0)?.0)?] != Some(Kind::Nominal(evidence.owner)) {
            return Ok(None);
        }
        let usage = if matches!(op.op, Op::FieldLoad(..) | Op::StaticLoad(..)) {
            FieldUse::Read
        } else {
            let kind = match op.op {
                Op::FieldStore(_, _, _, kind) | Op::StaticStore(_, _, kind) => kind,
                _ => return Err(RequestError::InternalInvariant("field store lost kind")),
            };
            let source = op.input(usize::from(!is_static))?;
            let conversion = if kind.field_offset() == 2 {
                super::references::view(kinds, zeros, source, kind.kind())?
            } else {
                super::primitive_stores::view(ssa, kinds, bits, source, kind)?
            };
            let Some(conversion) = conversion else {
                return Ok(None);
            };
            if evidence.scope == crate::p4_types::bindings::FieldScope::ConstructorThis {
                if !super::calls::is_this(ssa, op.input(0)?, budget)? {
                    return Err(RequestError::FieldBindingEvidenceMissing {
                        reference,
                        pc: op.pc,
                    });
                }
                FieldUse::WriteThis(conversion)
            } else {
                FieldUse::Write(conversion)
            }
        };
        push(&mut uses, Some(usage), budget)?;
    }
    Ok(Some(uses))
}

fn solve_casts(
    ssa: &SsaBody<'_>,
    zeros: &super::zero::ZeroFacts,
    kinds: &[Option<Kind>],
    budget: &mut RunContext<'_>,
) -> Result<Option<Vec<Option<crate::p4_types::calls::TypedUse>>>, RequestError> {
    let mut uses = Vec::new();
    for op in &ssa.operations {
        budget.checkpoint()?;
        if !matches!(op.op, Op::Cast(..) | Op::InstanceOf(..)) {
            push(&mut uses, None, budget)?;
            continue;
        }
        let value = op.input(0)?;
        let Some(conversion) = super::references::view(kinds, zeros, value, Kind::Object)? else {
            return Ok(None);
        };
        push(
            &mut uses,
            Some(crate::p4_types::calls::TypedUse { value, conversion }),
            budget,
        )?;
    }
    Ok(Some(uses))
}

fn solve_filled_arrays(
    input: &crate::input::MethodSource<'_, '_>,
    ssa: &SsaBody<'_>,
    zeros: &super::zero::ZeroFacts,
    kinds: &[Option<Kind>],
    bits: &[u8],
    budget: &mut RunContext<'_>,
) -> Result<Option<Vec<FilledArrayUse>>, RequestError> {
    if ssa.filled_arrays.is_empty() {
        return Ok(Some(Vec::new()));
    }
    let mut result = Vec::new();
    for op in &ssa.operations {
        budget.checkpoint()?;
        let Op::FilledArray(id, _) = op.op else {
            continue;
        };
        if ix(id.0)? != result.len() {
            return Err(RequestError::InternalInvariant(
                "filled arrays changed identity",
            ));
        }
        let site = ssa
            .filled_arrays
            .get(ix(id.0)?)
            .ok_or(RequestError::InternalInvariant("filled array lost type"))?;
        if site.registers.len() != op.inputs.len() {
            return Err(RequestError::InternalInvariant(
                "filled array changed input count",
            ));
        }
        // ART executes non-int primitive filled forms as runtime errors, not array initializers.
        if matches!(site.component, crate::p2_flow::ArrayKind::Primitive(kind) if kind != crate::p4_types::PrimitiveArray::Int)
        {
            return Ok(None);
        }
        let target = site.component.element_type(input, budget)?;
        let mut arguments = Vec::new();
        for &value in &op.inputs {
            budget.checkpoint()?;
            let conversion = if target.field_offset() == 2 {
                super::references::view(kinds, zeros, value, target.kind())?
            } else {
                super::primitive_stores::view(ssa, kinds, bits, value, target)?
            };
            let Some(conversion) = conversion else {
                return Ok(None);
            };
            push(
                &mut arguments,
                crate::p4_types::calls::TypedUse { value, conversion },
                budget,
            )?;
        }
        push(
            &mut result,
            FilledArrayUse {
                _sealed: (),
                kind: site.component.kind(),
                arguments,
            },
            budget,
        )?;
    }
    Ok(Some(result))
}
fn solve_empty_fills(
    ssa: &SsaBody<'_>,
    kinds: &[Option<Kind>],
    budget: &mut RunContext<'_>,
) -> Result<Option<Vec<PrimitiveFillUse>>, RequestError> {
    let mut uses = Vec::new();
    for op in &ssa.operations {
        budget.checkpoint()?;
        let Op::EmptyFill(_, id) = op.op else {
            continue;
        };
        if ix(id.0)? != uses.len() {
            return Err(RequestError::InternalInvariant(
                "empty fill changed arena identity",
            ));
        }
        let site = ssa
            .empty_fills
            .get(ix(id.0)?)
            .ok_or(RequestError::InternalInvariant(
                "fill lost original payload",
            ))?;
        let array = op.input(0)?;
        let component = match kinds[ix(array.0)?] {
            Some(Kind::PrimitiveArray(component)) if component.width() == site.width => component,
            None => return Ok(None),
            _ if ssa.is_zero(array)? => return Ok(None),
            _ => {
                return Err(crate::input::MethodError::Malformed {
                    pc: op.pc,
                    problem: crate::input::MethodProblem::InvalidArrayPayloadType,
                }
                .into());
            }
        };
        push(
            &mut uses,
            PrimitiveFillUse {
                _sealed: (),
                array,
                component,
            },
            budget,
        )?;
    }
    Ok(Some(uses))
}

struct ArrayUses {
    lengths: Vec<crate::p4_types::calls::TypedUse>,
    fills: Vec<PrimitiveFillUse>,
}
fn solve_new_arrays(
    ssa: &SsaBody<'_>,
    kinds: &[Option<Kind>],
    budget: &mut RunContext<'_>,
) -> Result<Option<ArrayUses>, RequestError> {
    let mut lengths = Vec::new();
    let mut fills = Vec::new();
    for op in &ssa.operations {
        budget.checkpoint()?;
        match op.op {
            Op::NewArray(_, _, id) => {
                if ix(id.0)? != lengths.len() {
                    return Err(RequestError::InternalInvariant(
                        "array creation changed identity",
                    ));
                }
                let value = op.input(0)?;
                let conversion = match kinds[ix(value.0)?] {
                    Some(Kind::Int) => UseConversion::Identity,
                    Some(Kind::Boolean) => UseConversion::BooleanWord,
                    None => return Ok(None),
                    _ => {
                        return Err(crate::input::MethodError::Malformed {
                            pc: op.pc,
                            problem: crate::input::MethodProblem::InvalidTypeOperand,
                        }
                        .into());
                    }
                };
                push(
                    &mut lengths,
                    crate::p4_types::calls::TypedUse { value, conversion },
                    budget,
                )?;
            }
            Op::FreshFill(_, id) => {
                if ix(id.0)? != fills.len() {
                    return Err(RequestError::InternalInvariant(
                        "fresh fill changed identity",
                    ));
                }
                let site = ssa
                    .fresh_fills
                    .get(ix(id.0)?)
                    .ok_or(RequestError::InternalInvariant("fresh fill lost payload"))?;
                let array = op.input(0)?;
                let component = match kinds[ix(array.0)?] {
                    Some(Kind::PrimitiveArray(component)) if component.width() == site.width => {
                        component
                    }
                    None => return Ok(None),
                    _ if ssa.is_zero(array)? => return Ok(None),
                    _ => {
                        return Err(crate::input::MethodError::Malformed {
                            pc: op.pc,
                            problem: crate::input::MethodProblem::InvalidArrayPayloadType,
                        }
                        .into());
                    }
                };
                push(
                    &mut fills,
                    PrimitiveFillUse {
                        _sealed: (),
                        array,
                        component,
                    },
                    budget,
                )?;
            }
            _ => {}
        }
    }
    Ok(Some(ArrayUses { lengths, fills }))
}

fn return_use(
    typed: &TypedBody<'_>,
    value: crate::p3_ssa::ValueId,
) -> Result<Option<UseConversion>, RequestError> {
    use crate::p2_flow::{DeclarationType, ResultType};
    let index = ix(value.0)?;
    let ResultType::Value(result) = typed.ssa.result else {
        return Ok(None);
    };
    if let DeclarationType::Narrow(kind) = result {
        let mut range = typed.ranges[index];
        if typed.integral_bits[index] != 0 && typed.integral_bits[index] & 4 == 0 {
            range = range.and_then(super::IntRange::known01);
        }
        return Ok(
            if typed.kinds[index] == Some(Kind::Int)
                && range.is_some_and(|range| range.within(kind.range()))
            {
                Some(UseConversion::Narrow(kind))
            } else {
                None
            },
        );
    }
    if super::reference_roots::reference(result.kind()) {
        return super::references::view(&typed.kinds, &typed.zeros, value, result.kind());
    }
    if matches!(result.kind(), Kind::Float | Kind::Long | Kind::Double) {
        return super::numeric::view(&typed.ssa, &typed.kinds, value, result.kind());
    }
    Ok(match (typed.kinds[index], result.kind()) {
        (Some(Kind::Int), Kind::Int) | (Some(Kind::Boolean), Kind::Boolean) => {
            Some(UseConversion::Identity)
        }
        (Some(Kind::Boolean), Kind::Int) => Some(UseConversion::BooleanWord),
        (Some(Kind::Int), Kind::Boolean)
            if typed.integral_bits[index] != 0 && typed.integral_bits[index] & 4 == 0 =>
        {
            Some(UseConversion::Boolean01)
        }
        _ => None,
    })
}
