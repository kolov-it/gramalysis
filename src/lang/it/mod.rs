//! Italian: tokenizer, function words, clitics, and the grammar in
//! `data/it/grammar.gram`.

mod clitics;
mod closed;
mod sound;
mod tokenize;

pub use clitics::split_enclitics;
pub use sound::onset;
pub use tokenize::tokenize;

use std::collections::HashSet;

use crate::lang::Language;
use crate::morph::{Analysis, FormKind, Lang, Number, PartOfSpeech, Reading};
use crate::syntax::feats::{self, FeatMap};
use crate::syntax::grammar::{Grammar, GrammarError};
use crate::syntax::lattice::{Lattice, Morpheme};
use closed::{with_surface, FunctionWords};
use tokenize::is_punct;

const CLOSED: &str = include_str!("../../../data/it/closed.txt");
const GRAMMAR: &str = include_str!("../../../data/it/grammar.gram");
const DRIFTED: &str = include_str!("../../../data/it/drifted-participles.txt");
const DI_VERBS: &str = include_str!("../../../data/it/di-verbs.tsv");

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
    /// Participle adjectives whose meaning drifted from the verb (esatto).
    drifted: HashSet<String>,
    /// Verbs that take a di + noun phrase complement (parlare di).
    di_verbs: HashSet<String>,

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
        Ok(Self {
            closed: closed::parse(CLOSED)?,
            drifted: word_list(DRIFTED),
            di_verbs: first_column(DI_VERBS),
            grammar: Grammar::parse(GRAMMAR)?,
        })
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
                readings.extend(closed.iter().map(|r| with_surface(r, token)).map(|mut r| {
                    // A clitic written as its own word stands before the verb.
                    for m in &mut r {
                        if m.feats.get("clitic").map(String::as_str) == Some("yes") {
                            m.feats.insert("attached".to_owned(), "no".to_owned());
                        }
                    }
                    r
                }));
            }
            for reading in analysis.map(|a| a.readings.as_slice()).unwrap_or_default() {
                let cat = feats::category(&reading.pos);
                if closed.is_some() && OWNED_CATS.contains(&cat.as_str()) {
                    continue;
                }
                // The list already has this word (`non` Adv, `questo` as an
                // adjective, `mio` as a determiner): the list's readings win.
                let listed = closed.into_iter().flatten().any(|r| r.iter().any(|m| m.lemma == reading.lemma));
                if listed {
                    continue;
                }
                let reading = present_participle_features(reading.clone(), &lower);
                readings.extend(self.morphemes(&lower, token, &reading));
            }
            merge_participle_adjectives(&mut readings, &self.drifted);
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
        // The auxiliary the verb takes in compound tenses, when the lexicon
        // says: the grammar tells a compound tense (sono salite) from a state
        // (sono accese) by it. Unknown leaves both open.
        if reading.pos == PartOfSpeech::Verb && !reading.features.auxiliaries.is_empty() {
            for aux in ["avere", "essere"] {
                let takes = reading.features.auxiliaries.iter().any(|a| a == aux);
                host_feats.insert(format!("takes_{aux}"), if takes { "yes" } else { "no" }.to_owned());
            }
        }
        if reading.pos == PartOfSpeech::Verb {
            let takes = self.di_verbs.contains(&non_pronominal(&reading.lemma));
            host_feats.insert("takes_di".to_owned(), if takes { "yes" } else { "no" }.to_owned());
        }
        let lemma = reading.lemma.clone();
        let host = |s: &str, feats: FeatMap| Morpheme {
            surface: s.to_owned(),
            lemma: lemma.clone(),
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
                .map(|mut m| {
                    m.feats.insert("attached".to_owned(), "yes".to_owned());
                    Morpheme { surface: clitic.clone(), ..m }
                })
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

/// A past participle is one form of its verb, whether the sentence uses it
/// as a verb (ha scritto) or like an adjective (un giornale scritto, le luci
/// accese); the grammar's `ap.participle` rule covers the second use. So the
/// dictionary's separate adjective entry for the same form (scritto, acceso)
/// is dropped: it would only pose as a second answer. Two exceptions keep
/// the adjective as its own word:
/// - an adjective whose meaning drifted from the verb (`drifted`: esatto is
///   "exact", not "demanded"); its participle gets `drifted=yes`, which
///   `ap.participle` refuses;
/// - a present participle (pesante, corrente): in Italian it is almost always
///   an adjective, and a verb only with the verb's complements
///   (`partp.present`: il fiume passante per Parigi). The participle itself
///   can be used like an adjective too (`ap.present-participle`), which
///   covers the forms without an adjective entry (il treno fuggente).
///
/// Adjectives are marked `participle=<verb>` when the token is also that
/// verb's participle, `participle=no` otherwise, so `ap.complement` can leave
/// a participle with complements to the participle phrase.
fn merge_participle_adjectives(readings: &mut Vec<Vec<Morpheme>>, drifted: &HashSet<String>) {
    let participle = |r: &Vec<Morpheme>, kind: &str| match r.as_slice() {
        [m] if m.cat == "Verb" && m.feats.get("form").map(String::as_str) == Some(kind) => Some(m.lemma.clone()),
        _ => None,
    };
    let past = readings.iter().find_map(|r| participle(r, "pastpart"));
    let present = readings.iter().find_map(|r| participle(r, "prespart"));
    let surface = readings.iter().find_map(|r| r.first()).map(|m| m.surface.to_lowercase()).unwrap_or_default();
    let is_drifted = past.is_some() && masculine_singular(&surface).is_some_and(|ms| drifted.contains(&ms));
    if past.is_some() && !is_drifted {
        readings.retain(|r| !matches!(r.as_slice(), [m] if m.cat == "Adj"));
    }
    for r in readings.iter_mut() {
        let [m] = r.as_mut_slice() else { continue };
        if m.cat == "Adj" {
            let of = if is_drifted { None } else { present.clone() };
            m.feats.insert("participle".to_owned(), of.unwrap_or_else(|| "no".to_owned()));
        } else if is_drifted && m.cat == "Verb" && m.feats.get("form").map(String::as_str) == Some("pastpart") {
            m.feats.insert("drifted".to_owned(), "yes".to_owned());
        }
    }
}

/// Present participles and the adjectives made from them (tremante,
/// sorridente) have one form for both genders, -nte in the singular and -nti
/// in the plural. The lexicon leaves that out in two ways, corrected here:
/// - a present participle comes without a number (fuggente), so it would
///   agree with a plural noun; the ending gives it;
/// - the plural of such an adjective can come back masculine only (tremanti:
///   masculine plural), so it would refuse a feminine noun (le foglie
///   tremanti); its gender is left unknown.
fn present_participle_features(mut reading: Reading, form: &str) -> Reading {
    let number = if form.ends_with("nte") {
        Number::Singular
    } else if form.ends_with("nti") {
        Number::Plural
    } else {
        return reading;
    };
    match reading.pos {
        PartOfSpeech::Verb if reading.features.form_kind == Some(FormKind::PresentParticiple) => {
            reading.features.number.get_or_insert(number);
        }
        PartOfSpeech::Adjective
            if number == Number::Plural
                && reading.lemma.strip_suffix('e') == form.strip_suffix('i') =>
        {
            reading.features.gender = None;
        }
        _ => {}
    }
    reading
}

/// A past participle's masculine singular: participles inflect like
/// -o/-a/-i/-e adjectives (accesa, accesi, accese → acceso).
fn masculine_singular(form: &str) -> Option<String> {
    let last = form.chars().last()?;
    matches!(last, 'o' | 'a' | 'i' | 'e').then(|| format!("{}o", &form[..form.len() - last.len_utf8()]))
}

fn word_list(text: &str) -> HashSet<String> {
    text.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')).map(str::to_owned).collect()
}

fn first_column(text: &str) -> HashSet<String> {
    word_list(text).iter().filter_map(|l| l.split('\t').next()).map(str::to_owned).collect()
}

/// The infinitive without its pronouns, as `di-verbs.tsv` lists it: the
/// lexicon gives pronominal verbs both ways (accorgersi, ricordare).
fn non_pronominal(lemma: &str) -> String {
    for suffix in ["sene", "si", "ne"] {
        if let Some(stem) = lemma.strip_suffix(suffix)
            && stem.ends_with('r')
        {
            let ending = if stem.ends_with("or") || stem.ends_with("ur") { "re" } else { "e" };
            return format!("{stem}{ending}");
        }
    }
    lemma.to_owned()
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
        assert!(it.di_verbs.contains("parlare") && !it.di_verbs.contains("portare"));
    }

    #[test]
    fn pronominal_verbs_lose_their_pronouns() {
        assert_eq!(non_pronominal("accorgersi"), "accorgere");
        assert_eq!(non_pronominal("fregarsene"), "fregare");
        assert_eq!(non_pronominal("disporsi"), "disporre");
        assert_eq!(non_pronominal("parlare"), "parlare");
    }
}
