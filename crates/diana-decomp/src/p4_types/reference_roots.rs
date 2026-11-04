use super::normal::Kind;
use crate::p2_flow::Op;
use crate::p3_ssa::{Definition, SsaBody, Sym, ValueId};
use crate::substrate::owned::push;
use crate::{RequestError, RunContext, TypeId};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Root {
    Parameter(u32),
    Class(TypeId),
    Null,
    Definition(ValueId),
}
#[derive(Clone, Copy, Eq, PartialEq)]
enum Fact {
    None,
    One(Root),
    Many,
}
pub(crate) struct ReferenceRoots {
    facts: Vec<Fact>,
}
impl ReferenceRoots {
    pub(crate) fn one(&self, value: ValueId) -> Result<Option<Root>, RequestError> {
        Ok(
            match self
                .facts
                .get(ix(value.0)?)
                .ok_or(RequestError::InternalInvariant("foreign reference root"))?
            {
                Fact::One(root) => Some(*root),
                _ => None,
            },
        )
    }
}
fn ix(id: u32) -> Result<usize, RequestError> {
    usize::try_from(id).map_err(|_| RequestError::PlatformCapacity)
}
fn merge(a: Fact, b: Fact) -> Fact {
    match (a, b) {
        (Fact::None, b) => b,
        (a, Fact::None) => a,
        (Fact::One(a), Fact::One(b)) if a == b => Fact::One(a),
        _ => Fact::Many,
    }
}
pub(crate) fn reference(kind: Kind) -> bool {
    matches!(
        kind,
        Kind::Object
            | Kind::String
            | Kind::Class
            | Kind::Throwable
            | Kind::Nominal(_)
            | Kind::ProjectedNominal(_)
            | Kind::ReferenceArray(_)
            | Kind::ObjectArray
            | Kind::PrimitiveArray(_)
    )
}
pub(super) fn solve(
    ssa: &SsaBody<'_>,
    kinds: &[Option<Kind>],
    budget: &mut RunContext<'_>,
) -> Result<ReferenceRoots, RequestError> {
    let mut facts = Vec::new();
    for (index, definition) in ssa.values.iter().enumerate() {
        budget.checkpoint()?;
        let value = ValueId(u32::try_from(index).map_err(|_| RequestError::PlatformCapacity)?);
        let fact = match *definition {
            Definition::Phi => Fact::None,
            Definition::OpResult(id) => match ssa.operation(id)?.op {
                Op::Move(..) | Op::Cast(..) => Fact::None,
                Op::Constant(_, 0) => Fact::One(Root::Null),
                Op::Class(_, id) => Fact::One(Root::Class(id)),
                _ if kinds[index].is_some_and(reference) => Fact::One(Root::Definition(value)),
                _ => Fact::Many,
            },
            Definition::Parameter(index) if kinds[ix(value.0)?].is_some_and(reference) => {
                Fact::One(Root::Parameter(index))
            }
            Definition::ExceptionAt(_) => Fact::One(Root::Definition(value)),
            Definition::Parameter(_) => Fact::Many,
        };
        push(&mut facts, fact, budget)?;
    }
    loop {
        let mut changed = false;
        for op in &ssa.operations {
            budget.checkpoint()?;
            if !matches!(op.op, Op::Move(..) | Op::Cast(..)) {
                continue;
            }
            let value = op.result.ok_or(RequestError::InternalInvariant(
                "reference alias lost definition",
            ))?;
            let next = merge(facts[ix(value.0)?], facts[ix(op.input(0)?.0)?]);
            if next != facts[ix(value.0)?] {
                facts[ix(value.0)?] = next;
                changed = true;
            }
        }
        for phi in &ssa.phis {
            budget.checkpoint()?;
            let mut next = facts[ix(phi.result.0)?];
            for &(_, value) in &phi.inputs {
                budget.checkpoint()?;
                let Sym::Value(value) = value else {
                    return Err(RequestError::InternalInvariant("unavailable reference Phi"));
                };
                next = merge(next, facts[ix(value.0)?]);
            }
            if next != facts[ix(phi.result.0)?] {
                facts[ix(phi.result.0)?] = next;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    Ok(ReferenceRoots { facts })
}
