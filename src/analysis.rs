//! Sentence in, parses out: the pipeline that ties the parts together.

use std::collections::HashSet;

use serde::Serialize;

use crate::lang::Language;
use crate::morph::{Lexicon, LexiconError};
use crate::syntax::lattice::{Edge, Lattice, Morpheme};
use crate::syntax::parser::{self, ParseError, Tree};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Lexicon(#[from] LexiconError),
    #[error(transparent)]
    Parse(#[from] ParseError),
}

#[derive(Debug, Serialize)]
pub struct SentenceAnalysis {
    pub tokens: Vec<String>,
    /// Tokens no reading was found for. A sentence with one cannot parse.
    pub unknown: Vec<String>,
    pub lattice: Lattice,
    /// How many parses there are (saturating).
    pub parse_count: u64,
    /// The first parses, at most [`MAX_TREES`].
    pub parses: Vec<Tree>,
    /// Per token, the readings used by at least one parse.
    pub chosen: Vec<TokenChoice>,
}

/// Parses kept as trees; the rest are counted, and still count for `chosen`.
pub const MAX_TREES: usize = 20;

#[derive(Debug, Serialize)]
pub struct TokenChoice {
    pub token: String,
    /// Each distinct reading (as its morphemes) any parse used.
    pub readings: Vec<Vec<Morpheme>>,
    /// Every parse read this token the same way.
    pub agreed: bool,
}

pub async fn analyze(lexicon: &dyn Lexicon, language: &dyn Language, text: &str) -> Result<SentenceAnalysis, Error> {
    let tokens = language.tokenize(text);
    let forms = language.lexicon_forms(&tokens);
    let form_refs: Vec<&str> = forms.iter().map(String::as_str).collect();
    let analyses = lexicon.analyze(language.lang(), &form_refs).await?;
    let lattice = language.lattice(&tokens, &analyses);
    let unknown = (0..tokens.len())
        .filter(|t| !lattice.edges.iter().any(|e| e.token == *t))
        .map(|t| tokens[t].clone())
        .collect();
    let forest = parser::parse(language.grammar(), &lattice)?;
    let parse_count = forest.count();
    let parses = forest.trees(&lattice, MAX_TREES);
    let chosen = choices(&tokens, &lattice, &forest.used_edges(), !forest.is_empty());
    Ok(SentenceAnalysis { tokens, unknown, lattice, parse_count, parses, chosen })
}

/// Per token, the readings (paths of morphemes through the token) that the
/// parses use. Built from the edges any parse uses, so it covers every parse,
/// not only the ones kept as trees.
fn choices(tokens: &[String], lattice: &Lattice, used: &HashSet<usize>, parsed: bool) -> Vec<TokenChoice> {
    (0..tokens.len())
        .map(|t| {
            let edges: Vec<&Edge> = lattice.edges.iter().enumerate().filter(|(e, edge)| edge.token == t && used.contains(e)).map(|(_, edge)| edge).collect();
            let starts: HashSet<usize> = edges.iter().map(|e| e.from).collect();
            let ends: HashSet<usize> = edges.iter().map(|e| e.to).collect();
            let first_nodes: Vec<usize> = starts.difference(&ends).copied().collect();
            let mut readings: Vec<Vec<Morpheme>> = Vec::new();
            for start in first_nodes {
                walk(&edges, start, &mut Vec::new(), &mut readings);
            }
            let key = |r: &[Morpheme]| r.iter().map(|m| (m.lemma.clone(), m.cat.clone(), m.feats.clone())).collect::<Vec<_>>();
            let mut unique: Vec<Vec<Morpheme>> = Vec::new();
            for r in readings {
                if !unique.iter().any(|u| key(u) == key(&r)) {
                    unique.push(r);
                }
            }
            TokenChoice { token: tokens[t].clone(), agreed: parsed && unique.len() == 1, readings: unique }
        })
        .collect()
}

fn walk(edges: &[&Edge], node: usize, path: &mut Vec<Morpheme>, out: &mut Vec<Vec<Morpheme>>) {
    let next: Vec<&&Edge> = edges.iter().filter(|e| e.from == node).collect();
    if next.is_empty() {
        if !path.is_empty() {
            out.push(path.clone());
        }
        return;
    }
    for e in next {
        path.push(e.morph.clone());
        walk(edges, e.to, path, out);
        path.pop();
    }
}
