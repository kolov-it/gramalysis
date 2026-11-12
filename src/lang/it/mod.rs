//! Italian: tokenizer, function words, clitics, and the grammar in
//! `data/it/grammar.gram`.

mod clitics;
mod closed;
mod sound;
mod tokenize;

pub use clitics::split_enclitics;
pub use sound::onset;
pub use tokenize::tokenize;

use crate::lang::Language;
use crate::morph::{Analysis, Lang, PartOfSpeech, Reading};
use crate::syntax::feats::{self, FeatMap};
use crate::syntax::grammar::{Grammar, GrammarError};
use crate::syntax::lattice::{Lattice, Morpheme};
use closed::{with_surface, FunctionWords};
use tokenize::is_punct;

const CLOSED: &str = include_str!("../../../data/it/closed.txt");
const GRAMMAR: &str = include_str!("../../../data/it/grammar.gram");

/// Categories the function-word list owns: the lexicon's readings in these
/// categories are replaced by the list's, its other readings (`la` the
/// musical note, `dei` the gods) are kept.
const OWNED_CATS: [&str; 5] = ["Det", "Pron", "Prep", "Conj", "Contraction"];

#[derive(Debug, thiserror::Error)]
pub enum DataError {
    #[error("data/it/closed.txt line {line}: {message}")]
    Closed { line: usize, message: String },
    #[error("data/it/grammar.gram: {0}")]
    Grammar(#[from] GrammarError),
}

pub struct Italian {
    closed: FunctionWords,
    grammar: Grammar,
}

impl Language for Italian {
    fn lang(&self) -> Lang {
        Lang::It
    }

    fn grammar(&self) -> &Grammar {
        &self.grammar
    }

    fn tokenize(&self, text: &str) -> Vec<String> {
        tokenize(text)
    }

    fn lexicon_forms(&self, tokens: &[String]) -> Vec<String> {
        tokens.iter().filter(|t| !is_punct(t)).map(|t| t.to_lowercase()).collect()
    }

    fn lattice(&self, tokens: &[String], analyses: &[Analysis]) -> Lattice {
        self.build_lattice(tokens, analyses)
    }
}

impl Italian {
    pub fn load() -> Result<Self, DataError> {
        Ok(Self { closed: closed::parse(CLOSED)?, grammar: Grammar::parse(GRAMMAR)? })
    }

    fn build_lattice(&self, tokens: &[String], analyses: &[Analysis]) -> Lattice {
        let mut lattice = Lattice::new();
        let mut words = analyses.iter();
        for (i, token) in tokens.iter().enumerate() {
            if is_punct(token) {
                let morph = Morpheme { surface: token.clone(), lemma: token.clone(), cat: "Punct".to_owned(), feats: FeatMap::new() };
                lattice.push_token(token, vec![vec![morph]]);
                continue;
            }
            let lower = token.to_lowercase();
            let analysis = words.next();
            let mut readings: Vec<Vec<Morpheme>> = Vec::new();
            let closed = self.closed.get(&lower);
            if let Some(closed) = closed {
                readings.extend(closed.iter().map(|r| with_surface(r, token)));
            }
            for reading in analysis.map(|a| a.readings.as_slice()).unwrap_or_default() {
                let cat = feats::category(&reading.pos);
                if closed.is_some() && OWNED_CATS.contains(&cat.as_str()) {
                    continue;
                }
                // The list already has this word in this category (`non` Adv).
                let listed = closed.into_iter().flatten().any(|r| r.len() == 1 && r[0].cat == cat && r[0].lemma == reading.lemma);
                if listed {
                    continue;
                }
                readings.extend(self.morphemes(&lower, token, reading));
            }
            // Words from the lexicon carry the sound they start with, for the
            // article before them to check (il cane, lo zio, l'amico).
            for reading in &mut readings {
                if let Some(first) = reading.first_mut()
                    && !OWNED_CATS.contains(&first.cat.as_str())
                {
                    first.feats.insert("onset".to_owned(), onset(&lower).to_owned());
                }
            }
            let capitalised = token.chars().next().is_some_and(char::is_uppercase);
            if capitalised && (i > 0 || readings.is_empty()) {
                let mut feats = FeatMap::new();
                feats.insert("onset".to_owned(), onset(&lower).to_owned());
                readings.push(vec![Morpheme { surface: token.clone(), lemma: token.clone(), cat: "ProperNoun".to_owned(), feats }]);
            }
            lattice.push_token(token, readings);
        }
        lattice
    }

    /// One lexicon reading as morpheme sequences: one sequence, or several
    /// when the reading carries clitics and they can be read in several ways.
    fn morphemes(&self, lower: &str, surface: &str, reading: &Reading) -> Vec<Vec<Morpheme>> {
        let mut host_feats = feats::from_features(&reading.features);
        complete_features(lower, reading, &mut host_feats);
        let host = |s: &str, feats: FeatMap| Morpheme {
            surface: s.to_owned(),
            lemma: reading.lemma.clone(),
            cat: feats::category(&reading.pos),
            feats,
        };
        if !reading.clitic || reading.pos != PartOfSpeech::Verb {
            return vec![vec![host(surface, host_feats)]];
        }
        let Some((host_surface, clitics)) = split_enclitics(lower) else {
            return vec![vec![host(surface, host_feats)]];
        };
        let mut sequences = vec![vec![host(&host_surface, host_feats)]];
        let last = clitics.len() - 1;
        for (i, clitic) in clitics.iter().enumerate() {
            let combining = i < last;
            let options: Vec<Morpheme> = self
                .closed
                .get(clitic)
                .into_iter()
                .flatten()
                .filter(|r| r.len() == 1)
                .map(|r| r[0].clone())
                .filter(|m| m.feats.get("clitic").map(String::as_str) == Some("yes"))
                .filter(|m| m.feats.contains_key("combining") == combining)
                .map(|m| Morpheme { surface: clitic.clone(), ..m })
                .collect();
            sequences = sequences
                .into_iter()
                .flat_map(|seq| {
                    options.iter().map(move |m| {
                        let mut s = seq.clone();
                        s.push(m.clone());
                        s
                    })
                })
                .collect();
        }
        sequences
    }
}

/// Fills in what Italian grammar implies but the lexicon leaves out
/// (doc/lexicon-api-needs.md lists each gap; remove a rule here once the API
/// answers it).
fn complete_features(lower: &str, reading: &Reading, feats: &mut FeatMap) {
    if reading.pos == PartOfSpeech::Verb {
        // `arrivata` comes tagged "participle"; Italian's inflected
        // participles are past participles.
        if feats.get("form").map(String::as_str) == Some("part") {
            feats.insert("form".to_owned(), "pastpart".to_owned());
        }
        // The citation participle (`arrivato`, `fatto`) is masculine singular.
        if feats.get("form").map(String::as_str) == Some("pastpart") && !feats.contains_key("number") && !feats.contains_key("gender") {
            feats.insert("number".to_owned(), "sg".to_owned());
            feats.insert("gender".to_owned(), "m".to_owned());
        }
        // The passato remoto (`fece`, tagged "historic past") has a person but
        // no mood: it is indicative.
        if feats.contains_key("person") && !feats.contains_key("mood") && !feats.contains_key("form") {
            feats.insert("mood".to_owned(), "ind".to_owned());
            feats.insert("finite".to_owned(), "yes".to_owned());
        }
    }
    complete_lemma_form(lower, reading, feats);
}

/// The lexicon gives no grammar for a form that is the dictionary form
/// itself. In Italian that form is known: a verb's is the infinitive, a
/// noun's the singular, an adjective's the masculine singular.
fn complete_lemma_form(lower: &str, reading: &Reading, feats: &mut FeatMap) {
    if lower != reading.lemma || !feats.is_empty() {
        return;
    }
    let mut put = |k: &str, v: &str| {
        feats.insert(k.to_owned(), v.to_owned());
    };
    match reading.pos {
        PartOfSpeech::Verb => {
            put("form", "inf");
            put("finite", "no");
        }
        PartOfSpeech::Noun => put("number", "sg"),
        PartOfSpeech::Adjective => {
            put("number", "sg");
            put("gender", "m");
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn function_word_list_loads() {
        let it = Italian::load().unwrap();
        assert_eq!(it.closed["del"].len(), 1);
        assert_eq!(it.closed["del"][0].len(), 2);
        assert!(it.closed["la"].len() >= 2);
    }
}
