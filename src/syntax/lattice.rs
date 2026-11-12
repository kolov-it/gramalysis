//! The sentence as a graph of morphemes. Nodes are positions; every edge is
//! one possible morpheme between two positions. A token with several readings
//! has several parallel edges, and a token that splits (`dammelo` = da + me +
//! lo) has a path through nodes of its own. Node ids are in topological order.

use serde::Serialize;

use crate::syntax::feats::FeatMap;

#[derive(Debug, Clone, Serialize)]
pub struct Morpheme {
    pub surface: String,
    pub lemma: String,
    /// Terminal category in the grammar: Verb, Noun, Det, Pron, …
    pub cat: String,
    pub feats: FeatMap,
}

#[derive(Debug, Clone, Serialize)]
pub struct Edge {
    pub from: usize,
    pub to: usize,
    /// Index of the token this morpheme belongs to.
    pub token: usize,
    pub morph: Morpheme,
}

#[derive(Debug, Default, Serialize)]
pub struct Lattice {
    pub tokens: Vec<String>,
    pub nodes: usize,
    pub edges: Vec<Edge>,
}

impl Lattice {
    pub fn new() -> Self {
        Self { tokens: Vec::new(), nodes: 1, edges: Vec::new() }
    }

    pub fn last_node(&self) -> usize {
        self.nodes - 1
    }

    /// Adds a token after the current end. Each reading is a sequence of one
    /// or more morphemes.
    pub fn push_token(&mut self, surface: &str, readings: Vec<Vec<Morpheme>>) {
        let token = self.tokens.len();
        self.tokens.push(surface.to_owned());
        let start = self.last_node();
        // Inner nodes are allocated before the token's end node, so ids stay
        // in topological order.
        let mut paths: Vec<(Vec<usize>, Vec<Morpheme>)> = Vec::new();
        for morphs in readings {
            let inner: Vec<usize> = (1..morphs.len()).map(|_| self.alloc()).collect();
            paths.push((inner, morphs));
        }
        let end = self.alloc();
        for (inner, morphs) in paths {
            let mut from = start;
            for (i, morph) in morphs.into_iter().enumerate() {
                let to = inner.get(i).copied().unwrap_or(end);
                self.edges.push(Edge { from, to, token, morph });
                from = to;
            }
        }
    }

    fn alloc(&mut self) -> usize {
        self.nodes += 1;
        self.nodes - 1
    }
}
