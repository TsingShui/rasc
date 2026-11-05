use super::build::*;
use super::*;
use crate::p3::{SsaFacts, StateId};

fn machine_read<'f>(
    s: &SsaFacts,
    f: &'f TypeFacts,
    state: StateId,
    required: Families,
    missing: Option<MissingFact>,
    meaning: ReadMeaning,
) -> Result<(Interpretation, &'f ValueFacts), Problem> {
    if !s
        .state(state)
        .is_some_and(|s| s.availability().is_available())
    {
        return Err(Problem::SourceUnavailable(state));
    }
    let value = state_value(s, state)
        .and_then(|v| f.value(v))
        .ok_or(Problem::SourceUnavailable(state))?;
    if let Some(missing) = missing {
        return Err(Problem::Missing(missing));
    }
    if let Some(bits) = value.literal() {
        let families = required.intersect(literal_families(bits));
        if families.is_conflict() {
            return Err(if required == mask(Family::Reference) {
                Problem::NonzeroReferenceLiteral
            } else {
                Problem::FamilyConflict
            });
        }
        let family = match families.selected() {
            Some(family) => family,
            None if meaning == ReadMeaning::CopyBits => {
                return Ok((Interpretation::LiteralBits(bits), value));
            }
            None if meaning == ReadMeaning::Equality && families.contains(Family::Integral) => {
                Family::Integral
            }
            None => return Err(Problem::AmbiguousRepresentation(families)),
        };
        let read = if family == Family::Reference {
            Interpretation::Null
        } else {
            Interpretation::Literal { bits, family }
        };
        return Ok((read, value));
    }
    let families = value.families.intersect(required);
    if families.is_conflict() {
        return Err(Problem::FamilyConflict);
    }
    let Some(family) = families.selected() else {
        if meaning == ReadMeaning::CopyBits {
            let width = s
                .width(value.value())
                .ok_or(Problem::SourceUnavailable(state))?;
            return Ok((Interpretation::CopyBits(width), value));
        }
        return Err(Problem::AmbiguousRepresentation(families));
    };
    Ok((Interpretation::Direct(family), value))
}

fn proven_integral_view(
    f: &TypeFacts,
    value: &ValueFacts,
    read: Interpretation,
    types: &[TypeNameId],
) -> Option<Interpretation> {
    let &declared = types.first()?;
    let bounds = f
        .name(declared)?
        .units
        .first()
        .copied()
        .and_then(tag_range)?;
    let (family, range) = match read {
        Interpretation::Direct(family @ (Family::Integral | Family::Long)) => {
            (family, value.integral_range()?)
        }
        Interpretation::Literal {
            bits,
            family: family @ (Family::Integral | Family::Long),
        } => {
            // An exact literal already supplies its machine-domain view. Select
            // a declared view only when it adds a narrower, value-preserving form.
            let domain = tag_range(match family {
                Family::Integral => u16::from(b'I'),
                _ => u16::from(b'J'),
            })?;
            if bounds == domain {
                return None;
            }
            let actual = if bits.width == crate::SlotWidth::One {
                i64::from(bits.value as u32 as i32)
            } else {
                bits.value as i64
            };
            (
                family,
                IntegralRange {
                    min: actual,
                    max: actual,
                },
            )
        }
        _ => return None,
    };
    for &wanted in types {
        if name_family(f, wanted) != Some(family) {
            return None;
        }
        let bounds = f.name(wanted)?.units.first().copied().and_then(tag_range)?;
        if range.min < bounds.min || range.max > bounds.max {
            return None;
        }
    }
    Some(Interpretation::IntegralView { declared })
}

fn interpretation(
    s: &SsaFacts,
    f: &TypeFacts,
    state: StateId,
    required: Families,
    types: &[TypeNameId],
    missing: Option<MissingFact>,
    meaning: ReadMeaning,
) -> Interpretation {
    let (read, value) = match machine_read(s, f, state, required, missing, meaning) {
        Ok(established) => established,
        Err(problem) => return Interpretation::Unknown(problem),
    };
    let read = if let Interpretation::LiteralBits(bits) = read {
        let family = types.first().and_then(|&name| name_family(f, name));
        match family {
            Some(family)
                if required.contains(family)
                    && literal_families(bits).contains(family)
                    && types
                        .iter()
                        .all(|&name| name_family(f, name) == Some(family)) =>
            {
                if family == Family::Reference {
                    Interpretation::Null
                } else {
                    Interpretation::Literal { bits, family }
                }
            }
            _ => read,
        }
    } else {
        read
    };
    proven_integral_view(f, value, read, types).unwrap_or(read)
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
                // A numeric Phi transfer copies a common word/wide bit
                // encoding; it does not evaluate an int/float interpretation.
                // Mixed numeric/reference candidates still need a real choice.
                let meaning = if families.contains(Family::Reference) {
                    ReadMeaning::Value
                } else {
                    ReadMeaning::CopyBits
                };
                interpretation(s, f, input.state(), families, &[], None, meaning)
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
