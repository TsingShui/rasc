use super::normal::Kind;
use crate::p2_flow::{ArrayKind, DeclarationType};
use crate::{RequestError, RunContext};
impl DeclarationType {
    pub(crate) fn kind(self) -> Kind {
        match self {
            Self::Int | Self::Narrow(_) => Kind::Int,
            Self::Boolean => Kind::Boolean,
            Self::Object => Kind::Object,
            Self::String => Kind::String,
            Self::Float => Kind::Float,
            Self::Long => Kind::Long,
            Self::Double => Kind::Double,
            Self::PrimitiveArray(kind) => Kind::PrimitiveArray(kind),
            Self::ObjectArray => Kind::ObjectArray,
            Self::Nominal(id) => Kind::Nominal(id),
            Self::ReferenceArray(key) => Kind::ReferenceArray(key),
            Self::ProjectedNominal(key) => Kind::ProjectedNominal(key),
        }
    }
    pub(crate) fn range(self) -> Option<crate::p4_types::IntRange> {
        use crate::p4_types::IntRange;
        match self {
            Self::Int => Some(IntRange::ALL),
            Self::Boolean => Some(IntRange::BOOLEAN),
            Self::Narrow(kind) => Some(kind.range()),
            Self::Object
            | Self::String
            | Self::Float
            | Self::Long
            | Self::Double
            | Self::PrimitiveArray(_)
            | Self::ObjectArray
            | Self::Nominal(_)
            | Self::ReferenceArray(_)
            | Self::ProjectedNominal(_) => None,
        }
    }
}
impl Kind {
    pub(crate) fn reference_descriptor(
        self,
        input: &crate::input::MethodSource<'_, '_>,
        budget: &mut RunContext<'_>,
    ) -> Result<Vec<u16>, RequestError> {
        let units = match self {
            Self::Nominal(id) => {
                return crate::substrate::owned::copy_units(
                    input.type_descriptor(id, budget)?,
                    budget,
                );
            }
            Self::ReferenceArray(key) | Self::ProjectedNominal(key) => {
                return crate::substrate::owned::copy_units(
                    input.projected_name(key, budget)?,
                    budget,
                );
            }
            Self::Object => super::BuiltinReference::Object.descriptor(),
            Self::String => super::BuiltinReference::String.descriptor(),
            Self::Class => &const { crate::substrate::owned::ascii_units(*b"Ljava/lang/Class;") },
            Self::Throwable => {
                &const { crate::substrate::owned::ascii_units(*b"Ljava/lang/Throwable;") }
            }
            Self::PrimitiveArray(kind) => kind.descriptor(),
            Self::ObjectArray => ArrayKind::object_descriptor(),
            _ => {
                return Err(RequestError::InternalInvariant(
                    "numeric kind requested reference descriptor",
                ));
            }
        };
        let mut result = Vec::new();
        for &unit in units {
            crate::substrate::owned::push(&mut result, unit, budget)?;
        }
        Ok(result)
    }
}

impl ArrayKind {
    pub(crate) fn kind(self) -> Kind {
        self.declaration().kind()
    }
}
