use super::normal::Kind;
use crate::p2_flow::{ArrayKind, DeclarationType};
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
    pub(crate) fn range(self) -> Option<crate::p5_types::IntRange> {
        use crate::p5_types::IntRange;
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
impl ArrayKind {
    pub(crate) fn kind(self) -> Kind {
        self.declaration().kind()
    }
}
