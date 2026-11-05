use super::build::*;
use super::constraints::Work;
use super::references::*;
use super::*;
use crate::RequestError;
use crate::p2::{ArrayAccess, EdgeRole, OpKind, ValueKind};
use crate::p3::{PointId, SsaMethod, StateId, Target, TransferId, TransferOrigin, ValueId};
use std::collections::HashMap;

// Projected bounds describe non-null alternatives. Null-only source paths are
// grounded but supply no element type or evidence of a normal array execution.
#[derive(Clone, Copy, Eq, PartialEq)]
struct Projection {
    name: TypeNameId,
    depth: u32,
}
#[derive(Default)]
struct Bounds {
    names: Vec<Projection>,
    gaps: Vec<ComponentGap>,
    null: bool,
}
impl Bounds {
    fn merge(&mut self, other: &Self, refined: Option<Projection>) -> Result<bool, TypeError> {
        let mut changed = false;
        for &name in &other.names {
            if !self.names.contains(&name) {
                push(&mut self.names, name)?;
                changed = true;
            }
        }
        for &gap in &other.gaps {
            // A successful check supplies a missing declaration bound, not a
            // missing value. Availability damage cannot be discharged by it.
            if refined.is_some() && !matches!(gap, ComponentGap::SourceUnavailable(_)) {
                continue;
            }
            if !self.gaps.contains(&gap) {
                push(&mut self.gaps, gap)?;
                changed = true;
            }
        }
        // Null satisfies every check-cast but supplies no element type. Wait
        // for a non-null alternative (also excluding construction bottom) before
        // using a successful check as component evidence.
        if let Some(name) = refined
            && (!other.names.is_empty() || !other.gaps.is_empty())
            && !self.names.contains(&name)
        {
            push(&mut self.names, name)?;
            changed = true;
        }
        if other.null && !self.null {
            self.null = true;
            changed = true;
        }
        Ok(changed)
    }
    fn is_bottom(&self) -> bool {
        self.names.is_empty() && self.gaps.is_empty() && !self.null
    }
    fn knowledge(&self) -> Result<Self, TypeError> {
        let mut result = Self::default();
        result.merge(self, None)?;
        if result.names.is_empty() && result.gaps.is_empty() {
            push(&mut result.gaps, ComponentGap::NoDeclaration)?;
        }
        Ok(result)
    }
}

#[derive(Clone, Copy, Eq, Hash, PartialEq)]
struct Query {
    state: StateId,
    point: PointId,
    depth: u32,
}
#[derive(Clone, Copy)]
struct Link {
    source: usize,
    refinement: Option<(usize, Projection)>,
}
struct Node {
    key: Query,
    seed: Bounds,
    result: Bounds,
    links: Vec<Link>,
    users: Vec<usize>,
}
struct Solver<'s, 'a> {
    refs: References<'s, 'a>,
    work: &'s Work,
    depth_limit: u32,
    components: Vec<Option<(StateId, PointId)>>,
    incoming: Vec<Vec<TransferId>>,
    events: Vec<Option<usize>>,
    aliases: Aliases,
    index: HashMap<Query, usize>,
    nodes: Vec<Node>,
}
impl Solver<'_, '_> {
    fn project(&self, name: TypeNameId, depth: u32) -> Option<Projection> {
        let units = &self.refs.names[name.0 as usize].units;
        let prefix = units.get(..depth as usize)?;
        (prefix.len() < units.len() && prefix.iter().all(|&u| u == u16::from(b'[')))
            .then_some(Projection { name, depth })
    }
    fn query(&mut self, state: StateId, point: PointId, depth: u32) -> Result<usize, TypeError> {
        let point = if self.work.casts.is_empty() {
            self.refs.definition(state).unwrap_or(point)
        } else {
            self.refs
                .canonical(state, None, point, &self.incoming, Some(&self.events))
        };
        // Beyond all recorded array ranks, further projections have no nominal
        // evidence. This single abstract depth also closes component cycles;
        // it is derived from input descriptors, not a numeric analysis quota.
        let key = Query {
            state,
            point,
            depth: depth.min(self.depth_limit),
        };
        if let Some(&index) = self.index.get(&key) {
            return Ok(index);
        }
        self.index
            .try_reserve(1)
            .map_err(RequestError::Allocation)?;
        let index = self.nodes.len();
        push(
            &mut self.nodes,
            Node {
                key,
                seed: Bounds::default(),
                result: Bounds::default(),
                links: Vec::new(),
                users: Vec::new(),
            },
        )?;
        self.index.insert(key, index);
        Ok(index)
    }
    fn link(
        &mut self,
        target: usize,
        source: usize,
        refinement: Option<(usize, Projection)>,
    ) -> Result<(), TypeError> {
        push(&mut self.nodes[target].links, Link { source, refinement })?;
        push(&mut self.nodes[source].users, target)
    }
    fn normal(&self, transfer: TransferId) -> bool {
        match self.refs.ssa.facts().transfers()[transfer.0 as usize].origin() {
            TransferOrigin::Edge(edge) => matches!(
                self.refs.ssa.execution().edges()[edge.0 as usize].role(),
                EdgeRole::Normal(_)
            ),
            TransferOrigin::Continue | TransferOrigin::NormalCommit(_) => true,
            TransferOrigin::Entry | TransferOrigin::Return(_) => false,
        }
    }
    fn seed(&mut self, index: usize, value: ValueId) -> Result<(), TypeError> {
        let fact = &self.refs.values[value.0 as usize];
        let depth = self.nodes[index].key.depth;
        for &name in &fact.types {
            if let Some(projected) = self.project(name, depth) {
                push(&mut self.nodes[index].seed.names, projected)?;
            } else {
                push(
                    &mut self.nodes[index].seed.gaps,
                    ComponentGap::UnprojectableDeclaration(name),
                )?;
            }
        }
        if fact.type_unknown {
            push(
                &mut self.nodes[index].seed.gaps,
                ComponentGap::UnknownSource,
            )?;
        }
        if fact.types.is_empty() && !fact.type_unknown {
            if self.refs.is_null(self.nodes[index].key.state) {
                self.nodes[index].seed.null = true;
            } else {
                push(
                    &mut self.nodes[index].seed.gaps,
                    ComponentGap::NoDeclaration,
                )?;
            }
        }
        Ok(())
    }
    fn build(&mut self) -> Result<(), TypeError> {
        let mut index = 0;
        while index < self.nodes.len() {
            let Query {
                state,
                point,
                depth,
            } = self.nodes[index].key;
            if !self.refs.available(state) {
                push(
                    &mut self.nodes[index].seed.gaps,
                    ComponentGap::SourceUnavailable(state),
                )?;
            }
            let Some(value) = self.refs.value(state) else {
                index += 1;
                continue;
            };
            let source = self.refs.source(state)?;
            if self.refs.definition(state) == Some(point) && matches!(source, Source::Leaf) {
                if let Some((array, execute)) = self.components[value.0 as usize] {
                    let next = depth.checked_add(1).ok_or(RequestError::PlatformCapacity)?;
                    let source = self.query(array, execute, next)?;
                    self.link(index, source, None)?;
                } else {
                    self.seed(index, value)?;
                }
                index += 1;
                continue;
            }
            for i in 0..self.incoming[point.0 as usize].len() {
                let transfer = self.incoming[point.0 as usize][i];
                let from = self.refs.ssa.facts().transfers()[transfer.0 as usize].from();
                let state = self.refs.rewrite(state, point, transfer)?;
                let refinement = if self.normal(transfer) {
                    if let Some(event) = self.events[from.0 as usize] {
                        let cast = &self.work.casts[event];
                        if let Some(name) = self.project(cast.target, depth) {
                            let alias = alias_query(
                                &self.refs,
                                &self.incoming,
                                &mut self.aliases,
                                state,
                                cast.receiver,
                                from,
                            )?;
                            Some((alias, name))
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                } else {
                    None
                };
                let source = self.query(state, from, depth)?;
                self.link(index, source, refinement)?;
            }
            index += 1;
        }
        aliases(&self.refs, &self.incoming, &mut self.aliases)
    }
    fn solve(&mut self) -> Result<(), TypeError> {
        let mut queue = Vec::new();
        let mut queued = filled(self.nodes.len(), true)?;
        for i in 0..self.nodes.len() {
            push(&mut queue, i)?;
        }
        loop {
            while let Some(index) = queue.pop() {
                queued[index] = false;
                let mut result = Bounds::default();
                result.merge(&self.nodes[index].seed, None)?;
                for link in &self.nodes[index].links {
                    let refinement = link.refinement.and_then(|(alias, name)| {
                        (self.aliases.nodes[alias].bits == SAME).then_some(name)
                    });
                    result.merge(&self.nodes[link.source].result, refinement)?;
                }
                if !self.nodes[index].result.merge(&result, None)? {
                    continue;
                }
                for i in 0..self.nodes[index].users.len() {
                    let user = self.nodes[index].users[i];
                    if !queued[user] {
                        queued[user] = true;
                        push(&mut queue, user)?;
                    }
                }
            }
            let mut added = false;
            for (index, node) in self.nodes.iter_mut().enumerate() {
                if node.result.is_bottom() {
                    push(&mut node.seed.gaps, ComponentGap::UnknownSource)?;
                    queued[index] = true;
                    push(&mut queue, index)?;
                    added = true;
                }
            }
            if !added {
                return Ok(());
            }
        }
    }
}

pub(super) fn construct(
    ssa: &SsaMethod<'_>,
    f: &mut TypeFacts,
    work: &Work,
) -> Result<(), TypeError> {
    if work.arrays.is_empty() {
        return Ok(());
    }
    let mut max_rank = 0usize;
    for name in &f.names {
        max_rank = max_rank.max(
            name.units
                .iter()
                .take_while(|&&u| u == u16::from(b'['))
                .count(),
        );
    }
    let mut components = filled(ssa.facts().values().len(), None)?;
    for array in &work.arrays {
        if let Some(output) = array.output {
            let operation = &ssa.execution().operations()[array.operation.0 as usize];
            if matches!(
                operation.kind(),
                OpKind::Array {
                    access: ArrayAccess::Get,
                    kind: ValueKind::Object
                }
            ) {
                let execute = ssa
                    .facts()
                    .operation_site(array.operation)
                    .ok_or(TypeError::InternalInvariant(
                        "array producer lost its execution point",
                    ))?
                    .execute();
                components[output.0 as usize] = Some((array.array, execute));
            }
        }
    }
    let mut incoming = lists(ssa.facts().points().len())?;
    for transfer in ssa.facts().transfers() {
        if let Target::Point(target) = transfer.target() {
            push(&mut incoming[target.0 as usize], transfer.id())?;
        }
    }
    let mut events = filled(ssa.facts().points().len(), None)?;
    for (i, cast) in work.casts.iter().enumerate() {
        let execute = ssa
            .facts()
            .operation_site(cast.operation)
            .ok_or(TypeError::InternalInvariant(
                "cast bound lost its execution point",
            ))?
            .execute();
        events[execute.0 as usize] = Some(i);
    }
    let mut solver = Solver {
        refs: References::new(ssa, &f.values, &f.names, work)?,
        work,
        depth_limit: id(max_rank
            .checked_add(1)
            .ok_or(RequestError::PlatformCapacity)?)?,
        components,
        incoming,
        events,
        aliases: Aliases::new(),
        index: HashMap::new(),
        nodes: Vec::new(),
    };
    let mut queries = Vec::new();
    for array in &work.arrays {
        let execute = ssa
            .facts()
            .operation_site(array.operation)
            .ok_or(TypeError::InternalInvariant(
                "array access lost its execution point",
            ))?
            .execute();
        let query = solver.query(array.array, execute, 1)?;
        push(&mut queries, query)?;
    }
    solver.build()?;
    solver.solve()?;
    let mut knowledge = Vec::new();
    for query in queries {
        push(&mut knowledge, solver.nodes[query].result.knowledge()?)?;
    }
    drop(solver);
    for (array, bounds) in work.arrays.iter().zip(knowledge) {
        let mut component = ComponentKnowledge {
            types: Vec::new(),
            gaps: bounds.gaps,
        };
        // Materialize only suffixes demanded by published access facts, not
        // every dimension of every unrelated declaration in the method.
        for projection in bounds.names {
            let mut name = projection.name;
            for _ in 0..projection.depth {
                name = super::components::declaration(f, name)?.ok_or(
                    TypeError::InternalInvariant(
                        "component projection lost its checked descriptor",
                    ),
                )?;
            }
            if !component.types.contains(&name) {
                push(&mut component.types, name)?;
            }
        }
        if let Some(output) = array.output {
            let value = &mut f.values[output.0 as usize];
            for &name in &component.types {
                if !value.types.contains(&name) {
                    push(&mut value.types, name)?;
                }
            }
            value.type_unknown |= !component.gaps.is_empty();
        }
        push(
            &mut f.array_accesses,
            ArrayAccessView {
                operation: array.operation,
                array: array.array,
                component,
            },
        )?;
    }
    Ok(())
}
