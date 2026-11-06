//! Scheduler handoff: retain products from this execution, never retry a stage.
use super::MethodOutcome;
use crate::p8::{MemberStop, MethodDelivery, MethodMember, StoppedStage};
use crate::{MethodId, RequestError};
fn cause(
    kind: crate::FailureKind,
    error: &impl std::fmt::Display,
) -> Result<crate::p7::SourceFailure, RequestError> {
    Ok(crate::p7::SourceFailure {
        kind,
        detail: crate::substrate::text::text(format_args!("{error}"))?,
    })
}
fn copy<T: Copy>(items: &[T]) -> Result<Vec<T>, RequestError> {
    let mut out = Vec::new();
    crate::substrate::owned::reserve(&mut out, items.len())?;
    out.extend_from_slice(items);
    Ok(out)
}
pub(super) fn member(
    id: MethodId,
    result: &Result<MethodOutcome<'_>, RequestError>,
) -> MethodMember {
    MethodMember {
        id,
        delivery: retain(result),
    }
}
fn retain(
    result: &Result<MethodOutcome<'_>, RequestError>,
) -> Result<MethodDelivery, RequestError> {
    let outcome = match result {
        Ok(v) => v,
        Err(error) => {
            return Ok(MethodDelivery::Stopped(MemberStop {
                stage: StoppedStage::Reader,
                cause_kind: error.kind(),
                cause: cause(error.kind(), error),
                raw: Ok(None),
                decoded: Ok(Vec::new()),
                decoded_complete: false,
                expression: None,
            }));
        }
    };
    match outcome {
        MethodOutcome::Expressed(m) => return Ok(MethodDelivery::Expressed(m.shared_result())),
        MethodOutcome::Declared(m) => return Ok(MethodDelivery::Declared(m.clone())),
        _ => {}
    }
    let (stage, kind, error) = match outcome {
        MethodOutcome::DisassemblyStopped(s) => {
            let e = s.error.as_ref().ok_or(RequestError::InternalInvariant(
                "stopped disassembly has no cause",
            ))?;
            (StoppedStage::P1, e.kind(), cause(e.kind(), e))
        }
        MethodOutcome::FlowStopped(s) => (
            StoppedStage::P2,
            s.error().kind(),
            cause(s.error().kind(), s.error()),
        ),
        MethodOutcome::SsaStopped(s) => (
            StoppedStage::P3,
            s.error().kind(),
            cause(s.error().kind(), s.error()),
        ),
        MethodOutcome::TypeStopped(s) => (
            StoppedStage::P4,
            s.error().kind(),
            cause(s.error().kind(), s.error()),
        ),
        MethodOutcome::StorageStopped(s) => (
            StoppedStage::P5,
            s.error().kind(),
            cause(s.error().kind(), s.error()),
        ),
        MethodOutcome::RegionStopped(s) => (
            StoppedStage::P6,
            s.error().kind(),
            cause(s.error().kind(), s.error()),
        ),
        MethodOutcome::ExpressionStopped(s) => (
            StoppedStage::P7,
            s.error().kind(),
            cause(s.error().kind(), s.error()),
        ),
        MethodOutcome::Expressed(_) | MethodOutcome::Declared(_) => {
            return Err(RequestError::InternalInvariant(
                "ready method entered stop delivery",
            ));
        }
    };
    let (source, items, complete) = if let MethodOutcome::DisassemblyStopped(s) = outcome {
        (s.source(), s.prefix.as_slice(), false)
    } else {
        let e = outcome.evidence().ok_or(RequestError::InternalInvariant(
            "stopped method lost original evidence",
        ))?;
        (Some(e.source()), e.items(), true)
    };
    let raw = source.map(crate::input::RawMethodView::retain).transpose();
    let expression = if let MethodOutcome::ExpressionStopped(s) = outcome {
        Some(s.shared_partial())
    } else {
        None
    };
    Ok(MethodDelivery::Stopped(MemberStop {
        stage,
        cause_kind: kind,
        cause: error,
        raw,
        decoded: copy(items),
        decoded_complete: complete,
        expression,
    }))
}
