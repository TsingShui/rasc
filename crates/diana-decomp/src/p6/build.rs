use super::*;
use crate::substrate::owned;
use crate::{FailureKind, RequestError};
use thiserror::Error;
#[derive(Debug, Error)]
pub enum RegionError {
    #[error("P6 construction failed: {0}")]
    Resource(#[from] RequestError),
    #[error("BUG: {0}")]
    InternalInvariant(&'static str),
}
impl RegionError {
    pub fn kind(&self) -> FailureKind {
        match self {
            Self::Resource(e) => e.kind(),
            Self::InternalInvariant(_) => FailureKind::InternalInvariant,
        }
    }
}
pub(crate) enum RegionBuild<'a> {
    Ready(PlannedMethod<'a>),
    Stopped(RegionStop<'a>),
}
pub(crate) fn build_regions<'a>(
    _task: &crate::engine::MethodTask,
    storage: crate::p5::StorageMethod<'a>,
) -> RegionBuild<'a> {
    let mut facts = RegionFacts::empty();
    match construct(&storage, &mut facts) {
        Ok(()) => RegionBuild::Ready(PlannedMethod { storage, facts }),
        Err(error) => RegionBuild::Stopped(RegionStop {
            storage,
            error,
            partial: RegionPartial { facts },
        }),
    }
}
fn construct(
    storage: &crate::p5::StorageMethod<'_>,
    f: &mut RegionFacts,
) -> Result<(), RegionError> {
    let graph = super::graph::construct(storage, f)?;
    super::protection::collect(storage, f, &graph)?;
    let structure = super::dominance::Structure::construct(f, &graph)?;
    super::loops::construct(storage, f, &graph, &structure)?;
    super::locks::construct(storage, f, &graph)?;
    super::protection::plan(storage, f, &graph, &structure)?;
    super::regions::construct(storage, f, &graph, &structure)
}
pub(super) fn push<T>(v: &mut Vec<T>, x: T) -> Result<(), RegionError> {
    owned::push(v, x).map_err(Into::into)
}
pub(super) fn filled<T: Clone>(n: usize, x: T) -> Result<Vec<T>, RegionError> {
    let mut v = Vec::new();
    owned::reserve(&mut v, n)?;
    v.resize(n, x);
    Ok(v)
}
pub(super) fn lists<T>(n: usize) -> Result<Vec<Vec<T>>, RegionError> {
    let mut v = Vec::new();
    owned::reserve(&mut v, n)?;
    v.resize_with(n, Vec::new);
    Ok(v)
}
pub(super) fn id(n: usize) -> Result<u32, RegionError> {
    u32::try_from(n).map_err(|_| RequestError::PlatformCapacity.into())
}
pub(super) fn bug(s: &'static str) -> RegionError {
    RegionError::InternalInvariant(s)
}
