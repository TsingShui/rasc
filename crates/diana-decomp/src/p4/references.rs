use super::build::*;
use super::constraints::{RuleKind, Work};
use super::dataflow::Dataflow;
use super::*;
use crate::SlotWidth;
use crate::p2::OpKind;
use crate::p3::{PhiId, PointId, SsaMethod, StateId, TransferId, ValueId, ValueOrigin};

pub(super) const SAME: u32 = 1;
pub(super) const DIFFERENT: u32 = 2;
pub(super) const MAY_ALIAS: u32 = 4;

#[derive(Clone, Copy)]
pub(super) enum Source {
    Leaf,
    Alias { input: StateId },
    Phi(PhiId),
}

pub(super) struct References<'s, 'a> {
    pub ssa: &'s SsaMethod<'a>,
    pub(super) values: &'s [ValueFacts],
    pub(super) names: &'s [TypeName],
    work: &'s Work,
    rule: Vec<Option<usize>>,
}
impl<'s, 'a> References<'s, 'a> {
    pub fn new(
        ssa: &'s SsaMethod<'a>,
        values: &'s [ValueFacts],
        names: &'s [TypeName],
        work: &'s Work,
    ) -> Result<Self, TypeError> {
        let mut rule = filled(ssa.facts().values().len(), None)?;
        for (index, r) in work.rules.iter().enumerate() {
            rule[r.target.0 as usize] = Some(index);
        }
        Ok(Self {
            ssa,
            values,
            names,
            work,
            rule,
        })
    }
    pub fn value(&self, state: StateId) -> Option<ValueId> {
        state_value(self.ssa.facts(), state)
    }
    pub fn available(&self, state: StateId) -> bool {
        self.ssa
            .facts()
            .state(state)
            .is_some_and(|s| s.availability().is_available())
    }
    pub fn definition(&self, state: StateId) -> Option<PointId> {
        self.value(state)
            .map(|v| self.ssa.facts().values()[v.0 as usize].point())
    }
    pub fn source(&self, state: StateId) -> Result<Source, TypeError> {
        let Some(value) = self.value(state) else {
            return Ok(Source::Leaf);
        };
        if let ValueOrigin::Phi(phi) = self.ssa.facts().values()[value.0 as usize].origin() {
            return Ok(Source::Phi(phi));
        }
        if let Some(index) = self.rule[value.0 as usize] {
            let rule = &self.work.rules[index];
            if rule.kind == RuleKind::Copy {
                let [input] = rule.inputs.as_slice() else {
                    return Err(TypeError::InternalInvariant(
                        "reference-preserving operation has no single SSA source",
                    ));
                };
                return Ok(Source::Alias { input: *input });
            }
        }
        Ok(Source::Leaf)
    }
    pub fn is_null(&self, state: StateId) -> bool {
        self.available(state)
            && self.value(state).is_some_and(|v| {
                self.values[v.0 as usize]
                    .literal()
                    .is_some_and(|b| b.width == SlotWidth::One && b.value == 0)
            })
    }
    pub fn fresh(&self, state: StateId) -> bool {
        self.value(state).is_some_and(|v| {
            matches!(self.ssa.facts().values()[v.0 as usize].origin(), ValueOrigin::Operation { operation, .. }
                if matches!(self.ssa.execution().operations()[operation.0 as usize].kind(),
                    OpKind::NewInstance { .. } | OpKind::NewArray { .. } | OpKind::FilledNewArray { .. }))
        })
    }
    pub fn rewrite(
        &self,
        state: StateId,
        point: PointId,
        transfer: TransferId,
    ) -> Result<StateId, TypeError> {
        if self.definition(state) != Some(point) {
            return Ok(state);
        }
        match self.source(state)? {
            Source::Alias { input } => Ok(input),
            Source::Phi(phi) => self.ssa.facts().phis()[phi.0 as usize]
                .incoming()
                .iter()
                .find(|i| i.transfer() == transfer)
                .map(|i| i.state())
                .ok_or(TypeError::InternalInvariant(
                    "reference Phi lost its original incoming transfer",
                )),
            Source::Leaf => Ok(state),
        }
    }
    pub fn canonical<E>(
        &self,
        state: StateId,
        other: Option<StateId>,
        mut point: PointId,
        incoming: &[Vec<TransferId>],
        events: Option<&[Option<E>]>,
    ) -> PointId {
        let definition = self.definition(state);
        let other_definition = other.and_then(|s| self.definition(s));
        // Eliminate identity equations only. Definitions capture reference
        // occurrences; joins and relevant events retain their original edges.
        // The arena-sized walk also terminates on an ungrounded degree-one cycle.
        for _ in self.ssa.facts().points() {
            if definition == Some(point) || other_definition == Some(point) {
                break;
            }
            let [transfer] = incoming[point.0 as usize].as_slice() else {
                break;
            };
            let from = self.ssa.facts().transfers()[transfer.0 as usize].from();
            if events.is_some_and(|e| e[from.0 as usize].is_some()) {
                break;
            }
            point = from;
        }
        point
    }
}

#[derive(Clone, Copy, Eq, Hash, PartialEq)]
pub(super) struct AliasQuery {
    a: StateId,
    b: StateId,
    point: PointId,
}
pub(super) type Aliases = Dataflow<AliasQuery, ()>;
pub(super) fn alias_query(
    refs: &References<'_, '_>,
    incoming: &[Vec<TransferId>],
    graph: &mut Aliases,
    a: StateId,
    b: StateId,
    point: PointId,
) -> Result<usize, TypeError> {
    let (a, b) = if a < b { (a, b) } else { (b, a) };
    let point = refs.canonical::<()>(a, Some(b), point, incoming, None);
    graph.intern(AliasQuery { a, b, point })
}
pub(super) fn aliases(
    refs: &References<'_, '_>,
    incoming: &[Vec<TransferId>],
    graph: &mut Aliases,
) -> Result<(), TypeError> {
    let mut index = 0;
    while index < graph.nodes.len() {
        if graph.nodes[index].seed != 0 {
            index += 1;
            continue;
        }
        let AliasQuery { a, b, point } = graph.nodes[index].key;
        if !refs.available(a) || !refs.available(b) {
            graph.nodes[index].seed = MAY_ALIAS;
        } else if refs.value(a).is_some_and(|v| Some(v) == refs.value(b))
            || (refs.is_null(a) && refs.is_null(b))
        {
            graph.nodes[index].seed = SAME;
        } else {
            let a_birth =
                refs.definition(a) == Some(point) && matches!(refs.source(a)?, Source::Leaf);
            let b_birth =
                refs.definition(b) == Some(point) && matches!(refs.source(b)?, Source::Leaf);
            if a_birth || b_birth {
                // A fresh normal result cannot alias a reference already present
                // at allocation execute. Static allocation-site equality is not
                // used as an instance-identity proof across a backedge.
                let fresh = |x, y| {
                    refs.fresh(x)
                        && (refs.is_null(y)
                            || refs.value(x).is_some_and(|v| {
                                let ValueOrigin::Operation { operation, .. } =
                                    refs.ssa.facts().values()[v.0 as usize].origin()
                                else {
                                    return false;
                                };
                                refs.ssa
                                    .facts()
                                    .operation_site(operation)
                                    .is_some_and(|site| {
                                        refs.definition(y).is_some_and(|p| {
                                            refs.ssa.facts().dominates(p, site.execute())
                                        })
                                    })
                            }))
                };
                graph.nodes[index].seed = if (a_birth && fresh(a, b)) || (b_birth && fresh(b, a)) {
                    DIFFERENT
                } else {
                    MAY_ALIAS
                };
            } else {
                for &transfer in &incoming[point.0 as usize] {
                    let a = refs.rewrite(a, point, transfer)?;
                    let b = refs.rewrite(b, point, transfer)?;
                    let from = refs.ssa.facts().transfers()[transfer.0 as usize].from();
                    let source = alias_query(refs, incoming, graph, a, b, from)?;
                    graph.link(index, source, ())?;
                }
            }
        }
        index += 1;
    }
    graph.solve(|_| MAY_ALIAS, |(), bits| bits)
}
