use super::*;
use crate::p3::{SsaFacts, StateId, StateKind, ValueId};
use crate::p4::TypedMethod;
use crate::substrate::owned;
use crate::{FailureKind, RequestError};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("P5 construction failed: {0}")]
    Resource(#[from] RequestError),
    #[error("BUG: {0}")]
    InternalInvariant(&'static str),
}
impl StorageError {
    pub fn kind(&self) -> FailureKind {
        match self {
            Self::Resource(e) => e.kind(),
            Self::InternalInvariant(_) => FailureKind::InternalInvariant,
        }
    }
}
pub(crate) enum StorageBuild<'a> {
    Ready(StorageMethod<'a>),
    Stopped(StorageStop<'a>),
}
pub(crate) fn build_storage<'a>(
    _task: &crate::engine::MethodTask,
    typed: TypedMethod<'a>,
) -> StorageBuild<'a> {
    let mut facts = StorageFacts::empty();
    let result = construct(&typed, &mut facts);
    match result {
        Ok(()) => StorageBuild::Ready(StorageMethod { typed, facts }),
        Err(error) => StorageBuild::Stopped(StorageStop {
            typed,
            error,
            partial: StoragePartial { facts },
        }),
    }
}
fn construct(typed: &TypedMethod<'_>, f: &mut StorageFacts) -> Result<(), StorageError> {
    super::bindings::construct(typed, f)?;
    super::transfers::construct(typed, f)?;
    super::copies::construct(f)?;
    super::materialization::construct(typed, f)
}
pub(super) fn push<T>(v: &mut Vec<T>, item: T) -> Result<(), StorageError> {
    owned::push(v, item).map_err(Into::into)
}
pub(super) fn filled<T: Copy>(n: usize, item: T) -> Result<Vec<T>, StorageError> {
    let mut v = Vec::new();
    owned::reserve(&mut v, n)?;
    v.resize(n, item);
    Ok(v)
}
pub(super) fn id(n: usize) -> Result<u32, StorageError> {
    u32::try_from(n).map_err(|_| RequestError::PlatformCapacity.into())
}
pub(super) fn value(s: &SsaFacts, state: StateId) -> Option<ValueId> {
    match s.state(state)?.kind() {
        StateKind::Value(v) => Some(v),
        StateKind::Phi(p) => s.phis().get(p.0 as usize).map(|p| p.value()),
        _ => None,
    }
}
pub(super) fn local(
    f: &mut StorageFacts,
    storage_type: StorageType,
    kind: LocalKind,
) -> Result<LocalId, StorageError> {
    let id = LocalId(id(f.locals.len())?);
    push(
        &mut f.locals,
        Local {
            id,
            storage_type,
            kind,
        },
    )?;
    push(&mut f.local_gaps, None)?;
    Ok(id)
}
pub(super) fn issue(
    f: &mut StorageFacts,
    site: StorageIssueSite,
    gap: StorageGap,
) -> Result<(), StorageError> {
    push(&mut f.issues, StorageIssue { site, gap })
}
