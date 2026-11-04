use crate::p4_types::Kind;
use crate::p5_variables::{Initializer, Parameter, StorageBody};
use crate::substrate::owned::push;
use crate::{RequestError, RunContext};

#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum InitialValue {
    This,
    Parameter(u32),
    Outer,
    Literal(&'static str),
    Absent,
}
pub(crate) struct Local {
    pub id: super::branch::LocalId,
    pub(super) kind: Kind,
    pub(super) initializer: Initializer,
    pub source_type: super::types::SourceType,
    pub value: InitialValue,
}
pub(super) fn lower(
    storage: &StorageBody<'_>,
    names: &super::types::TypeNames,
    budget: &mut RunContext<'_>,
) -> Result<Vec<Local>, RequestError> {
    let mut result = Vec::new();
    for local in &storage.locals {
        budget.checkpoint()?;
        let value = match local.initializer {
            Initializer::ThisAfterChain | Initializer::Parameter(Parameter::This) => {
                InitialValue::This
            }
            Initializer::Parameter(Parameter::Explicit(index)) => {
                if storage.typed.capture.is_some() && index == 0 {
                    InitialValue::Outer
                } else {
                    InitialValue::Parameter(index)
                }
            }
            Initializer::Deferred(_) => InitialValue::Absent,
            Initializer::Default => InitialValue::Literal(match local.kind {
                Kind::Int => "0",
                Kind::Boolean => "false",
                Kind::Float => "0.0f",
                Kind::Long => "0L",
                Kind::Double => "0.0d",
                _ => "null",
            }),
        };
        push(
            &mut result,
            Local {
                id: local.id,
                kind: local.kind,
                initializer: local.initializer,
                source_type: names.source_type(local.kind, budget)?,
                value,
            },
            budget,
        )?;
    }
    Ok(result)
}
pub(super) fn verify(
    local: &Local,
    names: &super::types::TypeNames,
    budget: &mut RunContext<'_>,
) -> Result<bool, RequestError> {
    if names.source_name(local.source_type)? != names.name(local.kind, budget)? {
        return Ok(false);
    }
    Ok(match (local.initializer, local.value) {
        (
            Initializer::ThisAfterChain | Initializer::Parameter(Parameter::This),
            InitialValue::This,
        ) => true,
        (Initializer::Parameter(Parameter::Explicit(0)), InitialValue::Outer) => true,
        (Initializer::Parameter(Parameter::Explicit(a)), InitialValue::Parameter(b)) => a == b,
        (Initializer::Deferred(_), InitialValue::Absent) => true,
        (Initializer::Default, InitialValue::Literal(literal)) => match local.kind {
            Kind::Int => literal == "0",
            Kind::Boolean => literal == "false",
            Kind::Float => literal == "0.0f",
            Kind::Long => literal == "0L",
            Kind::Double => literal == "0.0d",
            Kind::Object
            | Kind::Throwable
            | Kind::Class
            | Kind::String
            | Kind::PrimitiveArray(_)
            | Kind::ObjectArray
            | Kind::Nominal(_)
            | Kind::ReferenceArray(_)
            | Kind::ProjectedNominal(_) => literal == "null",
        },
        _ => false,
    })
}
