use super::{Copy, CopyPlan, LocalId, LocalType, LocalTypeId, Width};
use crate::RequestError;
use crate::RunContext;
use crate::p2_flow::{BlockId, EdgeId};
use crate::p3_ssa::{Definition, Sym};
use crate::p4_types::Kind;
use crate::p4_types::TypedBody;
use crate::substrate::owned::push;

// Disjoint 64-bit coordinate domains retain both the root type ID and array depth.
pub(crate) fn local_type(kind: Kind) -> Result<LocalType, RequestError> {
    LocalType::new(
        LocalTypeId(match kind {
            Kind::Int => 0,
            Kind::Boolean => 1,
            Kind::Object => 2,
            Kind::Throwable => (3u128 << 64) + 16,
            Kind::Class => (4u128 << 64) + 16,
            Kind::Float => 3,
            Kind::String => 4,
            Kind::Long => 14,
            Kind::Double => 15,
            Kind::PrimitiveArray(kind) => match kind {
                crate::p4_types::PrimitiveArray::Int => 5,
                crate::p4_types::PrimitiveArray::Boolean => 7,
                crate::p4_types::PrimitiveArray::Byte => 8,
                crate::p4_types::PrimitiveArray::Short => 9,
                crate::p4_types::PrimitiveArray::Char => 10,
                crate::p4_types::PrimitiveArray::Float => 11,
                crate::p4_types::PrimitiveArray::Long => 12,
                crate::p4_types::PrimitiveArray::Double => 13,
            },
            Kind::ObjectArray => 6,
            Kind::Nominal(id) => u128::from(id.0) + 16,
            Kind::ReferenceArray(key) => 16 + (1u128 << 64) + key.identity(),
            Kind::ProjectedNominal(key) => 16 + (2u128 << 64) + key.identity(),
        }),
        if matches!(kind, Kind::Long | Kind::Double) {
            2
        } else {
            1
        },
    )
}
fn kind(
    local_type_value: LocalType,
    locals: &[Local],
    budget: &mut RunContext<'_>,
) -> Result<Kind, RequestError> {
    if local_type_value.identity.0 >= 16 {
        for local in locals {
            budget.checkpoint()?;
            if local_type(local.kind)? == local_type_value {
                return Ok(local.kind);
            }
        }
        return Err(RequestError::InternalInvariant(
            "temporary lost its checked source type",
        ));
    }
    let local_type = local_type_value;
    match (local_type.identity.0, local_type.width) {
        (0, Width::One) => Ok(Kind::Int),
        (1, Width::One) => Ok(Kind::Boolean),
        (2, Width::One) => Ok(Kind::Object),
        (3, Width::One) => Ok(Kind::Float),
        (4, Width::One) => Ok(Kind::String),
        (5, Width::One) => Ok(Kind::PrimitiveArray(crate::p4_types::PrimitiveArray::Int)),
        (id @ 7..=13, Width::One) => Ok(Kind::PrimitiveArray(match id {
            7 => crate::p4_types::PrimitiveArray::Boolean,
            8 => crate::p4_types::PrimitiveArray::Byte,
            9 => crate::p4_types::PrimitiveArray::Short,
            10 => crate::p4_types::PrimitiveArray::Char,
            11 => crate::p4_types::PrimitiveArray::Float,
            12 => crate::p4_types::PrimitiveArray::Long,
            _ => crate::p4_types::PrimitiveArray::Double,
        })),
        (6, Width::One) => Ok(Kind::ObjectArray),
        (14, Width::Two) => Ok(Kind::Long),
        (15, Width::Two) => Ok(Kind::Double),

        _ => Err(RequestError::InternalInvariant(
            "foreign normal storage type",
        )),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Parameter {
    Explicit(u32),
    This,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Initializer {
    Default,
    ThisAfterChain,
    Parameter(Parameter),
    Deferred(crate::p4_types::initialization::InitializationId),
}
#[derive(Clone, Copy)]
pub(crate) struct Local {
    pub id: LocalId,
    pub kind: Kind,
    pub initializer: Initializer,
}
crate::substrate::checked::checked_product!(StorageBody, StorageBodyData);
pub(crate) struct StorageBodyData<'a> {
    _sealed: (),
    pub typed: TypedBody<'a>,
    pub locals: Vec<Local>,
    pub filled_targets: Vec<LocalId>,
    pub empty_fills: Vec<super::empty_fill::LengthOnly>,
    pub fresh_fills: Vec<super::fresh_fill::FreshArray>,
    pub object_allocations: Vec<super::object_allocation::DeferredAllocation>,
    pub constructor_entry: Option<super::constructor_entry::ConstructorEntry>,
    pub exceptional: super::exceptional::ExceptionalStorage,
    entry: CopyPlan,
    normal: Vec<CopyPlan>,
}
impl StorageBody<'_> {
    pub(crate) fn copies(
        &self,
        incoming: Option<EdgeId>,
        target: BlockId,
    ) -> Result<&CopyPlan, RequestError> {
        match incoming {
            None if target == BlockId(0) => Ok(&self.entry),
            Some(edge) => {
                let index = usize::try_from(edge.0).map_err(|_| RequestError::PlatformCapacity)?;
                let original = self
                    .typed
                    .ssa
                    .edges
                    .get(index)
                    .ok_or(RequestError::InternalInvariant("foreign storage edge"))?;
                if original.target != target {
                    return Err(RequestError::InternalInvariant(
                        "storage edge changed target",
                    ));
                }
                self.normal
                    .get(index)
                    .ok_or(RequestError::InternalInvariant("edge lost storage copies"))
            }
            None => Err(RequestError::InternalInvariant(
                "EntrySeed enters a non-entry block",
            )),
        }
    }
}
fn parallel(
    typed: &TypedBody<'_>,
    incoming: Option<EdgeId>,
    target: BlockId,
    budget: &mut RunContext<'_>,
) -> Result<Vec<Copy>, RequestError> {
    let mut copies = Vec::new();
    for phi in &typed.ssa.phis {
        budget.checkpoint()?;
        if phi.block != target {
            continue;
        }
        let mut selected = None;
        for &(edge, value) in &phi.inputs {
            budget.checkpoint()?;
            if edge
                == incoming.map_or(
                    crate::p3_ssa::Incoming::EntrySeed,
                    crate::p3_ssa::Incoming::Normal,
                )
                && selected.replace(value).is_some()
            {
                return Err(RequestError::InternalInvariant(
                    "storage Phi has duplicate incoming edges",
                ));
            }
        }
        let Some(Sym::Value(source)) = selected else {
            return Err(RequestError::InternalInvariant(
                "storage Phi lost an available incoming value",
            ));
        };
        let target_kind = typed.kinds
            [usize::try_from(phi.result.0).map_err(|_| RequestError::PlatformCapacity)?]
        .ok_or(RequestError::InternalInvariant("Phi target lost kind"))?;
        let source_kind = typed.kinds
            [usize::try_from(source.0).map_err(|_| RequestError::PlatformCapacity)?]
        .ok_or(RequestError::InternalInvariant("Phi source lost kind"))?;
        let source = if local_type(source_kind)? == local_type(target_kind)? {
            crate::p5_variables::CopySource::Local(LocalId(source.0))
        } else {
            let evidence = typed
                .zeros
                .evidence(source)?
                .ok_or(RequestError::InternalInvariant(
                    "Phi copy lacks zero evidence",
                ))?;
            let view = crate::p4_types::zero::view(target_kind).ok_or(
                RequestError::InternalInvariant("word zero has incompatible storage width"),
            )?;
            crate::p5_variables::CopySource::Zero {
                evidence,
                view,
                kind: local_type(target_kind)?,
            }
        };
        push(
            &mut copies,
            Copy {
                target: LocalId(phi.result.0),
                source,
            },
            budget,
        )?;
    }
    Ok(copies)
}
fn plan_edge(
    typed: &TypedBody<'_>,
    incoming: Option<EdgeId>,
    target: BlockId,
    locals: &mut Vec<Local>,
    budget: &mut RunContext<'_>,
) -> Result<CopyPlan, RequestError> {
    let copies = parallel(typed, incoming, target, budget)?;
    if copies.is_empty() {
        budget.checkpoint()?;
        return Ok(CopyPlan {
            steps: Vec::new(),
            temporaries: Vec::new(),
        });
    }
    let mut types = Vec::new();
    for local in locals.iter() {
        budget.checkpoint()?;
        push(&mut types, local_type(local.kind)?, budget)?;
    }
    let plan = super::plan(&copies, &types, budget)?;
    for temporary in &plan.temporaries {
        budget.checkpoint()?;
        if usize::try_from(temporary.local.0).map_err(|_| RequestError::PlatformCapacity)?
            != locals.len()
        {
            return Err(RequestError::InternalInvariant(
                "temporary does not extend storage",
            ));
        }
        let source_kind = kind(temporary.kind, locals, budget)?;
        push(
            locals,
            Local {
                id: temporary.local,
                kind: source_kind,
                initializer: Initializer::Default,
            },
            budget,
        )?;
    }
    Ok(plan)
}
pub(crate) fn allocate<'a>(
    _schedule: &crate::engine::SchedulePermit,
    typed: TypedBody<'a>,
    budget: &mut RunContext<'_>,
) -> Result<crate::substrate::outcome::Outcome<StorageBody<'a>>, RequestError> {
    allocate_impl(typed, budget)
}

fn allocate_impl<'a>(
    typed: TypedBody<'a>,
    budget: &mut RunContext<'_>,
) -> Result<crate::substrate::outcome::Outcome<StorageBody<'a>>, RequestError> {
    let mut locals = Vec::new();
    for (index, definition) in typed.ssa.values.iter().enumerate() {
        budget.checkpoint()?;
        let Some(kind) = typed.kinds[index] else {
            return Ok(crate::substrate::outcome::Outcome::Declined(
                crate::OutputReason::JavaCheckFailed {
                    code: crate::DiagnosticCode::JavaAnalysisUnavailable,
                    phase: "P5",
                    check: "storage_missing_kind",
                    pc: None,
                },
            ));
        };
        let parameter = match definition {
            Definition::Parameter(parameter) => Some(match typed.ssa.frame {
                crate::p2_flow::MethodFrame::Static => Parameter::Explicit(*parameter),
                crate::p2_flow::MethodFrame::Instance(_)
                | crate::p2_flow::MethodFrame::Constructor(_)
                    if *parameter == 0 =>
                {
                    Parameter::This
                }
                crate::p2_flow::MethodFrame::Instance(_)
                | crate::p2_flow::MethodFrame::Constructor(_) => {
                    Parameter::Explicit(parameter.checked_sub(1).ok_or(
                        RequestError::InternalInvariant("implicit receiver frame underflow"),
                    )?)
                }
            }),
            _ => None,
        };
        push(
            &mut locals,
            Local {
                id: LocalId(u32::try_from(index).map_err(|_| RequestError::PlatformCapacity)?),
                kind,
                initializer: if typed.initializations.deferred.get(index)
                    == Some(&Some(
                        crate::p4_types::initialization::InitializationId::This,
                    )) {
                    Initializer::ThisAfterChain
                } else if let Some(parameter) = parameter {
                    Initializer::Parameter(parameter)
                } else if let Some(Some(id)) = typed.initializations.deferred.get(index) {
                    Initializer::Deferred(*id)
                } else {
                    Initializer::Default
                },
            },
            budget,
        )?;
    }
    let constructor_entry = super::constructor_entry::prove(&typed, budget)?;
    let mut object_allocations = Vec::new();
    for plan in &typed.initializations.plans {
        budget.checkpoint()?;
        let proof = super::object_allocation::prove(&typed, plan, budget)?;
        push(&mut object_allocations, proof, budget)?;
    }
    let mut fresh_fills = Vec::new();
    for op in &typed.ssa.operations {
        budget.checkpoint()?;
        let crate::p2_flow::Op::FreshFill(_, id) = op.op else {
            continue;
        };
        if usize::try_from(id.0).map_err(|_| RequestError::PlatformCapacity)? != fresh_fills.len() {
            return Err(RequestError::InternalInvariant(
                "fresh fill changed storage identity",
            ));
        }
        let proof = super::fresh_fill::prove(&typed, op, budget)?;
        push(&mut fresh_fills, proof, budget)?;
    }
    let mut filled_targets = Vec::new();
    if !typed.filled_arrays.is_empty() {
        for op in &typed.ssa.operations {
            budget.checkpoint()?;
            let crate::p2_flow::Op::FilledArray(id, result) = op.op else {
                continue;
            };
            let index = usize::try_from(id.0).map_err(|_| RequestError::PlatformCapacity)?;
            if index != filled_targets.len() {
                return Err(RequestError::InternalInvariant(
                    "filled storage changed identity",
                ));
            }
            let usage = typed
                .filled_arrays
                .get(index)
                .ok_or(RequestError::InternalInvariant(
                    "filled array lost typed contents",
                ))?;
            let target = match (result, op.result) {
                (crate::p2_flow::FilledResult::Stored { .. }, Some(value)) => {
                    let value_index =
                        usize::try_from(value.0).map_err(|_| RequestError::PlatformCapacity)?;
                    if typed.kinds.get(value_index).copied().flatten() != Some(usage.kind) {
                        return Err(RequestError::InternalInvariant(
                            "filled result changed source type",
                        ));
                    }
                    LocalId(value.0)
                }
                (crate::p2_flow::FilledResult::Discarded, None) => {
                    // Java array creation is not a standalone statement expression.
                    let id = LocalId(
                        u32::try_from(locals.len()).map_err(|_| RequestError::PlatformCapacity)?,
                    );
                    push(
                        &mut locals,
                        Local {
                            id,
                            kind: usage.kind,
                            initializer: Initializer::Default,
                        },
                        budget,
                    )?;
                    id
                }
                _ => {
                    return Err(RequestError::InternalInvariant(
                        "filled result lost original binding",
                    ));
                }
            };
            push(&mut filled_targets, target, budget)?;
        }
    }
    let mut empty_fills = Vec::new();
    for op in &typed.ssa.operations {
        budget.checkpoint()?;
        let crate::p2_flow::Op::EmptyFill(_, id) = op.op else {
            continue;
        };
        if usize::try_from(id.0).map_err(|_| RequestError::PlatformCapacity)? != empty_fills.len() {
            return Err(RequestError::InternalInvariant(
                "fill storage changed identity",
            ));
        }
        let result =
            LocalId(u32::try_from(locals.len()).map_err(|_| RequestError::PlatformCapacity)?);
        let proof = super::empty_fill::prove(&typed, op, result, budget)?;
        push(
            &mut locals,
            Local {
                id: result,
                kind: Kind::Int,
                initializer: Initializer::Default,
            },
            budget,
        )?;
        push(&mut empty_fills, proof, budget)?;
    }
    let entry = plan_edge(&typed, None, BlockId(0), &mut locals, budget)?;
    let mut normal = Vec::new();
    for (index, edge) in typed.ssa.edges.iter().enumerate() {
        budget.checkpoint()?;
        let id = EdgeId(u32::try_from(index).map_err(|_| RequestError::PlatformCapacity)?);
        let plan = plan_edge(&typed, Some(id), edge.target, &mut locals, budget)?;
        push(&mut normal, plan, budget)?;
    }
    let Some(exceptional) = super::exceptional::build(&typed, &mut locals, budget)? else {
        return Ok(crate::substrate::outcome::Outcome::Declined(
            crate::OutputReason::JavaCheckFailed {
                code: crate::DiagnosticCode::JavaAnalysisUnavailable,
                phase: "P5",
                check: "exceptional_storage",
                pc: None,
            },
        ));
    };
    Ok(crate::substrate::outcome::Outcome::Ready(StorageBody(
        StorageBodyData {
            _sealed: (),
            exceptional,
            typed,
            locals,
            filled_targets,
            empty_fills,
            fresh_fills,
            object_allocations,
            constructor_entry,
            entry,
            normal,
        },
    )))
}
