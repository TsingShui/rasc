//! Normal-control LT forests. Reverse LT supplies finite-exit follow candidates;
//! original links, including divergent paths, remain in every region check.
use super::build::*;
use super::graph::Graph;
use super::*;
use crate::p2::Terminal;
use crate::p5::StorageMethod;

pub(super) struct Dominance {
    pub parent: Vec<Option<usize>>,
    depth: Vec<usize>,
    before: Vec<usize>,
    after: Vec<usize>,
}
fn eval(
    v: usize,
    ancestor: &mut [usize],
    label: &mut [usize],
    semi: &[usize],
    path: &mut Vec<usize>,
) -> Result<usize, RegionError> {
    if ancestor[v] == 0 {
        return Ok(label[v]);
    }
    let mut x = v;
    path.clear();
    while ancestor[x] != 0 && ancestor[ancestor[x]] != 0 {
        push(path, x)?;
        x = ancestor[x];
    }
    while let Some(x) = path.pop() {
        let a = ancestor[x];
        if semi[label[a]] < semi[label[x]] {
            label[x] = label[a];
        }
        ancestor[x] = ancestor[a];
    }
    Ok(label[v])
}
impl Dominance {
    fn construct(out: &[Vec<usize>], root: usize) -> Result<Self, RegionError> {
        let n = out.len();
        let size = n
            .checked_add(1)
            .ok_or(crate::RequestError::PlatformCapacity)?;
        let mut incoming = lists(n)?;
        for (from, row) in out.iter().enumerate() {
            for &to in row {
                push(&mut incoming[to], from)?;
            }
        }
        let mut number = filled(n, 0usize)?;
        let mut vertex = filled(size, 0usize)?;
        let mut parent = filled(size, 0usize)?;
        let mut count = 1;
        number[root] = 1;
        vertex[1] = root;
        let mut stack = Vec::new();
        push(&mut stack, (root, 0usize))?;
        while let Some(&(node, cursor)) = stack.last() {
            if cursor == out[node].len() {
                stack.pop();
                continue;
            }
            stack.last_mut().ok_or(bug("P6 DFS lost its frame"))?.1 += 1;
            let next = out[node][cursor];
            if number[next] == 0 {
                count += 1;
                number[next] = count;
                vertex[count] = next;
                parent[count] = number[node];
                push(&mut stack, (next, 0))?;
            }
        }
        let mut semi = filled(size, 0usize)?;
        let mut label = filled(size, 0usize)?;
        for i in 1..=count {
            semi[i] = i;
            label[i] = i;
        }
        let mut ancestor = filled(size, 0usize)?;
        let mut immediate = filled(size, 0usize)?;
        let mut heads = filled(size, 0usize)?;
        let mut links = filled(size, 0usize)?;
        let mut path = Vec::new();
        for w in (2..=count).rev() {
            for &p in &incoming[vertex[w]] {
                let p = number[p];
                if p == 0 {
                    continue;
                }
                let u = eval(p, &mut ancestor, &mut label, &semi, &mut path)?;
                semi[w] = semi[w].min(semi[u]);
            }
            links[w] = heads[semi[w]];
            heads[semi[w]] = w;
            ancestor[w] = parent[w];
            let mut v = heads[parent[w]];
            heads[parent[w]] = 0;
            while v != 0 {
                let next = links[v];
                let u = eval(v, &mut ancestor, &mut label, &semi, &mut path)?;
                immediate[v] = if semi[u] < semi[v] { u } else { parent[w] };
                v = next;
            }
        }
        for w in 2..=count {
            if immediate[w] != semi[w] {
                immediate[w] = immediate[immediate[w]];
            }
            if immediate[w] == 0 {
                return Err(bug("P6 LT did not establish a parent"));
            }
        }
        let mut parents = filled(n, None)?;
        let mut children = lists(n)?;
        for w in 2..=count {
            let p = vertex[immediate[w]];
            parents[vertex[w]] = Some(p);
            push(&mut children[p], vertex[w])?;
        }
        let mut depth = filled(n, 0usize)?;
        let mut before = filled(n, usize::MAX)?;
        let mut after = filled(n, usize::MAX)?;
        stack.clear();
        push(&mut stack, (root, 0))?;
        let mut clock = 0;
        before[root] = clock;
        clock += 1;
        while let Some(&(node, cursor)) = stack.last() {
            if cursor == children[node].len() {
                after[node] = clock;
                stack.pop();
                continue;
            }
            stack
                .last_mut()
                .ok_or(bug("P6 dominator walk lost its frame"))?
                .1 += 1;
            let q = children[node][cursor];
            depth[q] = depth[node]
                .checked_add(1)
                .ok_or(crate::RequestError::PlatformCapacity)?;
            before[q] = clock;
            clock += 1;
            push(&mut stack, (q, 0))?;
        }
        Ok(Self {
            parent: parents,
            depth,
            before,
            after,
        })
    }
    pub fn dominates(&self, a: usize, b: usize) -> bool {
        self.before[a] != usize::MAX
            && self.before[b] != usize::MAX
            && self.before[a] <= self.before[b]
            && self.before[b] < self.after[a]
    }
    pub fn common(&self, mut a: usize, mut b: usize) -> Option<usize> {
        if self.before[a] == usize::MAX || self.before[b] == usize::MAX {
            return None;
        }
        while self.depth[a] > self.depth[b] {
            a = self.parent[a]?;
        }
        while self.depth[b] > self.depth[a] {
            b = self.parent[b]?;
        }
        while a != b {
            a = self.parent[a]?;
            b = self.parent[b]?;
        }
        Some(a)
    }
}
pub(super) struct Structure {
    pub dom: Dominance,
    pub post: Dominance,
    pub component: Vec<usize>,
    pub cyclic: Vec<bool>,
    pub virtual_node: usize,
}
/// Chooses the vertex each node contributes to post-dominance. A void `return`
/// prints the same statement wherever it stands, so every path that ends in one
/// ends in the same place: one shared `return;` block can serve as the follow of
/// the branches that reach it instead of making their arms overlap at it. A
/// return that carries a value keeps its own vertex, because that value is
/// original register state this stage does not compare, and a `throw` keeps its
/// own vertex for the same reason.
fn exit_vertices(storage: &StorageMethod<'_>, f: &RegionFacts) -> Result<Vec<usize>, RegionError> {
    let mut vertices = Vec::new();
    for node in 0..f.nodes.len() {
        push(&mut vertices, node)?;
    }
    let mut shared = None;
    for (index, node) in f.nodes.iter().enumerate() {
        if !node.terminal || !node.operations.is_empty() {
            continue;
        }
        let terminal = storage.execution().blocks()[node.block.0 as usize].terminal();
        if !matches!(terminal, Some(Terminal::Return { width: None, .. })) {
            continue;
        }
        match shared {
            None => shared = Some(index),
            Some(first) => vertices[index] = first,
        }
    }
    Ok(vertices)
}
impl Structure {
    pub fn construct(
        storage: &StorageMethod<'_>,
        f: &RegionFacts,
        g: &Graph,
    ) -> Result<Self, RegionError> {
        let n = f.nodes.len();
        let size = n
            .checked_add(1)
            .ok_or(crate::RequestError::PlatformCapacity)?;
        let exits = exit_vertices(storage, f)?;
        let mut out = lists(size)?;
        let mut reverse = lists(size)?;
        for l in &f.links {
            push(&mut out[l.from.0 as usize], l.to.0 as usize)?;
            push(
                &mut reverse[exits[l.to.0 as usize]],
                exits[l.from.0 as usize],
            )?;
        }
        for &root in &g.roots {
            push(&mut out[n], root.0 as usize)?;
        }
        for (node, &exit) in exits.iter().enumerate() {
            if g.outgoing[node].is_empty() {
                push(&mut reverse[n], exit)?;
            }
        }
        let dom = Dominance::construct(&out, n)?;
        let post = Dominance::construct(&reverse, n)?;
        let mut seen = filled(n, false)?;
        let mut order = Vec::new();
        let mut stack = Vec::new();
        for seed in 0..n {
            if seen[seed] {
                continue;
            }
            seen[seed] = true;
            push(&mut stack, (seed, 0usize))?;
            while let Some(&(p, cursor)) = stack.last() {
                if cursor == g.outgoing[p].len() {
                    push(&mut order, p)?;
                    stack.pop();
                    continue;
                }
                stack
                    .last_mut()
                    .ok_or(bug("P6 SCC traversal lost frame"))?
                    .1 += 1;
                let q = f.links[g.outgoing[p][cursor].0 as usize].to.0 as usize;
                if !seen[q] {
                    seen[q] = true;
                    push(&mut stack, (q, 0))?;
                }
            }
        }
        let mut component = filled(n, usize::MAX)?;
        let mut sizes = Vec::new();
        let mut queue = Vec::new();
        while let Some(seed) = order.pop() {
            if component[seed] != usize::MAX {
                continue;
            }
            let id = sizes.len();
            let mut count = 0;
            component[seed] = id;
            push(&mut queue, seed)?;
            while let Some(p) = queue.pop() {
                count += 1;
                for &l in &g.incoming[p] {
                    let q = f.links[l.0 as usize].from.0 as usize;
                    if component[q] == usize::MAX {
                        component[q] = id;
                        push(&mut queue, q)?;
                    }
                }
            }
            push(&mut sizes, count)?;
        }
        let mut cyclic = filled(sizes.len(), false)?;
        for (i, &size) in sizes.iter().enumerate() {
            cyclic[i] = size > 1;
        }
        for l in &f.links {
            if l.from == l.to {
                cyclic[component[l.from.0 as usize]] = true;
            }
        }
        Ok(Self {
            dom,
            post,
            component,
            cyclic,
            virtual_node: n,
        })
    }
    pub fn follow(&self, node: NodeId) -> Option<NodeId> {
        self.post.parent[node.0 as usize]
            .filter(|&p| p != self.virtual_node)
            .and_then(|p| u32::try_from(p).ok())
            .map(NodeId)
    }
    pub fn common_follow(&self, nodes: &[NodeId]) -> Option<NodeId> {
        let mut result = nodes.first()?.0 as usize;
        for n in &nodes[1..] {
            result = self.post.common(result, n.0 as usize)?;
        }
        if result == self.virtual_node {
            None
        } else {
            u32::try_from(result).ok().map(NodeId)
        }
    }
}
