//! Lengauer–Tarjan with iterative DFS and iterative union/find compression.
use super::graph::{Csr, Graph};
use super::scratch::{bug, filled, id, push, reserve};
use super::{PointId, SsaError, SsaFacts};

pub(super) struct Dominance {
    #[cfg(test)]
    pub parent: Vec<usize>,
    pub depth: Vec<usize>,
    pub children: Csr,
}

fn eval(
    v: usize,
    ancestor: &mut [usize],
    label: &mut [usize],
    semi: &[usize],
    path: &mut Vec<usize>,
) -> Result<usize, SsaError> {
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

pub(super) fn construct(f: &mut SsaFacts, graph: &Graph) -> Result<Dominance, SsaError> {
    let n = f.points.len();
    if n == 0 {
        return Err(bug("empty SSA state graph"));
    }
    let size = n.checked_add(1).ok_or(SsaError::PlatformCapacity)?;
    let mut number = filled(n, 0usize)?;
    let mut vertex = filled(size, 0usize)?;
    let mut parent = filled(size, 0usize)?;
    number[0] = 1;
    let mut count = 1usize;
    let mut stack = Vec::new();
    reserve(&mut stack, n)?;
    push(&mut stack, (0usize, 0usize))?;
    while let Some(&(node, cursor)) = stack.last() {
        let row = graph.outgoing.row(node);
        if cursor == row.len() {
            stack.pop();
            continue;
        }
        stack.last_mut().ok_or(bug("DFS stack lost its frame"))?.1 += 1;
        let next = Graph::successor(f, row[cursor]);
        if number[next] == 0 {
            count += 1;
            number[next] = count;
            vertex[count] = next;
            parent[count] = number[node];
            push(&mut stack, (next, 0))?;
        }
    }
    if count != n {
        return Err(bug("private state graph has an unreachable position"));
    }
    let mut semi = filled(size, 0usize)?;
    let mut label = filled(size, 0usize)?;
    for i in 1..size {
        semi[i] = i;
        label[i] = i;
    }
    let mut ancestor = filled(size, 0usize)?;
    let mut immediate = filled(size, 0usize)?;
    let mut heads = filled(size, 0usize)?;
    let mut links = filled(size, 0usize)?;
    let mut path = Vec::new();
    reserve(&mut path, n)?;
    for w in (2..size).rev() {
        for &edge in graph.incoming.row(vertex[w]) {
            let pred = number[f.transfers[edge].from.0 as usize];
            let u = eval(pred, &mut ancestor, &mut label, &semi, &mut path)?;
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
    for w in 2..size {
        if immediate[w] != semi[w] {
            immediate[w] = immediate[immediate[w]];
        }
        if immediate[w] == 0 {
            return Err(bug("LT failed to establish an immediate dominator"));
        }
    }
    let mut point_parent = filled(n, 0usize)?;
    for w in 2..size {
        point_parent[vertex[w]] = vertex[immediate[w]];
    }
    let children = Csr::new(n, (1..n).map(|v| (point_parent[v], v)))?;
    let mut depth = filled(n, 0usize)?;
    stack.clear();
    push(&mut stack, (0, 0))?;
    let mut clock = 0usize;
    f.points[0].before = id(clock)?;
    clock += 1;
    while let Some(&(node, cursor)) = stack.last() {
        let row = children.row(node);
        if cursor == row.len() {
            f.points[node].after = id(clock)?;
            stack.pop();
            continue;
        }
        stack
            .last_mut()
            .ok_or(bug("dominator traversal lost its frame"))?
            .1 += 1;
        let next = row[cursor];
        depth[next] = depth[node]
            .checked_add(1)
            .ok_or(SsaError::PlatformCapacity)?;
        f.points[next].idom = Some(PointId(id(node)?));
        f.points[next].before = id(clock)?;
        clock += 1;
        push(&mut stack, (next, 0))?;
    }
    if clock != n {
        return Err(bug(
            "dominator tree does not cover the reachable state graph",
        ));
    }
    Ok(Dominance {
        #[cfg(test)]
        parent: point_parent,
        depth,
        children,
    })
}

impl Dominance {
    /// Check the predecessor-meet equations, independently of LT's semi/eval data.
    #[cfg(test)]
    pub fn verify(&self, f: &SsaFacts, graph: &Graph) -> Result<(), SsaError> {
        let n = self.parent.len();
        let levels = (usize::BITS - n.leading_zeros()) as usize;
        let mut up = Vec::new();
        reserve(&mut up, levels)?;
        let mut first = filled(n, 0usize)?;
        first.copy_from_slice(&self.parent);
        push(&mut up, first)?;
        for k in 1..levels {
            let mut row = filled(n, 0usize)?;
            for (v, value) in row.iter_mut().enumerate() {
                *value = up[k - 1][up[k - 1][v]];
            }
            push(&mut up, row)?;
        }
        let lca = |mut a: usize, mut b: usize| {
            if self.depth[a] < self.depth[b] {
                std::mem::swap(&mut a, &mut b);
            }
            let difference = self.depth[a] - self.depth[b];
            for (k, row) in up.iter().enumerate() {
                if difference & (1usize << k) != 0 {
                    a = row[a];
                }
            }
            if a == b {
                return a;
            }
            for row in up.iter().rev() {
                if row[a] != row[b] {
                    a = row[a];
                    b = row[b];
                }
            }
            self.parent[a]
        };
        for node in 1..n {
            let mut meet = None;
            for &edge in graph.incoming.row(node) {
                let p = f.transfers[edge].from.0 as usize;
                meet = Some(match meet {
                    Some(m) => lca(m, p),
                    None => p,
                });
            }
            if meet != Some(self.parent[node]) {
                return Err(bug("dominator predecessor meet does not match"));
            }
        }
        Ok(())
    }
}
