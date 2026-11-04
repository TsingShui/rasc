use super::{DexString, MethodSource};
use crate::{RequestError, RunContext, TypeId};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct TypeProjection {
    root: TypeId,
    removed: u32,
}
impl TypeProjection {
    pub(crate) fn root(self) -> TypeId {
        self.root
    }
    pub(crate) fn removed(self) -> u32 {
        self.removed
    }
    pub(crate) fn identity(self) -> u128 {
        (u128::from(self.removed) << 32) | u128::from(self.root.0)
    }
}
pub(super) fn original(root: TypeId) -> TypeProjection {
    TypeProjection { root, removed: 0 }
}
pub(super) fn component(
    input: &MethodSource<'_, '_>,
    projection: TypeProjection,
    budget: &mut RunContext<'_>,
) -> Result<TypeProjection, RequestError> {
    let name = input.projected_name(projection, budget)?;
    if name.reader()?.next(budget)? != Some(91) {
        return Err(RequestError::InternalInvariant(
            "component requested for a non-array",
        ));
    }
    let result = TypeProjection {
        root: projection.root,
        removed: projection
            .removed
            .checked_add(1)
            .ok_or(RequestError::PlatformCapacity)?,
    };
    let suffix = input.projected_name(result, budget)?;
    if let Some(id) = input.exact_type(suffix, budget)? {
        Ok(original(id))
    } else {
        Ok(result)
    }
}
pub(super) fn suffix<'a>(
    name: DexString<'a>,
    projection: TypeProjection,
    budget: &mut RunContext<'_>,
) -> Result<DexString<'a>, RequestError> {
    Ok(name.array_suffix(projection.removed, budget)?)
}
