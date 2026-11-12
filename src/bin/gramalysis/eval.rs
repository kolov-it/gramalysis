//! `gramalysis eval`: runs a sentence set (data/it/homograph-set.jsonl) and
//! measures how the ambiguous word in each sentence was read.
//!
//! The reference for an item is its label (data/it/homograph-labels.tsv) when
//! it has one, otherwise spaCy's tag, which is a guess and only provisional.
//! What is compared is the question the verb pages need answered: is the word
//! a verb, and which lemma.

use std::collections::{BTreeMap, HashMap};
use std::io::Write;
use std::path::Path;

use serde::{Deserialize, Serialize};

use gramalysis::lang::Language;
use gramalysis::morph::Lexicon;
use gramalysis::{analyze, Error};

#[derive(Deserialize)]
pub struct Item {
    pub tatoeba_id: u64,
    pub text: String,
    pub form: String,
    pub occurrence: usize,
    pub spacy: Spacy,
}

#[derive(Deserialize)]
pub struct Spacy {
    pub pos: String,
    pub lemma: String,
}

/// Is it a verb, and which lemma: the decision a verb page needs.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, PartialOrd, Ord)]
pub struct Reading {
    pub verb: bool,
    pub lemma: String,
}

#[derive(Serialize)]
struct ItemResult<'a> {
    tatoeba_id: u64,
    text: &'a str,
    form: &'a str,
    status: Status,
    parses: u64,
    unknown: Vec<String>,
    /// The readings of the word used across the parses.
    readings: Vec<Reading>,
    reference: Option<Reading>,
    reference_source: &'static str,
    correct: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, PartialOrd, Ord)]
#[serde(rename_all = "kebab-case")]
enum Status {
    /// A word the lexicon does not know.
    UnknownWord,
    /// The grammar does not accept the sentence.
    NoParse,
    /// Parses disagree about the word.
    Ambiguous,
    /// Every parse reads the word the same way.
    Decided,
}

pub fn load_items(path: &Path) -> Result<Vec<Item>, Box<dyn std::error::Error>> {
    std::fs::read_to_string(path)?
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).map_err(|e| e.into()))
        .collect()
}

/// (tatoeba_id, form) → reading.
pub fn load_labels(path: &Path) -> Result<HashMap<(u64, String), Reading>, Box<dyn std::error::Error>> {
    let mut out = HashMap::new();
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(out),
        Err(e) => return Err(e.into()),
    };
    for (i, line) in text.lines().enumerate() {
        if line.starts_with('#') || line.trim().is_empty() {
            continue;
        }
        let cols: Vec<&str> = line.split('\t').collect();
        let [id, form, lemma, pos, ..] = cols[..] else {
            return Err(format!("{}:{}: expected id, form, lemma, pos", path.display(), i + 1).into());
        };
        out.insert((id.parse()?, form.to_owned()), Reading { verb: pos == "verb", lemma: lemma.to_owned() });
    }
    Ok(out)
}

/// A verb page needs one answer: which verb, or not a verb at all. Which noun
/// or adjective a non-verb is (`regalino` or `regalo`) does not matter.
fn same_for_a_verb_page(a: &Reading, b: &Reading) -> bool {
    a.verb == b.verb && (!a.verb || base_verb(&a.lemma) == base_verb(&b.lemma))
}

/// A pronominal infinitive's base verb: mettersi → mettere, smetterla →
/// smettere, andarsene → andare, porsi → porre. Dictionaries and labellers
/// name such verbs either way; for a verb page they are the same verb.
fn base_verb(lemma: &str) -> String {
    const CLITICS: [&str; 14] =
        ["sene", "sela", "selo", "cela", "celo", "gliela", "glielo", "si", "ci", "vi", "la", "lo", "le", "ne"];
    for c in CLITICS {
        if let Some(stem) = lemma.strip_suffix(c)
            && stem.len() >= 3
            && stem.ends_with('r')
        {
            // -rre verbs: porre, condurre, trarre.
            let rre = ["por", "dur", "trar"].iter().any(|s| stem.ends_with(s));
            return format!("{stem}{}", if rre { "re" } else { "e" });
        }
    }
    lemma.to_owned()
}

fn spacy_reading(s: &Spacy) -> Reading {
    Reading { verb: matches!(s.pos.as_str(), "VERB" | "AUX"), lemma: s.lemma.to_lowercase() }
}

pub async fn run(
    items: &[Item],
    labels: &HashMap<(u64, String), Reading>,
    lexicon: &dyn Lexicon,
    language: &dyn Language,
    out: Option<&Path>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut results = Vec::with_capacity(items.len());
    for item in items {
        results.push(evaluate(item, labels, lexicon, language).await?);
    }
    if let Some(path) = out {
        let mut f = std::io::BufWriter::new(std::fs::File::create(path)?);
        for r in &results {
            writeln!(f, "{}", serde_json::to_string(r)?)?;
        }
    }
    summarize(&results);
    Ok(())
}

async fn evaluate<'a>(
    item: &'a Item,
    labels: &HashMap<(u64, String), Reading>,
    lexicon: &dyn Lexicon,
    language: &dyn Language,
) -> Result<ItemResult<'a>, Error> {
    let a = analyze(lexicon, language, &item.text).await?;
    let token = a
        .tokens
        .iter()
        .enumerate()
        .filter(|(_, t)| t.to_lowercase() == item.form)
        .nth(item.occurrence)
        .map(|(i, _)| i);
    let mut readings: Vec<Reading> = token
        .map(|t| {
            a.chosen[t]
                .readings
                .iter()
                .filter_map(|r| r.first())
                .map(|m| Reading { verb: m.cat == "Verb", lemma: m.lemma.to_lowercase() })
                .collect()
        })
        .unwrap_or_default();
    readings.sort();
    readings.dedup();
    let status = if !a.unknown.is_empty() {
        Status::UnknownWord
    } else if a.parse_count == 0 {
        Status::NoParse
    } else if readings.len() == 1 {
        Status::Decided
    } else {
        Status::Ambiguous
    };
    let (reference, reference_source) = match labels.get(&(item.tatoeba_id, item.form.clone())) {
        Some(r) => (Some(r.clone()), "label"),
        None => (Some(spacy_reading(&item.spacy)), "spacy"),
    };
    let correct = match (status, &reference) {
        (Status::Decided, Some(r)) => Some(same_for_a_verb_page(&readings[0], r)),
        _ => None,
    };
    Ok(ItemResult {
        tatoeba_id: item.tatoeba_id,
        text: &item.text,
        form: &item.form,
        status,
        parses: a.parse_count,
        unknown: a.unknown,
        readings,
        reference,
        reference_source,
        correct,
    })
}

fn summarize(results: &[ItemResult]) {
    let n = results.len();
    let pct = |k: usize| if n == 0 { 0.0 } else { 100.0 * k as f64 / n as f64 };
    let mut by_status: BTreeMap<Status, usize> = BTreeMap::new();
    for r in results {
        *by_status.entry(r.status).or_default() += 1;
    }
    println!("items {n}");
    for (s, k) in &by_status {
        println!("  {:<13} {k:>5}  {:5.1}%", format!("{s:?}"), pct(*k));
    }
    for source in ["label", "spacy"] {
        let decided: Vec<&ItemResult> =
            results.iter().filter(|r| r.status == Status::Decided && r.reference_source == source).collect();
        if decided.is_empty() {
            continue;
        }
        let right = decided.iter().filter(|r| r.correct == Some(true)).count();
        let what = if source == "label" { "labels" } else { "spaCy (provisional reference)" };
        println!(
            "decided vs {what}: {right} of {} agree ({:.1}%)",
            decided.len(),
            100.0 * right as f64 / decided.len() as f64
        );
    }
    let mut unknown: BTreeMap<&str, usize> = BTreeMap::new();
    for r in results {
        for w in &r.unknown {
            *unknown.entry(w.as_str()).or_default() += 1;
        }
    }
    let mut unknown: Vec<(&str, usize)> = unknown.into_iter().collect();
    unknown.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
    if !unknown.is_empty() {
        let top: Vec<String> = unknown.iter().take(15).map(|(w, k)| format!("{w} {k}")).collect();
        println!("most frequent unknown words: {}", top.join(", "));
    }
}

#[cfg(test)]
mod tests {
    use super::base_verb;

    #[test]
    fn pronominal_infinitives_map_to_their_base_verb() {
        for (a, b) in [("mettersi", "mettere"), ("smetterla", "smettere"), ("andarsene", "andare"),
                       ("porsi", "porre"), ("prenderci", "prendere"), ("fare", "fare"), ("dire", "dire")] {
            assert_eq!(base_verb(a), b, "{a}");
        }
    }
}
