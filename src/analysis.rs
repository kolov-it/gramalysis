//! Sentence in, parses out: the pipeline that ties the parts together.

use serde::Serialize;

use crate::lang::Language;
use crate::morph::{Lexicon, LexiconError};
use crate::syntax::lattice::{Lattice, Morpheme};
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
    pub parses: Vec<Tree>,
    /// Per token, the readings used by at least one parse.
    pub chosen: Vec<TokenChoice>,
}

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
    let parses = parser::parse(language.grammar(), &lattice)?;
    let chosen = choices(&tokens, &parses);
    Ok(SentenceAnalysis { tokens, unknown, lattice, parses, chosen })
}

fn choices(tokens: &[String], parses: &[Tree]) -> Vec<TokenChoice> {
    let mut out: Vec<TokenChoice> = tokens
        .iter()
        .map(|t| TokenChoice { token: t.clone(), readings: Vec::new(), agreed: !parses.is_empty() })
        .collect();
    for parse in parses {
        let mut per_token: Vec<Vec<Morpheme>> = vec![Vec::new(); tokens.len()];
        for (token, morph) in parse.leaves() {
            per_token[token].push(morph.clone());
        }
        for (t, reading) in per_token.into_iter().enumerate() {
            let key = |r: &[Morpheme]| r.iter().map(|m| (m.lemma.clone(), m.cat.clone(), m.feats.clone())).collect::<Vec<_>>();
            if !out[t].readings.iter().any(|r| key(r) == key(&reading)) {
                out[t].readings.push(reading);
            }
        }
    }
    for c in &mut out {
        c.agreed = c.agreed && c.readings.len() == 1;
    }
    out
}
