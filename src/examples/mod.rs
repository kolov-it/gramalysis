//! Example sentences for a verb: sentences in, the ones where a parse reads a
//! word as that verb in the asked-for cells out, each with where the word is.
//!
//! - [`table`]: the verb tables (`data/<lang>/verbs.json`) and cell names.
//! - [`align`]: tokens back to their place in the original text.
//!
//! The forms come from the verb's table; a cheap string match on the tokens
//! (clitics attached allowed) picks the sentences worth parsing; the parse
//! decides. Every parse is kept: a word some parse reads otherwise is
//! reported `ambiguous`, never resolved.
//!
//! Compound tenses (passato prossimo, pretérito perfecto, passé composé) are
//! not looked for: the tables have no cells for them.

pub mod align;
pub mod table;

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;

use crate::analysis::{self, TokenChoice, analyze};
use crate::lang::Language;
use crate::morph::Lexicon;
use crate::syntax::lattice::Morpheme;
use crate::verbs::verb_use;
use align::{AlignError, token_spans};
use table::{Target, cell_of, page_verb};

pub use table::{Query, QueryError, VerbTables};

pub type BoxError = Box<dyn std::error::Error + Send + Sync>;

#[derive(Debug, thiserror::Error)]
pub enum ExampleError {
    #[error(transparent)]
    Analysis(#[from] analysis::Error),
    #[error(transparent)]
    Align(#[from] AlignError),
    #[error("reading sentences: {0}")]
    Input(BoxError),
    #[error("writing examples: {0}")]
    Output(BoxError),
}

/// A sentence from a corpus.
#[derive(Debug, Clone)]
pub struct Sentence {
    /// Where it comes from: `tatoeba`, `leipzig`, a file name.
    pub source: String,
    pub id: u64,
    /// Tatoeba's contributor, for the CC BY attribution.
    pub author: Option<String>,
    pub text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Status {
    /// Every parse reads the word as this verb, in the same cell.
    Decided,
    /// Some parse reads it as this verb in an asked-for cell, and some parse
    /// reads it otherwise (another word, another cell, an adjective use).
    Ambiguous,
}

/// One word of one sentence read as the verb.
///
/// Where the word is, two ways: `before` + `form` + `after` is exactly
/// `text`, whatever encoding the reader uses; `char_start`..`char_end`
/// counts Unicode scalar values (Rust `chars()`, Python `str` indices; not
/// UTF-16 code units, which differ outside the Basic Multilingual Plane).
#[derive(Debug, Clone, Serialize)]
pub struct Example {
    pub source: String,
    pub id: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    pub text: String,
    pub infinitive: String,
    /// The lemma the parse gave (mettersi for the infinitive mettere).
    pub lemma: String,
    /// The asked-for cells the parses read the word as: one when decided.
    pub cells: Vec<String>,
    pub status: Status,
    /// How many parses the sentence has.
    pub parses: u64,
    /// The token's index in the tokenizer's output.
    pub token: usize,
    /// The word as written in `text`, clitics included (farlo, Dámelo).
    pub form: String,
    pub before: String,
    pub after: String,
    pub char_start: usize,
    pub char_end: usize,
    /// For a verb with attached clitics: the verb part as the parse split it
    /// (far in farlo, da in dammelo, Dá in Dámelo) and the clitics.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verb: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub clitics: Vec<String>,
    /// The parses' other readings of the word, when ambiguous.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub other_readings: Vec<String>,
}

/// What one sentence gave.
#[derive(Debug)]
pub enum Outcome {
    /// No word may be one of the forms; the sentence was not parsed.
    NotCandidate,
    /// These words are unknown to the lexicon, so the sentence cannot parse.
    UnknownWord(Vec<String>),
    /// The grammar does not accept the sentence.
    NoParse,
    /// The words read as the verb in an asked-for cell (possibly none).
    Parsed(Vec<Example>),
}

#[derive(Debug, Clone, Default)]
pub struct Options {
    /// Stop after this many examples are emitted.
    pub limit: Option<usize>,
    /// Emit ambiguous examples too.
    pub include_ambiguous: bool,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct Stats {
    /// Sentences read.
    pub sentences: u64,
    /// Sentences with a word that may be one of the forms, and so parsed.
    pub candidates: u64,
    /// Candidates with a word the lexicon does not know.
    pub unknown_word: u64,
    /// Candidates the grammar does not accept.
    pub no_parse: u64,
    /// Words found, by status (emitted or not).
    pub decided: u64,
    pub ambiguous: u64,
    pub emitted: u64,
}

pub struct Extractor<'a> {
    lexicon: &'a dyn Lexicon,
    language: &'a dyn Language,
    target: Target,
}

impl<'a> Extractor<'a> {
    pub fn new(
        lexicon: &'a dyn Lexicon,
        language: &'a dyn Language,
        tables: &VerbTables,
        query: &Query,
    ) -> Result<Self, QueryError> {
        let target = Target::new(tables, language.lang(), query)?;
        Ok(Self {
            lexicon,
            language,
            target,
        })
    }

    /// The cells asked for, each with its forms.
    pub fn cells(&self) -> &BTreeMap<String, Vec<String>> {
        &self.target.cells
    }

    /// The examples in one sentence: one per word read as the verb.
    pub async fn examples(&self, sentence: &Sentence) -> Result<Outcome, ExampleError> {
        let tokens = self.language.tokenize(&sentence.text);
        let candidates: Vec<usize> = (0..tokens.len())
            .filter(|&t| self.target.may_be(&tokens[t].to_lowercase()))
            .collect();
        if candidates.is_empty() {
            return Ok(Outcome::NotCandidate);
        }
        let a = analyze(self.lexicon, self.language, &sentence.text).await?;
        if !a.unknown.is_empty() {
            return Ok(Outcome::UnknownWord(a.unknown));
        }
        if a.parse_count == 0 {
            return Ok(Outcome::NoParse);
        }
        let spans = token_spans(&sentence.text, &a.tokens)?;
        let text = &sentence.text;
        let mut out = Vec::new();
        for t in candidates {
            let Some(found) = self.classify(&a.chosen[t]) else {
                continue;
            };
            let span = spans[t].clone();
            let status = if found.others.is_empty() && found.cells.len() == 1 {
                Status::Decided
            } else {
                Status::Ambiguous
            };
            let (verb, clitics) = match found.reading.split_first() {
                Some((host, rest)) if !rest.is_empty() => (
                    Some(host.surface.clone()),
                    rest.iter().map(|m| m.surface.clone()).collect(),
                ),
                _ => (None, Vec::new()),
            };
            out.push(Example {
                source: sentence.source.clone(),
                id: sentence.id,
                author: sentence.author.clone(),
                text: text.clone(),
                infinitive: self.target.infinitive.clone(),
                lemma: found.reading[0].lemma.clone(),
                cells: found.cells.into_iter().collect(),
                status,
                parses: a.parse_count,
                token: t,
                form: text[span.clone()].to_owned(),
                before: text[..span.start].to_owned(),
                after: text[span.end..].to_owned(),
                char_start: text[..span.start].chars().count(),
                char_end: text[..span.end].chars().count(),
                verb,
                clitics,
                other_readings: found.others.into_iter().collect(),
            });
        }
        Ok(Outcome::Parsed(out))
    }

    /// Runs over `sentences` and hands each example to `emit`, until the
    /// sentences end or `options.limit` examples were emitted.
    pub async fn run<I, F>(
        &self,
        sentences: I,
        options: &Options,
        mut emit: F,
    ) -> Result<Stats, ExampleError>
    where
        I: IntoIterator<Item = Result<Sentence, BoxError>>,
        F: FnMut(&Example) -> Result<(), BoxError>,
    {
        let mut stats = Stats::default();
        for sentence in sentences {
            if options.limit.is_some_and(|l| stats.emitted >= l as u64) {
                break;
            }
            let sentence = sentence.map_err(ExampleError::Input)?;
            stats.sentences += 1;
            let examples = match self.examples(&sentence).await? {
                Outcome::NotCandidate => continue,
                Outcome::UnknownWord(_) => {
                    stats.candidates += 1;
                    stats.unknown_word += 1;
                    continue;
                }
                Outcome::NoParse => {
                    stats.candidates += 1;
                    stats.no_parse += 1;
                    continue;
                }
                Outcome::Parsed(examples) => examples,
            };
            stats.candidates += 1;
            for e in &examples {
                match e.status {
                    Status::Decided => stats.decided += 1,
                    Status::Ambiguous => stats.ambiguous += 1,
                }
                if options.limit.is_some_and(|l| stats.emitted >= l as u64) {
                    continue;
                }
                if e.status == Status::Decided || options.include_ambiguous {
                    emit(e).map_err(ExampleError::Output)?;
                    stats.emitted += 1;
                }
            }
        }
        Ok(stats)
    }

    /// The token's readings, sorted into the asked-for verb cells and the
    /// rest. `None` when no reading is the verb in an asked-for cell.
    fn classify<'c>(&self, choice: &'c TokenChoice) -> Option<Found<'c>> {
        let lang = self.target.lang;
        let mut cells = BTreeSet::new();
        let mut others = BTreeSet::new();
        let mut reading = None;
        for (r, uses) in choice.readings.iter().zip(&choice.uses) {
            let Some(m) = r.first() else { continue };
            let used = verb_use(m, uses);
            let cell = (m.cat == "Verb").then(|| cell_of(&m.feats, lang)).flatten();
            if used.as_verb {
                match &cell {
                    Some(c)
                        if page_verb(&m.lemma, lang) == self.target.base
                            && self.target.written_as(
                                c,
                                &m.surface.to_lowercase(),
                                r.len() > 1,
                            ) =>
                    {
                        cells.insert(c.clone());
                        reading.get_or_insert(r);
                    }
                    _ => {
                        others.insert(describe(m, cell.as_deref()));
                    }
                }
            }
            if used.as_adjective {
                others.insert(format!(
                    "Adj {} (the participle used as an adjective)",
                    m.surface.to_lowercase()
                ));
            }
        }
        Some(Found {
            cells,
            others,
            reading: reading?,
        })
    }
}

struct Found<'c> {
    cells: BTreeSet<String>,
    others: BTreeSet<String>,
    /// The first reading that is the verb in an asked-for cell.
    reading: &'c [Morpheme],
}

/// `Verb fare indicative.present.3s`, `Noun porta`.
fn describe(m: &Morpheme, cell: Option<&str>) -> String {
    match cell {
        Some(cell) => format!("{} {} {cell}", m.cat, m.lemma),
        None => format!("{} {}", m.cat, m.lemma),
    }
}
