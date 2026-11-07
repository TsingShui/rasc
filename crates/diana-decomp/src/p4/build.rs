use super::*;
use crate::p3::{SsaFacts, SsaMethod, StateId, StateKind, ValueId};
use crate::substrate::owned;
use crate::{FailureKind, RequestError};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum TypeError {
    #[error("P4 construction failed: {0}")]
    Resource(#[from] RequestError),
    #[error("BUG: {0}")]
    InternalInvariant(&'static str),
}
impl TypeError {
    pub fn kind(&self) -> FailureKind {
        match self {
            Self::Resource(e) => e.kind(),
            Self::InternalInvariant(_) => FailureKind::InternalInvariant,
        }
    }
}
pub(crate) enum TypeBuild<'a> {
    Ready(TypedMethod<'a>),
    Stopped(TypeStop<'a>),
}
pub(crate) fn build_types<'a>(
    _task: &crate::engine::MethodTask,
    ssa: SsaMethod<'a>,
) -> TypeBuild<'a> {
    let mut facts = TypeFacts::empty();
    let result = construct(&ssa, &mut facts);
    match result {
        Ok(()) => TypeBuild::Ready(TypedMethod { ssa, facts }),
        Err(error) => TypeBuild::Stopped(TypeStop {
            ssa,
            error,
            partial: TypePartial { facts },
        }),
    }
}
fn construct(ssa: &SsaMethod<'_>, f: &mut TypeFacts) -> Result<(), TypeError> {
    let mut work = super::constraints::collect(ssa, f)?;
    super::refinements::construct(ssa, f, &work)?;
    super::solve::sources(ssa.facts(), f, &work)?;
    super::redundancy::construct(ssa, f, &work)?;
    super::components::finish(ssa, f, &work)?;
    super::solve::families(ssa.facts(), f, &mut work)?;
    super::views::finish(ssa.facts(), f)?;
    super::arrays::finish(ssa, f)?;
    super::initialization::construct(ssa, f, &work)?;
    Ok(())
}
pub(super) fn push<T>(v: &mut Vec<T>, value: T) -> Result<(), TypeError> {
    owned::push(v, value).map_err(Into::into)
}
pub(super) fn filled<T: Copy>(n: usize, value: T) -> Result<Vec<T>, TypeError> {
    let mut v = Vec::new();
    owned::reserve(&mut v, n)?;
    v.resize(n, value);
    Ok(v)
}
pub(super) fn lists<T>(n: usize) -> Result<Vec<Vec<T>>, TypeError> {
    let mut v = Vec::new();
    owned::reserve(&mut v, n)?;
    v.resize_with(n, Vec::new);
    Ok(v)
}
pub(super) fn id(n: usize) -> Result<u32, TypeError> {
    u32::try_from(n).map_err(|_| RequestError::PlatformCapacity.into())
}
pub(super) fn state_value(s: &SsaFacts, state: StateId) -> Option<ValueId> {
    match s.state(state)?.kind() {
        StateKind::Value(v) => Some(v),
        StateKind::Phi(p) => s.phis().get(p.0 as usize).map(|p| p.value()),
        _ => None,
    }
}
pub(super) fn issue(f: &mut TypeFacts, site: IssueSite, problem: Problem) -> Result<(), TypeError> {
    push(&mut f.issues, TypeIssue { site, problem })
}
pub(super) fn mask(family: Family) -> Families {
    Families::one(family)
}
pub(super) fn literal_families(bits: Bits) -> Families {
    let allowed = Families::width(bits.width);
    if bits.value == 0 {
        allowed
    } else {
        Families(allowed.0 & !mask(Family::Reference).0)
    }
}
pub(super) fn tag_family(tag: u16) -> Option<Family> {
    Some(match tag {
        0x5a | 0x42 | 0x53 | 0x43 | 0x49 => Family::Integral,
        0x4a => Family::Long,
        0x46 => Family::Float,
        0x44 => Family::Double,
        0x4c | 0x5b => Family::Reference,
        _ => return None,
    })
}
pub(super) fn name_family(f: &TypeFacts, n: TypeNameId) -> Option<Family> {
    f.name(n)?.units.first().copied().and_then(tag_family)
}
pub(super) fn tag_range(tag: u16) -> Option<IntegralRange> {
    let (min, max) = match tag {
        0x5a => (0, 1),
        0x42 => (-128, 127),
        0x53 => (-32768, 32767),
        0x43 => (0, 65535),
        0x49 => (i64::from(i32::MIN), i64::from(i32::MAX)),
        0x4a => (i64::MIN, i64::MAX),
        _ => return None,
    };
    Some(IntegralRange { min, max })
}
pub(super) fn add_name(
    f: &mut TypeFacts,
    units: Vec<u16>,
    origin: TypeOrigin,
) -> Result<TypeNameId, TypeError> {
    let result = TypeNameId(id(f.names.len())?);
    push(&mut f.names, TypeName { units, origin })?;
    Ok(result)
}
pub(super) fn intrinsic(f: &mut TypeFacts, text: &[u8]) -> Result<TypeNameId, TypeError> {
    if let Some(i) = f.names.iter().position(|n| {
        n.origin == TypeOrigin::Intrinsic
            && n.units
                .iter()
                .copied()
                .eq(text.iter().map(|&b| u16::from(b)))
    }) {
        return Ok(TypeNameId(id(i)?));
    }
    let mut units = Vec::new();
    owned::reserve(&mut units, text.len())?;
    units.extend(text.iter().map(|&b| u16::from(b)));
    add_name(f, units, TypeOrigin::Intrinsic)
}
