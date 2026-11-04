use crate::context::Context;
use crate::input::{MethodSource, protection::CatchType};
use crate::p2_flow::{BlockId, Op, OpId};
use crate::p3_ssa::SsaBody;
use crate::substrate::owned::push;
use crate::{RequestError, RunContext, TypeId};

pub(crate) const THROWABLE: &[u16] =
    &const { crate::substrate::owned::ascii_units(*b"Ljava/lang/Throwable;") };

#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum ExceptionType {
    Nominal(TypeId),
    Throwable,
}
pub(crate) struct ExceptionBinding {
    pub handler: BlockId,
    pub operation: OpId,
    pub kind: Option<ExceptionType>,
}
pub(crate) fn collect(
    input: &MethodSource<'_, '_>,
    _catalog: &Context,
    ssa: &SsaBody<'_>,
    budget: &mut RunContext<'_>,
) -> Result<Vec<ExceptionBinding>, RequestError> {
    let mut result = Vec::new();
    for (index, block) in ssa.bound.iter().enumerate() {
        budget.checkpoint()?;
        let Some(&first) = block.ops.first() else {
            continue;
        };
        if !matches!(ssa.operation(first)?.op, Op::Exception(_)) {
            continue;
        }
        let handler = BlockId(u32::try_from(index).map_err(|_| RequestError::PlatformCapacity)?);
        let mut kind = None;
        for edge in &ssa.exception_edges {
            budget.checkpoint()?;
            if edge.target != handler {
                continue;
            }
            let incoming = match edge.kind {
                CatchType::Typed(id) => {
                    input.type_descriptor(id, budget)?;
                    ExceptionType::Nominal(id)
                }
                CatchType::All => ExceptionType::Throwable,
            };
            kind = Some(match kind {
                None => incoming,
                Some(old) if old == incoming => old,
                Some(_) => ExceptionType::Throwable,
            });
        }
        push(
            &mut result,
            ExceptionBinding {
                handler,
                operation: first,
                kind,
            },
            budget,
        )?;
    }
    Ok(result)
}
