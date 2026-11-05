use super::declarations::LocalDeclarations;
use super::{InputError, InputProblem, malformed};

use crate::{ClassDefId, TypeId};

#[derive(Clone, Copy)]
enum Location {
    Missing,
    Defined(ClassDefId),
    Ambiguous,
}

/// A logical DEX's class-def locations, not validation of declaration contents.
pub(super) struct ClassLocations {
    locations: Vec<Location>,
}
impl ClassLocations {
    pub(super) fn build(index: &LocalDeclarations<'_>) -> Result<Self, InputError> {
        let offset_byte = index.header.class_defs.offset_byte;
        let length = usize::try_from(index.header.type_ids.count)
            .map_err(|_| InputError::PlatformCapacity { offset_byte })?;
        length
            .checked_mul(size_of::<Location>())
            .filter(|&size| size <= isize::MAX as usize)
            .ok_or(InputError::PlatformCapacity { offset_byte })?;

        let mut locations = Vec::new();
        locations
            .try_reserve_exact(length)
            .map_err(|_| InputError::Allocation { offset_byte })?;
        for _ in 0..length {
            locations.push(Location::Missing);
        }
        for at in 0..index.header.class_defs.count {
            let class = ClassDefId(at);
            let ty = index.class_type_id(class)?;
            // A broken unrelated definition was not a match in the original
            // table scan. Its declaration query retains the local input error.
            let Some(location) = locations.get_mut(
                usize::try_from(ty.0).map_err(|_| InputError::PlatformCapacity { offset_byte })?,
            ) else {
                continue;
            };
            *location = match location {
                Location::Missing => Location::Defined(class),
                Location::Defined(_) | Location::Ambiguous => Location::Ambiguous,
            };
        }
        Ok(Self { locations })
    }

    pub(super) fn get(
        &self,
        ty: TypeId,
        index: &LocalDeclarations<'_>,
    ) -> Result<Option<ClassDefId>, InputError> {
        let at = usize::try_from(ty.0).map_err(|_| InputError::PlatformCapacity {
            offset_byte: index.header.class_defs.offset_byte,
        })?;
        match self.locations.get(at) {
            Some(Location::Defined(class)) => Ok(Some(*class)),
            Some(Location::Ambiguous) => Err(malformed(
                InputProblem::InvalidClassData,
                index.header.class_defs.offset_byte,
            )),
            Some(Location::Missing) | None => Ok(None),
        }
    }
}
