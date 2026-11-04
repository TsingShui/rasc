use crate::input::MethodSource;
use crate::substrate::owned::push;
use crate::{MethodId, RequestError, RunContext};

pub(crate) fn original_formals(
    input: &MethodSource<'_, '_>,
    id: MethodId,
    budget: &mut RunContext<'_>,
) -> Result<Vec<super::DeclarationType>, RequestError> {
    let reference = input.method_reference(id, budget)?;
    from_reference(input.program(), &reference.declaration.prototype, budget)
}

pub(crate) fn from_reference(
    index: &crate::input::ProgramIndex<'_>,
    prototype: &crate::input::Prototype<'_, '_>,
    budget: &mut RunContext<'_>,
) -> Result<Vec<super::DeclarationType>, RequestError> {
    let mut formals = Vec::new();
    prototype.visit_parameter_type_ids(budget, |id, name, budget| -> Result<(), RequestError> {
        let projection = index.type_projection(id, budget)?;
        let formal = super::normal::declaration_from_descriptor(name, projection, budget)?.ok_or(
            RequestError::InternalInvariant("checked call parameter lost declaration type"),
        )?;
        push(&mut formals, formal, budget)
    })?;
    Ok(formals)
}
