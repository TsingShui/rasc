use super::*;
use crate::{FailureKind, RequestError};
use thiserror::Error;
#[derive(Debug, Error)]
pub enum ExpressionError {
    #[error("P7 construction failed: {0}")]
    Request(#[from] RequestError),
    #[error("BUG: {0}")]
    InternalInvariant(&'static str),
}
impl ExpressionError {
    pub fn kind(&self) -> FailureKind {
        match self {
            Self::Request(e) => e.kind(),
            Self::InternalInvariant(_) => FailureKind::InternalInvariant,
        }
    }
}
pub(crate) enum MethodBuild<'a> {
    Ready(ExpressedMethod<'a>),
    Stopped(ExpressionStop<'a>),
}
pub(crate) fn build_method<'a>(
    _task: &crate::engine::MethodTask,
    planned: crate::p6::PlannedMethod<'a>,
) -> MethodBuild<'a> {
    let mut partial = ExpressionPartial {
        header: None,
        code: MethodCode::empty(),
    };
    let built = construct(&planned, &mut partial);
    match built {
        Ok(()) => {
            if let Some(header) = partial.header.take() {
                MethodBuild::Ready(ExpressedMethod {
                    planned,
                    result: std::sync::Arc::new(MethodResult {
                        header,
                        code: Some(partial.code),
                    }),
                })
            } else {
                MethodBuild::Stopped(ExpressionStop {
                    planned,
                    error: bug("completed method expression has no declaration"),
                    partial: std::sync::Arc::new(partial),
                })
            }
        }
        Err(error) => MethodBuild::Stopped(ExpressionStop {
            planned,
            error,
            partial: std::sync::Arc::new(partial),
        }),
    }
}
fn header(
    owner: &crate::p0::OwnerFacts,
    method: crate::MethodId,
    flags: u32,
    declaration: &crate::p0::MethodDeclaration,
    metadata: &crate::p0::MethodMetadata,
    metadata_error: Option<std::sync::Arc<RequestError>>,
) -> Result<MethodHeader, ExpressionError> {
    let mut parameter_names = Vec::new();
    for i in 0..declaration.parameter_count() {
        push(
            &mut parameter_names,
            super::references::text(format_args!("p{i}"))?,
        )?;
    }
    Ok(MethodHeader {
        parameter_names,
        key: crate::MethodKey {
            class: owner.key(),
            method,
        },
        owner_type: owner.type_id(),
        owner_descriptor: copy(owner.descriptor().units())?,
        access_flags: flags,
        declaration: declaration.try_copy()?,
        name_form: crate::substrate::names::name_form(declaration.name_units()),
        kind: if declaration
            .name_units()
            .iter()
            .copied()
            .eq(b"<init>".iter().copied().map(u16::from))
        {
            MethodKind::Constructor
        } else if declaration
            .name_units()
            .iter()
            .copied()
            .eq(b"<clinit>".iter().copied().map(u16::from))
        {
            MethodKind::ClassInitializer
        } else {
            MethodKind::Ordinary
        },
        metadata: metadata.clone(),
        metadata_error,
    })
}
pub(crate) fn build_declaration(
    _task: &crate::engine::MethodTask,
    source: &crate::input::MethodSource<'_, '_>,
) -> Result<MethodResult, ExpressionError> {
    Ok(MethodResult {
        header: header(
            source.owner_facts(),
            source.method_id(),
            source.access_flags(),
            source.declaration(),
            source.original_metadata(),
            source.metadata_error(),
        )?,
        code: None,
    })
}
fn construct(
    planned: &crate::p6::PlannedMethod<'_>,
    partial: &mut ExpressionPartial,
) -> Result<(), ExpressionError> {
    let flow = planned.execution();
    partial.header = Some(header(
        flow.owner_facts(),
        flow.method_id(),
        flow.access_flags(),
        flow.declaration(),
        flow.metadata(),
        flow.metadata_error().cloned(),
    )?);
    let mut c = super::values::Context::new(planned, &mut partial.code)?;
    super::values::prepare(&mut c)?;
    super::copies::construct(&mut c)?;
    super::lower::operations(&mut c)?;
    super::lower::terminals(&mut c)?;
    super::control::construct(&mut c)?;
    super::emission::construct(c.code)?;
    let key = partial
        .header
        .as_ref()
        .ok_or(bug("compose lost its method header"))?
        .key;
    super::compose::construct(c.code, planned.type_facts(), key)?;
    super::local_names::construct(
        c.code,
        planned.storage_facts().definitions(),
        &partial
            .header
            .as_ref()
            .ok_or(bug("local naming lost its method header"))?
            .parameter_names,
    )
}
pub(super) fn push<T>(v: &mut Vec<T>, x: T) -> Result<(), ExpressionError> {
    crate::substrate::owned::push(v, x).map_err(Into::into)
}
pub(super) fn copy<T: Copy>(source: &[T]) -> Result<Vec<T>, ExpressionError> {
    let mut v = Vec::new();
    crate::substrate::owned::reserve(&mut v, source.len())?;
    v.extend_from_slice(source);
    Ok(v)
}
pub(super) fn filled<T: Clone>(n: usize, value: T) -> Result<Vec<T>, ExpressionError> {
    let mut v = Vec::new();
    crate::substrate::owned::reserve(&mut v, n)?;
    v.resize(n, value);
    Ok(v)
}
pub(super) fn id(n: usize) -> Result<u32, ExpressionError> {
    u32::try_from(n).map_err(|_| RequestError::PlatformCapacity.into())
}
pub(super) fn bug(message: &'static str) -> ExpressionError {
    ExpressionError::InternalInvariant(message)
}
