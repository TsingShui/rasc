use crate::input::MethodInput;
use crate::substrate::owned::push;
use crate::{MethodBudget, MethodId, RequestError};

#[derive(Clone, Copy)]
pub(crate) enum CallFormal {
    Numeric(crate::p2_flow::primitives::ScalarType),
    Int,
    Boolean,
    Narrow(crate::substrate::primitive_types::Narrow),
    Nominal(crate::TypeId),
    Reference(crate::substrate::primitive_types::BuiltinReference),
}
pub(crate) fn original_formals(
    input: &MethodInput<'_, '_>,
    id: MethodId,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<Option<Vec<CallFormal>>, RequestError> {
    let reference = input.method_reference(id, budget)?;
    Ok(Some(from_reference(
        &reference.declaration.prototype,
        budget,
    )?))
}

pub(crate) fn from_reference(
    prototype: &crate::input::Prototype<'_, '_>,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<Vec<CallFormal>, RequestError> {
    let mut formals = Vec::new();
    prototype.visit_parameter_type_ids(budget, |id, name, budget| -> Result<(), RequestError> {
        use crate::substrate::primitive_types::Narrow;
        let formal = if name.matches_with(&[73], budget)? {
            CallFormal::Int
        } else if name.matches_with(&[74], budget)? {
            CallFormal::Numeric(crate::p2_flow::primitives::ScalarType::Long)
        } else if name.matches_with(&[70], budget)? {
            CallFormal::Numeric(crate::p2_flow::primitives::ScalarType::Float)
        } else if name.matches_with(&[68], budget)? {
            CallFormal::Numeric(crate::p2_flow::primitives::ScalarType::Double)
        } else if name.matches_with(&[90], budget)? {
            CallFormal::Boolean
        } else if name.matches_with(&[66], budget)? {
            CallFormal::Narrow(Narrow::Byte)
        } else if name.matches_with(&[83], budget)? {
            CallFormal::Narrow(Narrow::Short)
        } else if name.matches_with(&[67], budget)? {
            CallFormal::Narrow(Narrow::Char)
        } else if name.matches_with(
            crate::substrate::primitive_types::BuiltinReference::Object.descriptor(),
            budget,
        )? {
            CallFormal::Reference(crate::substrate::primitive_types::BuiltinReference::Object)
        } else if name.matches_with(
            crate::substrate::primitive_types::BuiltinReference::String.descriptor(),
            budget,
        )? {
            CallFormal::Reference(crate::substrate::primitive_types::BuiltinReference::String)
        } else {
            CallFormal::Nominal(id)
        };
        push(&mut formals, formal, budget)
    })?;
    Ok(formals)
}
