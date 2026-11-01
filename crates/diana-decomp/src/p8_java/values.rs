use crate::p5_types::normal::UseConversion;
use crate::p6_variables::LocalId;

#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum ValueUse {
    Null {
        value: LocalId,
        kind: crate::p5_types::BuiltinReference,
    },
    UpcastObject(LocalId),
    Direct(LocalId),
    Boolean01(LocalId),
    BooleanWord(LocalId),
    Narrow {
        value: LocalId,
        kind: crate::p5_types::Narrow,
    },
}
impl ValueUse {
    pub(crate) fn value(self) -> LocalId {
        match self {
            Self::Direct(value)
            | Self::Boolean01(value)
            | Self::BooleanWord(value)
            | Self::Narrow { value, .. }
            | Self::UpcastObject(value)
            | Self::Null { value, .. } => value,
        }
    }
}
pub(crate) fn lower(local: LocalId, conversion: UseConversion) -> ValueUse {
    match conversion {
        UseConversion::Identity => ValueUse::Direct(local),
        UseConversion::BooleanWord => ValueUse::BooleanWord(local),
        UseConversion::Boolean01 => ValueUse::Boolean01(local),
        UseConversion::Null(kind) => ValueUse::Null { value: local, kind },
        UseConversion::UpcastObject => ValueUse::UpcastObject(local),
        UseConversion::Narrow(kind) => ValueUse::Narrow { value: local, kind },
    }
}
