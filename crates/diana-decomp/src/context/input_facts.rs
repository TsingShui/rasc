//! Class index derived from the analysed input. Member tables are materialized
//! later, when a class is first queried.

use crate::context::Context;
use crate::input::ProgramIndex;
use crate::substrate::owned::copy_units;
use crate::{ClassDefId, RequestError, RunContext};

/// Builds a descriptor index for every class defined in one input image.
pub(crate) fn from_input<'a>(
    index: &ProgramIndex<'a>,
    control: &mut RunContext<'_>,
) -> Result<Context<'a>, RequestError> {
    let mut catalog = Context::empty();
    let mut id = 0u32;
    while id < index.header.class_defs.count {
        let class_def = ClassDefId(id);
        let class = index.class_with(class_def, control)?;
        let descriptor = copy_units(class.descriptor, control)?;
        catalog.push(
            descriptor,
            index.bytes(),
            index.header.header_offset,
            class_def,
        );
        id = id.checked_add(1).ok_or(RequestError::PlatformCapacity)?;
    }
    Ok(catalog)
}
