use super::{Copy, CopyPlan, LocalId, LocalType, LocalTypeId, Width};
use crate::MethodBudget;
use crate::RequestError;
use crate::p2_flow::normal::{BlockId, EdgeId};
use crate::p3_ssa::normal::{Definition, Sym};
use crate::p5_types::normal::Kind;
use crate::p5_types::normal::TypedBody;
use crate::substrate::owned::push;

// Widening the original u32 type ID keeps nominal identities disjoint from
// builtin identities without restricting the input type pool.
pub(crate) fn local_type(kind: Kind) -> Result<LocalType, RequestError> {
    LocalType::new(
        LocalTypeId(match kind {
            Kind::Int => 0,
            Kind::Boolean => 1,
            Kind::Object => 2,
            Kind::Float => 3,
            Kind::String => 4,
            Kind::IntArray => 5,
            Kind::ObjectArray => 6,
            Kind::Nominal(id) => u64::from(id.0) + 7,
        }),
        1,
    )
}
pub(crate) fn kind(local_type: LocalType) -> Result<Kind, RequestError> {
    match (local_type.identity.0, local_type.width) {
        (0, Width::One) => Ok(Kind::Int),
        (1, Width::One) => Ok(Kind::Boolean),
        (2, Width::One) => Ok(Kind::Object),
        (3, Width::One) => Ok(Kind::Float),
        (4, Width::One) => Ok(Kind::String),
        (5, Width::One) => Ok(Kind::IntArray),
        (6, Width::One) => Ok(Kind::ObjectArray),
        (identity, Width::One) if identity >= 7 => Ok(Kind::Nominal(crate::TypeId(
            u32::try_from(identity - 7).map_err(|_| RequestError::PlatformCapacity)?,
        ))),
        _ => Err(RequestError::InternalInvariant(
            "foreign normal storage type",
        )),
    }
}

#[derive(Clone, Copy)]
pub(crate) struct Local {
    pub id: LocalId,
    pub kind: Kind,
    pub parameter: Option<u32>,
}
pub(crate) struct StorageBody<'a> {
    _sealed: (),
    pub typed: TypedBody<'a>,
    pub locals: Vec<Local>,
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
    budget: &mut MethodBudget<'_, '_>,
) -> Result<Vec<Copy>, RequestError> {
    let mut copies = Vec::new();
    for phi in &typed.ssa.phis {
        budget.charge_work(1)?;
        if phi.block != target {
            continue;
        }
        let mut selected = None;
        for &(edge, value) in &phi.inputs {
            budget.charge_work(1)?;
            if edge == incoming && selected.replace(value).is_some() {
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
        push(
            &mut copies,
            Copy {
                target: LocalId(phi.result.0),
                source: LocalId(source.0),
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
    budget: &mut MethodBudget<'_, '_>,
) -> Result<CopyPlan, RequestError> {
    let copies = parallel(typed, incoming, target, budget)?;
    let mut types = Vec::new();
    for local in locals.iter() {
        budget.charge_work(1)?;
        push(&mut types, local_type(local.kind)?, budget)?;
    }
    let plan = super::plan(&copies, &types, budget)?;
    for temporary in &plan.temporaries {
        budget.charge_work(1)?;
        if usize::try_from(temporary.local.0).map_err(|_| RequestError::PlatformCapacity)?
            != locals.len()
        {
            return Err(RequestError::InternalInvariant(
                "temporary does not extend storage",
            ));
        }
        push(
            locals,
            Local {
                id: temporary.local,
                kind: kind(temporary.kind)?,
                parameter: None,
            },
            budget,
        )?;
    }
    Ok(plan)
}
pub(crate) fn allocate<'a>(
    typed: TypedBody<'a>,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<Option<StorageBody<'a>>, RequestError> {
    let mut locals = Vec::new();
    for (index, definition) in typed.ssa.values.iter().enumerate() {
        budget.charge_work(1)?;
        let Some(kind) = typed.kinds[index] else {
            return Ok(None);
        };
        let parameter = match definition {
            Definition::Parameter(parameter) => Some(*parameter),
            _ => None,
        };
        push(
            &mut locals,
            Local {
                id: LocalId(u32::try_from(index).map_err(|_| RequestError::PlatformCapacity)?),
                kind,
                parameter,
            },
            budget,
        )?;
    }
    let entry = plan_edge(&typed, None, BlockId(0), &mut locals, budget)?;
    let mut normal = Vec::new();
    for (index, edge) in typed.ssa.edges.iter().enumerate() {
        budget.charge_work(1)?;
        let id = EdgeId(u32::try_from(index).map_err(|_| RequestError::PlatformCapacity)?);
        let plan = plan_edge(&typed, Some(id), edge.target, &mut locals, budget)?;
        push(&mut normal, plan, budget)?;
    }
    Ok(Some(StorageBody {
        _sealed: (),
        typed,
        locals,
        entry,
        normal,
    }))
}
