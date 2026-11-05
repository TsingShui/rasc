use super::build::*;
use super::*;
use crate::p3::{SsaFacts, StateId};

fn interpretation(
    s: &SsaFacts,
    f: &TypeFacts,
    state: StateId,
    required: Families,
    types: &[TypeNameId],
    missing: Option<MissingFact>,
    meaning: ReadMeaning,
) -> Interpretation {
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
    if family == Family::Reference {
        return Interpretation::Direct(Family::Reference);
    }
    if let Some(&declared) = types.first() {
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
        _ => Ok(()),
    }
}
pub(super) fn finish(s: &SsaFacts, f: &mut TypeFacts) -> Result<(), TypeError> {
    for i in 0..f.uses.len() {
        let u = &f.uses[i];
        let r = &u.requirement;
        let view = interpretation(s, f, u.state, r.families, &r.types, r.missing, r.meaning);
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
                interpretation(s, f, input.state(), families, &[], None, ReadMeaning::Value)
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
