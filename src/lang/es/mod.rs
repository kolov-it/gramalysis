//! Spanish: tokenizer, function words, clitics, and the grammar in
//! `data/es/grammar.gram`.

mod clitics;
mod closed;
mod tokenize;

pub use clitics::{split_enclitics, strip_accent, Split};
pub use tokenize::tokenize;

use std::collections::{HashMap, HashSet};

use crate::lang::Language;
use crate::morph::{Analysis, FormKind, Lang, Mood, Number, PartOfSpeech, Person, Reading, Tense};
use crate::syntax::feats::{self, FeatMap};
use crate::syntax::grammar::{Grammar, GrammarError};
use crate::syntax::lattice::{Lattice, Morpheme};
use closed::{with_surface, FunctionWords};
use tokenize::is_punct;

const CLOSED: &str = include_str!("../../../data/es/closed.txt");
const GRAMMAR: &str = include_str!("../../../data/es/grammar.gram");
const DRIFTED: &str = include_str!("../../../data/es/drifted-participles.txt");

/// Categories the function-word list owns: the lexicon's readings in these
/// categories are replaced by the list's, its other readings are kept.
const OWNED_CATS: [&str; 5] = ["Det", "Pron", "Prep", "Conj", "Contraction"];

/// Feminine nouns that start with a stressed a / ha and so take el and un in
/// the singular (el agua, un águila). The lexicon has no stress data.
const STRESSED_A: [&str; 24] = [
    "agua", "águila", "alma", "arma", "hambre", "hacha", "área", "aula", "ala", "ancla", "arpa", "hada", "habla",
    "asma", "ave", "alba", "alga", "ama", "ánima", "aria", "haba", "hampa", "ansia", "asa",
];

#[derive(Debug, thiserror::Error)]
pub enum DataError {
    #[error("data/es/closed.txt line {line}: {message}")]
    Closed { line: usize, message: String },
    #[error("data/es/grammar.gram: {0}")]
    Grammar(#[from] GrammarError),
}

pub struct Spanish {
    closed: FunctionWords,
    /// Participle adjectives whose meaning drifted from the verb (divertido).
    drifted: HashSet<String>,
    grammar: Grammar,
}

impl Language for Spanish {
    fn lang(&self) -> Lang {
        Lang::Es
    }

    fn grammar(&self) -> &Grammar {
        &self.grammar
    }

    fn tokenize(&self, text: &str) -> Vec<String> {
        tokenize(text)
    }

    /// The word tokens, then the hosts of every possible clitic split (dámelo
    /// → da, dá): the lexicon does not analyse every verb + clitics compound,
    /// so the host is looked up on its own.
    fn lexicon_forms(&self, tokens: &[String]) -> Vec<String> {
        let words: Vec<String> = tokens.iter().filter(|t| !is_punct(t)).map(|t| t.to_lowercase()).collect();
        let mut seen: HashSet<String> = words.iter().cloned().collect();
        let mut forms = words.clone();
        for w in &words {
            for split in split_enclitics(w) {
                for host in split.hosts {
                    if seen.insert(host.clone()) {
                        forms.push(host);
                    }
                }
            }
        }
        forms
    }

    fn lattice(&self, tokens: &[String], analyses: &[Analysis]) -> Lattice {
        let forms = self.lexicon_forms(tokens);
        let by_form: HashMap<&str, &Analysis> = forms.iter().map(String::as_str).zip(analyses).collect();
        self.build_lattice(tokens, &by_form)
    }
}

impl Spanish {
    pub fn load() -> Result<Self, DataError> {
        Ok(Self { closed: closed::parse(CLOSED)?, drifted: word_list(DRIFTED), grammar: Grammar::parse(GRAMMAR)? })
    }

    fn build_lattice(&self, tokens: &[String], by_form: &HashMap<&str, &Analysis>) -> Lattice {
        let mut lattice = Lattice::new();
        for (i, token) in tokens.iter().enumerate() {
            if is_punct(token) {
                let morph = Morpheme { surface: token.clone(), lemma: token.clone(), cat: "Punct".to_owned(), feats: FeatMap::new() };
                lattice.push_token(token, vec![vec![morph]]);
                continue;
            }
            let lower = token.to_lowercase();
            let lexicon: &[Reading] = by_form.get(lower.as_str()).map(|a| a.readings.as_slice()).unwrap_or_default();
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
            let enclitic = self.enclitic_readings(&lower, token, lexicon, by_form);
            for reading in lexicon {
                let cat = feats::category(&reading.pos);
                if closed.is_some() && OWNED_CATS.contains(&cat.as_str()) {
                    continue;
                }
                // Conjunctions are all in the list, each with its type; a
                // lexicon conjunction has none and would match both.
                if cat == "Conj" {
                    continue;
                }
                // The list already has this word (`como` as an adverb, `bajo`
                // as an adjective): the list's readings win.
                let listed = closed.into_iter().flatten().any(|r| r.iter().any(|m| m.lemma == reading.lemma));
                if listed {
                    continue;
                }
                if reading.pos == PartOfSpeech::Verb {
                    // A verb + clitics compound (dámelo, hacerlo) is read from
                    // its host; a verb reading without any grammar would match
                    // every constraint, so it is never used as it is.
                    let bare = reading.features.mood.is_none() && reading.features.form_kind.is_none();
                    if reading.clitic || bare {
                        continue;
                    }
                }
                readings.push(vec![Morpheme {
                    surface: token.clone(),
                    lemma: reading.lemma.clone(),
                    cat,
                    feats: feats::from_features(&reading.features),
                }]);
            }
            readings.extend(enclitic);
            merge_participle_adjectives(&mut readings, &self.drifted);
            drop_infinitive_nouns(&mut readings);
            for reading in &mut readings {
                for m in reading.iter_mut() {
                    if m.cat == "Verb" {
                        m.feats.insert("class".to_owned(), verb_class(&m.lemma).to_owned());
                        // ir never takes a direct object: fue médico is ser.
                        let obj = if m.lemma == "ir" { "no" } else { "yes" };
                        m.feats.insert("obj".to_owned(), obj.to_owned());
                        if m.lemma == "haber" {
                            // ha only forms compound tenses; there is = hay.
                            let exist = if m.surface.to_lowercase() == "ha" { "no" } else { "yes" };
                            m.feats.insert("exist".to_owned(), exist.to_owned());
                        }
                    }
                }
                // Words from the lexicon carry the sound they start with, for
                // the article before them to check (la casa, el agua).
                if let Some(first) = reading.first_mut()
                    && !OWNED_CATS.contains(&first.cat.as_str())
                {
                    first.feats.insert("onset".to_owned(), onset(&lower).to_owned());
                }
            }
            let capitalised = token.chars().next().is_some_and(char::is_uppercase);
            // A number written in digits (10, 1.000) is a numeral; the
            // lexicon has no entries for digits.
            if readings.is_empty() && token.chars().next().is_some_and(|c| c.is_ascii_digit())
                && token.chars().all(|c| c.is_ascii_digit() || c == '.' || c == ',')
            {
                let mut feats = FeatMap::new();
                feats.insert("number".to_owned(), if token == "1" { "sg" } else { "pl" }.to_owned());
                readings.push(vec![Morpheme { surface: token.clone(), lemma: token.clone(), cat: "Num".to_owned(), feats }]);
            }
            // A capital after . ? ! or : starts a sentence, like the first word.
            let starts_sentence = i == 0 || matches!(tokens[i - 1].as_str(), "." | "?" | "!" | ":");
            if capitalised && (!starts_sentence || readings.is_empty()) {
                let mut feats = FeatMap::new();
                feats.insert("onset".to_owned(), onset(&lower).to_owned());
                readings.push(vec![Morpheme { surface: token.clone(), lemma: token.clone(), cat: "ProperNoun".to_owned(), feats }]);
            }
            lattice.push_token(token, readings);
        }
        lattice
    }

    /// The token read as a verb host + attached clitics (dámelo = da + me +
    /// lo). The host's readings come from the lexicon's answer for the host
    /// itself, limited to the forms clitics attach to; when the lexicon knows
    /// none, from the token's own clitic readings (hacerlo: infinitive).
    fn enclitic_readings(
        &self,
        lower: &str,
        surface: &str,
        lexicon: &[Reading],
        by_form: &HashMap<&str, &Analysis>,
    ) -> Vec<Vec<Morpheme>> {
        let splits = split_enclitics(lower);
        let mut found: Vec<(&Split, Vec<Morpheme>)> = Vec::new();
        for split in &splits {
            let mut hosts: Vec<Morpheme> = Vec::new();
            for host in &split.hosts {
                for r in by_form.get(host.as_str()).map(|a| a.readings.as_slice()).unwrap_or_default() {
                    // A host that is itself a compound (dame in dámelo) is
                    // another split of the same token.
                    if r.pos == PartOfSpeech::Verb && !r.clitic && takes_enclitics(r) {
                        push_host(&mut hosts, r);
                    }
                }
            }
            if !hosts.is_empty() {
                found.push((split, hosts));
            }
        }
        // The token's own fully tagged clitic readings (iros: infinitive and
        // vosotros imperative, where the host ir has only the infinitive) join
        // the first split whose host is known, or the longest split when no
        // host spelling is known on its own.
        let own: Vec<&Reading> =
            lexicon.iter().filter(|r| r.pos == PartOfSpeech::Verb && r.clitic && takes_enclitics(r)).collect();
        if !own.is_empty() {
            if let Some((_, hosts)) = found.first_mut() {
                for r in &own {
                    push_host(hosts, r);
                }
            } else if let Some(split) = splits.first() {
                let mut hosts: Vec<Morpheme> = Vec::new();
                for r in &own {
                    push_host(&mut hosts, r);
                }
                found.push((split, hosts));
            }
        }
        let mut out: Vec<Vec<Morpheme>> = Vec::new();
        for (split, hosts) in found {
            let host_surface = &surface[..surface.len() - split.clitics.iter().map(String::len).sum::<usize>()];
            let mut sequences: Vec<Vec<Morpheme>> =
                hosts.into_iter().map(|h| vec![Morpheme { surface: host_surface.to_owned(), ..h }]).collect();
            for clitic in &split.clitics {
                let options: Vec<Morpheme> = self
                    .closed
                    .get(clitic)
                    .into_iter()
                    .flatten()
                    .filter(|r| r.len() == 1)
                    .map(|r| r[0].clone())
                    .filter(|m| m.feats.get("clitic").map(String::as_str) == Some("yes"))
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
            out.extend(sequences);
        }
        out
    }
}

fn push_host(hosts: &mut Vec<Morpheme>, r: &Reading) {
    let m = Morpheme { surface: String::new(), lemma: r.lemma.clone(), cat: "Verb".to_owned(), feats: feats::from_features(&r.features) };
    if !hosts.iter().any(|h| h.lemma == m.lemma && h.feats == m.feats) {
        hosts.push(m);
    }
}

/// Clitics attach to an infinitive, a gerund and an affirmative imperative,
/// including the usted / ustedes / nosotros forms the subjunctive lends it.
fn takes_enclitics(r: &Reading) -> bool {
    let f = &r.features;
    matches!(f.form_kind, Some(FormKind::Infinitive | FormKind::Gerund))
        || f.mood == Some(Mood::Imperative)
        || (f.mood == Some(Mood::Subjunctive)
            && f.tense == Some(Tense::Present)
            && (f.person == Some(Person::Third) || (f.person == Some(Person::First) && f.number == Some(Number::Plural))))
}

/// The class every verb carries, so the grammar can select the auxiliaries
/// without relying on a feature ordinary verbs lack: ser (copula, passive),
/// estar (state, progressive), haber (compound tenses, hay), lex (the rest).
fn verb_class(lemma: &str) -> &'static str {
    match lemma {
        "ser" => "ser",
        "estar" => "estar",
        "haber" => "haber",
        _ => "lex",
    }
}

/// `a` for a feminine noun starting with a stressed a (el agua), `c` for any
/// other word.
pub fn onset(word: &str) -> &'static str {
    if STRESSED_A.contains(&word) { "a" } else { "c" }
}

/// A past participle is one form of its verb, whether the sentence uses it
/// as a verb (ha cerrado, fue cerrada) or like an adjective (la puerta está
/// cerrada); the grammar's `ap.participle` rule covers the second use. So the
/// dictionary's separate adjective entry for the same form (cerrado) is
/// dropped, except for an adjective whose meaning drifted from the verb
/// (`drifted`: divertido is "funny"); its participle gets `drifted=yes`,
/// which `ap.participle` refuses. Adjectives are marked `participle=no`:
/// Spanish has no present participles that would make one a verb.
fn merge_participle_adjectives(readings: &mut Vec<Vec<Morpheme>>, drifted: &HashSet<String>) {
    let is_past = |r: &Vec<Morpheme>| {
        matches!(r.as_slice(), [m] if m.cat == "Verb" && m.feats.get("form").map(String::as_str) == Some("pastpart"))
    };
    let past = readings.iter().any(is_past);
    let surface = readings.iter().find_map(|r| r.first()).map(|m| m.surface.to_lowercase()).unwrap_or_default();
    let is_drifted = past && masculine_singular(&surface).is_some_and(|ms| drifted.contains(&ms));
    if past && !is_drifted {
        readings.retain(|r| !matches!(r.as_slice(), [m] if m.cat == "Adj"));
    }
    for r in readings.iter_mut() {
        let [m] = r.as_mut_slice() else { continue };
        if m.cat == "Adj" {
            m.feats.insert("participle".to_owned(), "no".to_owned());
        } else if is_drifted && m.cat == "Verb" && m.feats.get("form").map(String::as_str) == Some("pastpart") {
            m.feats.insert("drifted".to_owned(), "yes".to_owned());
        }
    }
}

/// Infinitives that became nouns of their own (el deber, el poder, el
/// placer): their noun reading stays next to the verb's.
const INFINITIVE_NOUNS: [&str; 12] =
    ["deber", "poder", "placer", "amanecer", "atardecer", "anochecer", "parecer", "ser", "pesar", "cantar", "querer", "andar"];

/// An infinitive is the verb, also with an article (el fumar, el comer);
/// the dictionary's noun entry for the same word (comer, hacer) would only
/// pose as a second answer, so it is dropped unless the infinitive became a
/// noun of its own (`INFINITIVE_NOUNS`).
fn drop_infinitive_nouns(readings: &mut Vec<Vec<Morpheme>>) {
    let infinitive = readings.iter().find_map(|r| match r.as_slice() {
        [m] if m.cat == "Verb" && m.feats.get("form").map(String::as_str) == Some("inf") => Some(m.lemma.clone()),
        _ => None,
    });
    if let Some(lemma) = infinitive
        && lemma.ends_with(['r', 'e'])
        && !INFINITIVE_NOUNS.contains(&lemma.as_str())
    {
        readings.retain(|r| !matches!(r.as_slice(), [m] if m.cat == "Noun" && m.lemma == lemma));
    }
}

/// A past participle's masculine singular: cerrada, cerrados, cerradas →
/// cerrado.
fn masculine_singular(form: &str) -> Option<String> {
    for (end, stem_len) in [("os", 2), ("as", 2), ("o", 1), ("a", 1)] {
        if form.ends_with(end) {
            return Some(format!("{}o", &form[..form.len() - stem_len]));
        }
    }
    None
}

fn word_list(text: &str) -> HashSet<String> {
    text.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')).map(str::to_owned).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn function_word_list_loads() {
        let es = Spanish::load().unwrap();
        assert_eq!(es.closed["del"][0].len(), 2);
        assert!(es.closed["se"].len() >= 2);
    }

    #[test]
    fn masculine_singular_of_participles() {
        assert_eq!(masculine_singular("cerradas").as_deref(), Some("cerrado"));
        assert_eq!(masculine_singular("divertida").as_deref(), Some("divertido"));
    }
}
