//! Sentence in, parses out: the pipeline that ties the parts together.

use std::collections::{HashMap, HashSet};

use serde::Serialize;

use crate::lang::Language;
use crate::morph::{Lexicon, LexiconError};
use crate::syntax::lattice::{Edge, Lattice, Morpheme};
use crate::syntax::parser::{self, ChartItem, ParseError, Tree};

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
    /// Lattice edges any parse uses.
    #[serde(skip)]
    pub used_edges: HashSet<usize>,
    /// For each used edge, the rules that consumed it.
    #[serde(skip)]
    pub edge_rules: HashMap<usize, HashSet<String>>,
    /// The chart items of compound tenses and passives: the rules that pass
    /// an auxiliary's features up (`aux_mood`, `aux_tense`, …) or mark a
    /// passive (`voice`).
    #[serde(skip)]
    pub verb_items: Vec<ChartItem>,
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
    /// For each reading in `readings` (same order), the grammar rules that
    /// used it across all parses: how the word functions in that reading
    /// (`ap.participle`: a participle used like an adjective;
    /// `v.perfect-essere`: in a compound tense).
    pub uses: Vec<Vec<String>>,
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
    let used_edges = forest.used_edges();
    let edge_rules = forest.edge_rules();
    let chosen = choices(&tokens, &lattice, &used_edges, &edge_rules, !forest.is_empty());
    let verb_items = forest.items_where(|r| r.lhs.constraints.iter().any(|(k, _)| k.starts_with("aux_") || k == "voice"));
    Ok(SentenceAnalysis { tokens, unknown, lattice, parse_count, parses, chosen, used_edges, edge_rules, verb_items })
}

/// Per token, the readings (paths of morphemes through the token) that the
/// parses use. Built from the edges any parse uses, so it covers every parse,
/// not only the ones kept as trees.
fn choices(
    tokens: &[String],
    lattice: &Lattice,
    used: &HashSet<usize>,
    rules: &HashMap<usize, HashSet<String>>,
    parsed: bool,
) -> Vec<TokenChoice> {
    (0..tokens.len())
        .map(|t| {
            let edges: Vec<(usize, &Edge)> =
                lattice.edges.iter().enumerate().filter(|(e, edge)| edge.token == t && used.contains(e)).collect();
            let starts: HashSet<usize> = edges.iter().map(|(_, e)| e.from).collect();
            let ends: HashSet<usize> = edges.iter().map(|(_, e)| e.to).collect();
            let mut readings: Vec<(usize, Vec<Morpheme>)> = Vec::new();
            for start in starts.difference(&ends) {
                walk(&edges, *start, None, &mut Vec::new(), &mut readings);
            }
            let key = |r: &[Morpheme]| r.iter().map(|m| (m.lemma.clone(), m.cat.clone(), m.feats.clone())).collect::<Vec<_>>();
            let mut unique: Vec<Vec<Morpheme>> = Vec::new();
            let mut uses: Vec<Vec<String>> = Vec::new();
            for (first_edge, r) in readings {
                let i = match unique.iter().position(|u| key(u) == key(&r)) {
                    Some(i) => i,
                    None => {
                        unique.push(r);
                        uses.push(Vec::new());
                        unique.len() - 1
                    }
                };
                uses[i].extend(rules.get(&first_edge).into_iter().flatten().cloned());
                uses[i].sort();
                uses[i].dedup();
            }
            TokenChoice { token: tokens[t].clone(), agreed: parsed && unique.len() == 1, readings: unique, uses }
        })
        .collect()
}

/// Every path of morphemes through a token, with the lattice edge of its
/// first morpheme (the one the grammar's rules consumed as the word).
fn walk(
    edges: &[(usize, &Edge)],
    node: usize,
    first: Option<usize>,
    path: &mut Vec<Morpheme>,
    out: &mut Vec<(usize, Vec<Morpheme>)>,
) {
    let next: Vec<&(usize, &Edge)> = edges.iter().filter(|(_, e)| e.from == node).collect();
    if next.is_empty() {
        if let Some(first) = first {
            out.push((first, path.clone()));
        }
        return;
    }
    for (i, e) in next {
        path.push(e.morph.clone());
        walk(edges, e.to, first.or(Some(*i)), path, out);
        path.pop();
    }
}
