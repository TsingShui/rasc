use crate::p2_flow::Op;
use crate::p3_ssa::{Definition, SsaBody, ValueId};
use crate::{CodeUnitOffset, FieldId, RequestError, RunContext};

pub(crate) struct CaptureUse {
    operation: crate::p2_flow::OpId,
    field: FieldId,
    initialization: CodeUnitOffset,
}
impl CaptureUse {
    pub(crate) fn operation(&self) -> crate::p2_flow::OpId {
        self.operation
    }
    pub(crate) fn field(&self) -> FieldId {
        self.field
    }
    pub(crate) fn initialization(&self) -> CodeUnitOffset {
        self.initialization
    }
}

pub(super) fn prove(
    ssa: &SsaBody<'_>,
    certificate: Option<&crate::CaptureCertificate>,
    budget: &mut RunContext<'_>,
) -> Result<Option<CaptureUse>, RequestError> {
    let Some(certificate) = certificate else {
        return Ok(None);
    };
    let mut capture = None;
    for operation in &ssa.operations {
        budget.checkpoint()?;
        if matches!(operation.op, Op::FieldStore(_, _, field, _) if field == certificate.field()) {
            if capture.is_some() || operation.pc != CodeUnitOffset(0) {
                return Err(RequestError::InitializationUnavailable { pc: operation.pc });
            }
            if !parameter(ssa, operation.input(0)?, 0, budget)?
                || !parameter(ssa, operation.input(1)?, 1, budget)?
            {
                return Err(RequestError::InitializationUnavailable { pc: operation.pc });
            }
            capture = Some(CaptureUse {
                operation: operation.id,
                field: certificate.field(),
                initialization: certificate.initialization(),
            });
        }
    }
    capture
        .ok_or(RequestError::InternalInvariant(
            "capture certificate lost shared field operation",
        ))
        .map(Some)
}
fn parameter(
    ssa: &SsaBody<'_>,
    value: ValueId,
    index: u32,
    budget: &mut RunContext<'_>,
) -> Result<bool, RequestError> {
    budget.checkpoint()?;
    match ssa
        .values
        .get(usize::try_from(value.0).map_err(|_| RequestError::PlatformCapacity)?)
    {
        Some(Definition::Parameter(actual)) => Ok(*actual == index),
        Some(Definition::Phi) => {
            for phi in &ssa.phis {
                budget.checkpoint()?;
                if phi.result == value {
                    if let [(crate::p3_ssa::Incoming::EntrySeed, crate::p3_ssa::Sym::Value(seed))] =
                        phi.inputs.as_slice()
                    {
                        return Ok(
                            matches!(ssa.values.get(usize::try_from(seed.0).map_err(|_| RequestError::PlatformCapacity)?), Some(Definition::Parameter(actual)) if *actual == index),
                        );
                    }
                    return Ok(false);
                }
            }
            Ok(false)
        }
        _ => Ok(false),
    }
}
