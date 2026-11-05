use super::TypeError;
use super::build::{filled, push};
use crate::RequestError;
use std::collections::HashMap;
use std::hash::Hash;

pub(super) struct Link<E> {
    pub source: usize,
    pub effect: E,
}
pub(super) struct Node<K, E> {
    pub key: K,
    pub seed: u32,
    pub bits: u32,
    links: Vec<Link<E>>,
    users: Vec<usize>,
}
pub(super) struct Dataflow<K, E> {
    index: HashMap<K, usize>,
    pub nodes: Vec<Node<K, E>>,
}
impl<K: Copy + Eq + Hash, E: Copy> Dataflow<K, E> {
    pub fn new() -> Self {
        Self {
            index: HashMap::new(),
            nodes: Vec::new(),
        }
    }
    pub fn intern(&mut self, key: K) -> Result<usize, TypeError> {
        if let Some(&index) = self.index.get(&key) {
            return Ok(index);
        }
        let index = self.nodes.len();
        self.index
            .try_reserve(1)
            .map_err(RequestError::Allocation)?;
        push(
            &mut self.nodes,
            Node {
                key,
                seed: 0,
                bits: 0,
                links: Vec::new(),
                users: Vec::new(),
            },
        )?;
        self.index.insert(key, index);
        Ok(index)
    }
    pub fn link(&mut self, target: usize, source: usize, effect: E) -> Result<(), TypeError> {
        push(&mut self.nodes[target].links, Link { source, effect })?;
        push(&mut self.nodes[source].users, target)
    }
    pub fn solve(
        &mut self,
        ungrounded: impl Fn(K) -> u32,
        apply: impl Fn(E, u32) -> u32,
    ) -> Result<(), TypeError> {
        let mut queue = Vec::new();
        let mut queued = filled(self.nodes.len(), false)?;
        for (i, pending) in queued.iter_mut().enumerate() {
            push(&mut queue, i)?;
            *pending = true;
        }
        loop {
            while let Some(index) = queue.pop() {
                queued[index] = false;
                let node = &self.nodes[index];
                let mut bits = node.seed;
                for link in &node.links {
                    let source = self.nodes[link.source].bits;
                    if source != 0 {
                        bits |= apply(link.effect, source);
                    }
                }
                if bits == self.nodes[index].bits {
                    continue;
                }
                self.nodes[index].bits = bits;
                for i in 0..self.nodes[index].users.len() {
                    let user = self.nodes[index].users[i];
                    if !queued[user] {
                        push(&mut queue, user)?;
                        queued[user] = true;
                    }
                }
            }
            // Bottom is absence of evidence, not an initialized value or a
            // vacuously proven object relation in an ungrounded source cycle.
            let mut added = false;
            for (index, node) in self.nodes.iter_mut().enumerate() {
                if node.bits == 0 {
                    node.seed |= ungrounded(node.key);
                    push(&mut queue, index)?;
                    queued[index] = true;
                    added = true;
                }
            }
            if !added {
                return Ok(());
            }
        }
    }
}
