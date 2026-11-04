use crate::p4_types::UseConversion;
use crate::p5_variables::LocalId;

#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum ValueUse {
    DoubleLiteral {
        value: LocalId,
        bits: u64,
    },
    FloatLiteral {
        value: LocalId,
        bits: u32,
    },
    UpcastObject(LocalId),
    Direct(LocalId),
    NullAssignment(LocalId),
    ObjectArrayView(LocalId),
    Boolean01(LocalId),
    BooleanWord(LocalId),
    Narrow {
        value: LocalId,
        kind: crate::p4_types::Narrow,
    },
}
impl ValueUse {
    pub(crate) fn value(self) -> LocalId {
        match self {
            Self::Direct(value)
            | Self::NullAssignment(value)
            | Self::ObjectArrayView(value)
            | Self::Boolean01(value)
            | Self::BooleanWord(value)
            | Self::Narrow { value, .. }
            | Self::UpcastObject(value)
            | Self::DoubleLiteral { value, .. }
            | Self::FloatLiteral { value, .. } => value,
        }
    }
}

pub(super) fn verify_reference_store(
    typed: &crate::p4_types::TypedBody<'_>,
    actual: ValueUse,
    source: crate::p3_ssa::ValueId,
    target: crate::p2_flow::DeclarationType,
    budget: &mut crate::RunContext<'_>,
) -> Result<bool, crate::RequestError> {
    use crate::p4_types::Kind;
    let at = usize::try_from(source.0).map_err(|_| crate::RequestError::PlatformCapacity)?;
    if target.field_offset() != 2 || actual.value().0 != source.0 {
        return Ok(false);
    }
    Ok(match actual {
        ValueUse::NullAssignment(_) => crate::p4_types::zero::proven(&typed.ssa, source, budget)?,
        ValueUse::Direct(_) => {
            typed.kinds[at].is_some_and(crate::p4_types::reference_roots::reference)
        }
        ValueUse::UpcastObject(_) => {
            target.kind() == Kind::Object
                && typed.kinds[at].is_some_and(crate::p4_types::reference_roots::reference)
        }
        _ => false,
    })
}

pub(super) fn verify_primitive_store(
    typed: &crate::p4_types::TypedBody<'_>,
    actual: ValueUse,
    source: crate::p3_ssa::ValueId,
    target: crate::p2_flow::DeclarationType,
) -> Result<bool, crate::RequestError> {
    use crate::p2_flow::DeclarationType;
    use crate::p4_types::Kind;
    let at = usize::try_from(source.0).map_err(|_| crate::RequestError::PlatformCapacity)?;
    if actual.value().0 != source.0 {
        return Ok(false);
    }
    Ok(match target {
        DeclarationType::Boolean => match actual {
            ValueUse::Direct(_) => typed.kinds[at] == Some(Kind::Boolean),
            ValueUse::Boolean01(_) => {
                typed.kinds[at] == Some(Kind::Int)
                    && typed.integral_bits[at] != 0
                    && typed.integral_bits[at] & !3 == 0
            }
            _ => false,
        },
        DeclarationType::Narrow(expected) => {
            matches!(actual, ValueUse::Narrow { kind, .. } if kind == expected)
                && typed.kinds[at] == Some(Kind::Int)
        }
        _ => super::dispatcher_verify::numeric_view(typed, actual, Some(&source), target.kind())?,
    })
}

pub(crate) fn lower(local: LocalId, conversion: UseConversion) -> ValueUse {
    match conversion {
        UseConversion::DoubleLiteral(bits) => ValueUse::DoubleLiteral { value: local, bits },
        UseConversion::FloatLiteral(bits) => ValueUse::FloatLiteral { value: local, bits },
        UseConversion::Identity => ValueUse::Direct(local),
        UseConversion::NullAssignment => ValueUse::NullAssignment(local),
        UseConversion::BooleanWord => ValueUse::BooleanWord(local),
        UseConversion::Boolean01 => ValueUse::Boolean01(local),
        UseConversion::UpcastObject => ValueUse::UpcastObject(local),
        UseConversion::Narrow(kind) => ValueUse::Narrow { value: local, kind },
    }
}
