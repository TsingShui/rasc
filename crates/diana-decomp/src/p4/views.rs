use super::build::*;
use super::*;
use crate::p3::{SsaMethod, StateId};

fn original_type(origin: TypeOrigin, owner: &crate::p0::OwnerFacts) -> Option<crate::TypeId> {
    match origin {
        TypeOrigin::Owner { key, original } if key == owner.key() => Some(original),
        TypeOrigin::Parameter { original, .. }
        | TypeOrigin::Catch { original }
        | TypeOrigin::Return { original }
        | TypeOrigin::Reference {
            original: Some(original),
            ..
        } => Some(original),
        _ => None,
    }
}

fn direct_superclass(source: &TypeName, target: &TypeName, owner: &crate::p0::OwnerFacts) -> bool {
    matches!(owner.superclass(), crate::p0::Fact::Present(_))
        && original_type(source.origin, owner) == Some(owner.type_id())
        && owner.superclass_type_id().is_some()
        && original_type(target.origin, owner) == owner.superclass_type_id()
}

fn interpretation(
    ssa: &SsaMethod<'_>,
    f: &TypeFacts,
    state: StateId,
    required: Families,
    types: &[TypeNameId],
    missing: Option<MissingFact>,
    meaning: ReadMeaning,
) -> Interpretation {
    let s = ssa.facts();
    if !s
        .state(state)
        .is_some_and(|s| s.availability().is_available())
    {
        return Interpretation::Unknown(Problem::SourceUnavailable(state));
    }
    let Some(value) = state_value(s, state).and_then(|v| f.value(v)) else {
        return Interpretation::Unknown(Problem::SourceUnavailable(state));
    };
    if let Some(missing) = missing {
        return Interpretation::Unknown(Problem::Missing(missing));
    }
    if let Some(bits) = value.literal() {
        let mut families = required.intersect(Families::width(bits.width));
        if bits.value != 0 {
            families = Families(families.0 & !mask(Family::Reference).0);
        }
        if families.is_conflict() {
            return Interpretation::Unknown(if required == mask(Family::Reference) {
                Problem::NonzeroReferenceLiteral
            } else {
                Problem::FamilyConflict
            });
        }
        let family = match families.selected() {
            Some(family) => family,
            None if meaning == ReadMeaning::CopyBits => return Interpretation::LiteralBits(bits),
            None if meaning == ReadMeaning::Equality && families.contains(Family::Integral) => {
                Family::Integral
            }
            None => return Interpretation::Unknown(Problem::AmbiguousRepresentation(families)),
        };
        if family == Family::Reference {
            return Interpretation::Null;
        }
        if matches!(family, Family::Integral | Family::Long) {
            let actual = if bits.width == crate::SlotWidth::One {
                i64::from(bits.value as u32 as i32)
            } else {
                bits.value as i64
            };
            for &name in types {
                if let Some(range) = f.names[name.0 as usize]
                    .units
                    .first()
                    .copied()
                    .and_then(tag_range)
                    && (actual < range.min || actual > range.max)
                {
                    return Interpretation::Unknown(Problem::LiteralOutOfRange);
                }
            }
        }
        return Interpretation::Literal { bits, family };
    }
    let family = value.families.intersect(required);
    if family.is_conflict() {
        return Interpretation::Unknown(Problem::FamilyConflict);
    }
    let Some(family) = family.selected() else {
        if meaning == ReadMeaning::CopyBits {
            return match s.width(value.value()) {
                Some(width) => Interpretation::CopyBits(width),
                None => Interpretation::Unknown(Problem::SourceUnavailable(state)),
            };
        }
        return Interpretation::Unknown(Problem::AmbiguousRepresentation(family));
    };
    if let Some(&declared) = types.first() {
        if family == Family::Reference {
            let mut relation = Relation::Same;
            for &wanted in types {
                let target = &f.names[wanted.0 as usize].units;
                let object = target
                    .iter()
                    .copied()
                    .eq(b"Ljava/lang/Object;".iter().map(|&b| u16::from(b)));
                if object {
                    if relation == Relation::Same {
                        relation = Relation::WidenToObject;
                    }
                    continue;
                }
                if value.guarantees_exception_object()
                    && target
                        .iter()
                        .copied()
                        .eq(b"Ljava/lang/Throwable;".iter().map(|&b| u16::from(b)))
                {
                    if relation != Relation::Unknown {
                        relation = Relation::ExceptionUpperBound;
                    }
                    continue;
                }
                if value.type_unknown || value.types.is_empty() {
                    relation = Relation::Unknown;
                    continue;
                }
                let owner = ssa.execution().owner_facts();
                for &source in &value.types {
                    let source = &f.names[source.0 as usize];
                    if source.units == *target {
                        continue;
                    }
                    if direct_superclass(source, &f.names[wanted.0 as usize], owner) {
                        if relation != Relation::Unknown {
                            relation = Relation::DirectSuperclass;
                        }
                    } else {
                        relation = Relation::Unknown;
                    }
                }
            }
            return Interpretation::Reference { declared, relation };
        }
        for &wanted in types {
            if let Some(bounds) = f.names[wanted.0 as usize]
                .units
                .first()
                .copied()
                .and_then(tag_range)
            {
                let Some(range) = value.integral_range() else {
                    return Interpretation::Unknown(Problem::NarrowingRange);
                };
                if range.min < bounds.min || range.max > bounds.max {
                    return Interpretation::Unknown(Problem::NarrowingRange);
                }
            }
        }
        if matches!(family, Family::Integral | Family::Long) {
            return Interpretation::IntegralView { declared };
        }
    }
    Interpretation::Direct(family)
}
fn record(f: &mut TypeFacts, site: IssueSite, view: Interpretation) -> Result<(), TypeError> {
    match view {
        Interpretation::Unknown(problem) => issue(f, site, problem),
        Interpretation::Reference {
            relation: Relation::Unknown,
            ..
        } => issue(f, site, Problem::Missing(MissingFact::Hierarchy)),
        _ => Ok(()),
    }
}
pub(super) fn finish(ssa: &SsaMethod<'_>, f: &mut TypeFacts) -> Result<(), TypeError> {
    let s = ssa.facts();
    for i in 0..f.uses.len() {
        let u = &f.uses[i];
        let r = &u.requirement;
        let view = interpretation(ssa, f, u.state, r.families, &r.types, r.missing, r.meaning);
        f.uses[i].interpretation = view;
        record(f, IssueSite::Use(f.uses[i].site), view)?;
    }
    for phi in s.phis() {
        let families = f.values[phi.value().0 as usize].families;
        let mut incoming = Vec::new();
        for input in phi.incoming() {
            let view = if let Some(bits) = f.values[phi.value().0 as usize].literal() {
                if s.state(input.state())
                    .is_some_and(|s| s.availability().is_available())
                {
                    Interpretation::LiteralBits(bits)
                } else {
                    Interpretation::Unknown(Problem::SourceUnavailable(input.state()))
                }
            } else {
                interpretation(
                    ssa,
                    f,
                    input.state(),
                    families,
                    &[],
                    None,
                    ReadMeaning::Value,
                )
            };
            record(
                f,
                IssueSite::Incoming {
                    phi: phi.id(),
                    transfer: input.transfer(),
                },
                view,
            )?;
            push(
                &mut incoming,
                IncomingView {
                    transfer: input.transfer(),
                    state: input.state(),
                    interpretation: view,
                },
            )?;
        }
        push(
            &mut f.phis,
            PhiView {
                phi: phi.id(),
                representation: match f.values[phi.value().0 as usize].literal() {
                    Some(bits) => PhiRepresentation::Literal(bits),
                    None => PhiRepresentation::Runtime(families),
                },
                families,
                incoming,
            },
        )?;
    }
    Ok(())
}
