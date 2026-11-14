//! Example sentences for a verb: sentences in, the ones where a parse reads a
//! word as that verb in the asked-for cells out, each with where the verb is.
//!
//! - [`tenses`]: the tenses of each language and their identifiers.
//! - [`table`]: the verb tables (`data/<lang>/verbs.json`) and cell names.
//! - [`align`]: tokens back to their place in the original text.
//!
//! A simple tense is one word; a compound tense (ho mangiato) or a passive
//! (fu chiesto, è stato venduto) is an auxiliary and the past participle, read
//! off the parse's verb groups ([`crate::syntax::parser::Forest::participle_groups`]):
//! the auxiliary's tense names the compound tense. A cheap string match picks
//! the sentences worth parsing (a form of the verb, clitics attached allowed;
//! or its participle in any gender and number with an auxiliary somewhere);
//! the parse decides. Every parse is kept: a word some parse reads otherwise
//! is reported `ambiguous`, never resolved.

pub mod align;
pub mod table;
pub mod tenses;

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use serde::Serialize;

use crate::analysis::{self, SentenceAnalysis, analyze};
use crate::lang::Language;
use crate::morph::Lexicon;
use crate::syntax::feats::FeatMap;
use crate::syntax::grammar::{Cat, Value};
use crate::syntax::lattice::{Lattice, Morpheme};
use crate::syntax::parser::{ChartItem, ItemChild};
use crate::verbs::verb_use;
use align::{AlignError, token_spans};
use table::{Target, cell_of, page_verb};
use tenses::{auxiliaries, compound_of, tense_of};

pub use table::{Query, QueryError, VerbTables, check_tense};

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Status {
    /// Every parse reads the word as this verb, in the same cell and voice,
    /// with the same auxiliaries.
    Decided,
    /// Some parse reads it as this verb in an asked-for cell, and some parse
    /// reads it otherwise (another word, another cell, an adjective use).
    Ambiguous,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Voice {
    Active,
    /// The participle with essere/venire, ser, être: fu chiesto, è stato
    /// venduto, fue construida, a été vendu.
    Passive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    /// The verb of a simple tense.
    Verb,
    /// An auxiliary of a compound tense or a passive (ho, è, è stato).
    Auxiliary,
    /// The past participle of a compound tense or a passive.
    Participle,
}

/// One highlighted word. `char_start`..`char_end` counts Unicode scalar
/// values (Rust `chars()`, Python `str` indices; not UTF-16 code units, which
/// differ outside the Basic Multilingual Plane).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Part {
    pub role: Role,
    /// The token's index in the tokenizer's output.
    pub token: usize,
    /// The word as written, clitics included (farlo, Dámelo, averlo).
    pub text: String,
    pub char_start: usize,
    pub char_end: usize,
    /// For a word with attached clitics: the verb part as the parse split it
    /// (far in farlo, da in dammelo, Dá in Dámelo) and the clitics.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verb: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub clitics: Vec<String>,
}

/// A piece of the sentence: the segments of an example concatenate to its
/// text, and the highlighted ones are its parts, in order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Segment {
    pub text: String,
    pub highlight: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<Role>,
}

/// The verb in one sentence, in an asked-for cell.
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
    /// The asked-for cells the parses read it as: one when decided.
    pub cells: Vec<String>,
    pub status: Status,
    pub voice: Voice,
    /// How many parses the sentence has.
    pub parses: u64,
    /// The verb's words: one for a simple tense; the auxiliaries and the
    /// participle for a compound tense or a passive (a word between them, an
    /// adverb or a clitic, is not a part: ho già mangiato).
    pub parts: Vec<Part>,
    pub segments: Vec<Segment>,
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
    /// The verb in an asked-for cell, once per word (possibly none).
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
    /// Examples found, by status (emitted or not).
    pub decided: u64,
    pub ambiguous: u64,
    pub emitted: u64,
}

/// A compound tense or a passive in a parse: an auxiliary (+ stacked
/// auxiliaries) + participle(s), from one chart item.
struct VerbGroup {
    /// The rules that consumed its words directly.
    rules: Vec<String>,
    /// The auxiliary that carries the tense (ho, è, fue, a).
    aux: usize,
    /// Auxiliaries in participle form before the main participle (stato,
    /// sido, été).
    layers: Vec<usize>,
    participles: Vec<usize>,
    voice: Voice,
    /// The cell the group is in, when the auxiliary's cell names one.
    cell: Option<String>,
    aux_cell: Option<String>,
}

/// A verb item's words in rule order, through the phrases that pass the
/// auxiliary up (Spanish `SerV`, `PassP`).
#[derive(Debug, Clone)]
enum Slot {
    Aux(Vec<usize>, String),
    Participle(Vec<usize>, String),
    Coordinator,
}

/// One way the parses read a candidate word.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum Interpretation {
    /// The verb in an asked-for cell. The parts are (edge, role).
    Match {
        cell: String,
        voice: Voice,
        lemma: String,
        parts: Vec<(usize, Role)>,
    },
    Other(String),
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

    /// The cells asked for; a simple one with its forms.
    pub fn cells(&self) -> &BTreeMap<String, Vec<String>> {
        &self.target.cells
    }

    /// The examples in one sentence: one per word read as the verb (the
    /// participle, for a compound tense).
    pub async fn examples(&self, sentence: &Sentence) -> Result<Outcome, ExampleError> {
        let lower: Vec<String> = self
            .language
            .tokenize(&sentence.text)
            .iter()
            .map(|t| t.to_lowercase())
            .collect();
        let auxiliary = lower.iter().any(|t| self.target.may_be_auxiliary(t));
        let candidates: Vec<usize> = (0..lower.len())
            .filter(|&t| {
                self.target.may_be(&lower[t])
                    || (auxiliary && self.target.may_be_participle(&lower[t]))
            })
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
        let all: Vec<VerbGroup> = self.verb_groups(&a);
        let groups: Vec<&VerbGroup> = all.iter().collect();
        let text = &sentence.text;
        let chars = |byte: usize| text[..byte].chars().count();
        let mut out = Vec::new();
        for t in candidates {
            let readings = self.interpret(&a, &groups, t, &lower[t]);
            let matches: Vec<&Interpretation> = readings
                .iter()
                .filter(|r| matches!(r, Interpretation::Match { .. }))
                .collect();
            let Some(Interpretation::Match {
                voice,
                lemma,
                parts,
                ..
            }) = matches.first().copied()
            else {
                continue;
            };
            let key = |r: &Interpretation| match r {
                Interpretation::Match {
                    cell, voice, parts, ..
                } => Some((
                    cell.clone(),
                    *voice,
                    parts
                        .iter()
                        .map(|(e, role)| (a.lattice.edges[*e].token, *role))
                        .collect::<Vec<_>>(),
                )),
                Interpretation::Other(_) => None,
            };
            let distinct: BTreeSet<_> = readings.iter().map(key).collect();
            let status = if distinct.len() == 1 {
                Status::Decided
            } else {
                Status::Ambiguous
            };
            let cells: BTreeSet<String> = matches
                .iter()
                .filter_map(|r| match r {
                    Interpretation::Match { cell, .. } => Some(cell.clone()),
                    Interpretation::Other(_) => None,
                })
                .collect();
            let others: BTreeSet<String> = readings
                .iter()
                .filter_map(|r| match r {
                    Interpretation::Other(d) => Some(d.clone()),
                    Interpretation::Match { .. } => None,
                })
                .collect();
            let parts: Vec<Part> = parts
                .iter()
                .map(|&(e, role)| {
                    let token = a.lattice.edges[e].token;
                    let span = spans[token].clone();
                    let (verb, clitics) = split_word(&a.lattice, e);
                    Part {
                        role,
                        token,
                        text: text[span.clone()].to_owned(),
                        char_start: chars(span.start),
                        char_end: chars(span.end),
                        verb,
                        clitics,
                    }
                })
                .collect();
            let mut segments = Vec::new();
            let mut at = 0;
            for p in &parts {
                let span = spans[p.token].clone();
                if span.start > at {
                    segments.push(Segment {
                        text: text[at..span.start].to_owned(),
                        highlight: false,
                        role: None,
                    });
                }
                segments.push(Segment {
                    text: p.text.clone(),
                    highlight: true,
                    role: Some(p.role),
                });
                at = span.end;
            }
            if at < text.len() {
                segments.push(Segment {
                    text: text[at..].to_owned(),
                    highlight: false,
                    role: None,
                });
            }
            out.push(Example {
                source: sentence.source.clone(),
                id: sentence.id,
                author: sentence.author.clone(),
                text: text.clone(),
                infinitive: self.target.infinitive.clone(),
                lemma: lemma.clone(),
                cells: cells.into_iter().collect(),
                status,
                voice: *voice,
                parses: a.parse_count,
                parts,
                segments,
                other_readings: others.into_iter().collect(),
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

    /// The compound tenses and passives the parses build. A construction
    /// whose only use is to pass its auxiliary up to another (Spanish ha
    /// sido in ha sido construido) is part of that one, not a verb of its own.
    fn verb_groups(&self, a: &SentenceAnalysis) -> Vec<VerbGroup> {
        let grammar = self.language.grammar();
        let items: HashMap<usize, &ChartItem> = a.verb_items.iter().map(|i| (i.id, i)).collect();
        let mut out = Vec::new();
        for item in &a.verb_items {
            let only_passed_up = !item.used_as.is_empty()
                && item
                    .used_as
                    .iter()
                    .all(|&(rule, at)| passes_up(&grammar.rules[rule].rhs[at]));
            if only_passed_up {
                continue;
            }
            for slots in self.slots(&items, item) {
                out.extend(self.verb_group(&a.lattice, item, &slots));
            }
        }
        out
    }

    /// An item's words in rule order, one list per combination of the
    /// phrases it passes the auxiliary up through (usually one).
    fn slots(&self, items: &HashMap<usize, &ChartItem>, item: &ChartItem) -> Vec<Vec<Slot>> {
        let rule = &self.language.grammar().rules[item.rule];
        let aux_vars: HashSet<&str> = rule
            .lhs
            .constraints
            .iter()
            .filter(|(k, _)| k.starts_with("aux_"))
            .filter_map(|(_, v)| match v {
                Value::Var(v) => Some(v.as_str()),
                _ => None,
            })
            .collect();
        let mut out: Vec<Vec<Slot>> = vec![Vec::new()];
        for (at, cat) in rule.rhs.iter().enumerate() {
            let edges: Vec<usize> = item.children[at]
                .iter()
                .filter_map(|p| match p {
                    ItemChild::Edge(e) => Some(*e),
                    ItemChild::Item(_) => None,
                })
                .collect();
            let slot = if cat.name == "Verb" && is_participle_slot(cat) {
                Some(Slot::Participle(edges, rule.name.clone()))
            } else if cat.name == "Verb"
                && cat
                    .constraints
                    .iter()
                    .any(|(_, v)| matches!(v, Value::Var(v) if aux_vars.contains(v.as_str())))
            {
                Some(Slot::Aux(edges, rule.name.clone()))
            } else if cat.name == "Conj" {
                Some(Slot::Coordinator)
            } else {
                None
            };
            if let Some(slot) = slot {
                for list in &mut out {
                    list.push(slot.clone());
                }
                continue;
            }
            if !passes_up(cat) {
                continue;
            }
            let inner: Vec<Vec<Slot>> = item.children[at]
                .iter()
                .filter_map(|p| match p {
                    ItemChild::Item(c) => items.get(c),
                    ItemChild::Edge(_) => None,
                })
                .flat_map(|child| self.slots(items, child))
                .collect();
            out = out
                .iter()
                .flat_map(|l| {
                    inner
                        .iter()
                        .map(move |i| l.iter().chain(i).cloned().collect())
                })
                .collect();
        }
        out
    }

    /// A verb item's slots as auxiliary + participle(s), or `None` when it
    /// has no participle of its own (ha sido alone, before its participle).
    fn verb_group(&self, lattice: &Lattice, item: &ChartItem, slots: &[Slot]) -> Option<VerbGroup> {
        let lang = self.target.lang;
        // The governing auxiliary is the last before the participles.
        let last_participle = slots
            .iter()
            .rposition(|s| matches!(s, Slot::Participle(..)))?;
        let aux_at = slots[..last_participle]
            .iter()
            .rposition(|s| matches!(s, Slot::Aux(..)))?;
        let Slot::Aux(aux_edges, aux_rule) = &slots[aux_at] else {
            return None;
        };
        let aux = *aux_edges.first()?;
        let mut rules = vec![aux_rule.clone()];
        let (mut layers, mut participles) = (Vec::new(), Vec::new());
        // A participle that another follows without a coordinator between is
        // a stacked auxiliary (stato in è stato venduto); across one, a verb
        // of its own (sono venuto e partito).
        let rest = &slots[aux_at + 1..];
        for (i, slot) in rest.iter().enumerate() {
            let Slot::Participle(edges, rule) = slot else {
                continue;
            };
            rules.push(rule.clone());
            let stacked = rest[i + 1..]
                .iter()
                .find(|s| !matches!(s, Slot::Aux(..)))
                .is_some_and(|s| matches!(s, Slot::Participle(..)));
            let is_aux =
                |e: &usize| auxiliaries(lang).contains(&lattice.edges[*e].morph.lemma.as_str());
            if stacked && edges.iter().all(is_aux) {
                layers.extend(edges);
            } else {
                participles.extend(edges);
            }
        }
        if participles.is_empty() {
            return None;
        }
        let voice = if item.feats.get("voice").map(String::as_str) == Some("passive") {
            Voice::Passive
        } else {
            Voice::Active
        };
        let compound = voice == Voice::Active || !layers.is_empty();
        let aux_feats: FeatMap = item
            .feats
            .iter()
            .filter_map(|(k, v)| k.strip_prefix("aux_").map(|k| (k.to_owned(), v.clone())))
            .collect();
        let aux_cell = cell_of(&aux_feats, lang);
        let cell = aux_cell.as_deref().and_then(|c| {
            if !compound {
                return Some(c.to_owned());
            }
            let tense = tense_of(c);
            compound_of(lang, tense).map(|t| format!("{}{}", t.id, &c[tense.len()..]))
        });
        rules.sort();
        rules.dedup();
        Some(VerbGroup {
            rules,
            aux,
            layers,
            participles,
            voice,
            cell,
            aux_cell,
        })
    }

    /// Every way the parses read token `t` (`lower` is its lower case).
    fn interpret(
        &self,
        a: &SentenceAnalysis,
        groups: &[&VerbGroup],
        t: usize,
        lower: &str,
    ) -> Vec<Interpretation> {
        let lang = self.target.lang;
        let edges: Vec<usize> = (0..a.lattice.edges.len())
            .filter(|e| a.lattice.edges[*e].token == t)
            .collect();
        let (Some(start), Some(end)) = (
            edges.iter().map(|&e| a.lattice.edges[e].from).min(),
            edges.iter().map(|&e| a.lattice.edges[e].to).max(),
        ) else {
            return Vec::new();
        };
        let mut out = BTreeSet::new();
        for &e in edges
            .iter()
            .filter(|&&e| a.lattice.edges[e].from == start && a.used_edges.contains(&e))
        {
            let m = &a.lattice.edges[e].morph;
            let surface = m.surface.to_lowercase();
            let this_verb = m.cat == "Verb" && page_verb(&m.lemma, lang) == self.target.base;
            let mut grouped: HashSet<&str> = HashSet::new();
            for g in groups.iter().filter(|g| g.participles.contains(&e)) {
                grouped.extend(g.rules.iter().map(String::as_str));
                match &g.cell {
                    Some(c) if this_verb && self.target.cells.contains_key(c) => {
                        let mut parts: Vec<(usize, Role)> =
                            std::iter::once((g.aux, Role::Auxiliary))
                                .chain(g.layers.iter().map(|&l| (l, Role::Auxiliary)))
                                .chain([(e, Role::Participle)])
                                .collect();
                        parts.sort_by_key(|(edge, _)| a.lattice.edges[*edge].token);
                        out.insert(Interpretation::Match {
                            cell: c.clone(),
                            voice: g.voice,
                            lemma: m.lemma.clone(),
                            parts,
                        });
                    }
                    _ => {
                        out.insert(Interpretation::Other(format!(
                            "Verb {} {}",
                            m.lemma,
                            group_name(g)
                        )));
                    }
                }
            }
            for g in groups
                .iter()
                .filter(|g| g.aux == e || g.layers.contains(&e))
            {
                grouped.extend(g.rules.iter().map(String::as_str));
                let of = &a.lattice.edges[g.participles[0]].morph.lemma;
                out.insert(Interpretation::Other(format!(
                    "auxiliary of {of} {}",
                    group_name(g)
                )));
            }
            let rest: Vec<String> = a
                .edge_rules
                .get(&e)
                .into_iter()
                .flatten()
                .filter(|r| !grouped.contains(r.as_str()))
                .cloned()
                .collect();
            if rest.is_empty() {
                continue;
            }
            let used = verb_use(m, &rest);
            let cell = (m.cat == "Verb").then(|| cell_of(&m.feats, lang)).flatten();
            if used.as_verb {
                let split = (a.lattice.edges[e].to != end).then_some(lower);
                match &cell {
                    Some(c) if this_verb && self.target.written_as(c, &surface, split) => {
                        out.insert(Interpretation::Match {
                            cell: c.clone(),
                            voice: Voice::Active,
                            lemma: m.lemma.clone(),
                            parts: vec![(e, Role::Verb)],
                        });
                    }
                    _ => {
                        out.insert(Interpretation::Other(describe(m, cell.as_deref())));
                    }
                }
            }
            if used.as_adjective {
                out.insert(Interpretation::Other(format!(
                    "Adj {surface} (the participle used as an adjective)"
                )));
            }
        }
        out.into_iter().collect()
    }
}

/// A category the rule passes an auxiliary's features or a voice through
/// (Spanish `SerV[aux_tense=?t]`, `PassP[voice=passive]`).
fn passes_up(cat: &Cat) -> bool {
    cat.constraints
        .iter()
        .any(|(k, _)| k.starts_with("aux_") || k == "voice")
}

/// A `Verb[form=pastpart]` position.
fn is_participle_slot(cat: &Cat) -> bool {
    cat.constraints.iter().any(|(k, v)| {
        k == "form" && matches!(v, Value::Any(a) if a.iter().any(|x| x == "pastpart"))
    })
}

/// The verb part and the clitics of a word whose reading starts at `edge`,
/// when clitics are attached to it.
fn split_word(lattice: &Lattice, edge: usize) -> (Option<String>, Vec<String>) {
    let token = lattice.edges[edge].token;
    let mut clitics = Vec::new();
    let mut node = lattice.edges[edge].to;
    while let Some(next) = lattice
        .edges
        .iter()
        .find(|n| n.token == token && n.from == node)
    {
        clitics.push(next.morph.surface.clone());
        node = next.to;
    }
    if clitics.is_empty() {
        return (None, clitics);
    }
    (Some(lattice.edges[edge].morph.surface.clone()), clitics)
}

/// `indicative.present_perfect.3s active`; `with the auxiliary in
/// imperative.present.1p` when the auxiliary names no tense of the table.
fn group_name(g: &VerbGroup) -> String {
    let voice = match g.voice {
        Voice::Active => "active",
        Voice::Passive => "passive",
    };
    match (&g.cell, &g.aux_cell) {
        (Some(cell), _) => format!("{cell} {voice}"),
        (None, Some(aux)) => format!("{voice}, with the auxiliary in {aux}"),
        (None, None) => format!("{voice}, with an auxiliary of no known cell"),
    }
}

/// `Verb fare indicative.present.3s`, `Noun porta`.
fn describe(m: &Morpheme, cell: Option<&str>) -> String {
    match cell {
        Some(cell) => format!("{} {} {cell}", m.cat, m.lemma),
        None => format!("{} {}", m.cat, m.lemma),
    }
}
