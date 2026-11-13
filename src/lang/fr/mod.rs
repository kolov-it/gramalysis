//! French: tokenizer, function words, the sound a word starts with, and the
//! grammar in `data/fr/grammar.gram`.

mod closed;
mod sound;
mod tokenize;

pub use sound::onset;
pub use tokenize::tokenize;

use std::collections::HashSet;

use crate::lang::Language;
use crate::morph::{Analysis, Lang, PartOfSpeech, Reading};
use crate::syntax::feats::{self, FeatMap};
use crate::syntax::grammar::{Grammar, GrammarError};
use crate::syntax::lattice::{Lattice, Morpheme};
use closed::{with_surface, FunctionWords};
use tokenize::{is_punct, DEMONSTRATIVE};

const CLOSED: &str = include_str!("../../../data/fr/closed.txt");
const GRAMMAR: &str = include_str!("../../../data/fr/grammar.gram");
const DRIFTED: &str = include_str!("../../../data/fr/drifted-participles.txt");
const ASPIRATED: &str = include_str!("../../../data/fr/aspirated-h.txt");

/// Categories the function-word list owns: the lexicon's readings in these
/// categories are replaced by the list's, its other readings (`son` the
/// sound, `or` the gold) are kept unless the list names their lemma.
const OWNED_CATS: [&str; 6] = ["Det", "Pron", "Prep", "Conj", "Contraction", "ProperNoun"];

#[derive(Debug, thiserror::Error)]
pub enum DataError {
    #[error("data/fr/closed.txt line {line}: {message}")]
    Closed { line: usize, message: String },
    #[error("data/fr/grammar.gram: {0}")]
    Grammar(#[from] GrammarError),
}

pub struct French {
    closed: FunctionWords,
    /// Participle adjectives whose meaning drifted from the verb.
    drifted: HashSet<String>,
    /// Words starting with an aspirated h (le héros).
    aspirated: HashSet<String>,
    grammar: Grammar,
}

impl Language for French {
    fn lang(&self) -> Lang {
        Lang::Fr
    }

    fn grammar(&self) -> &Grammar {
        &self.grammar
    }

    fn tokenize(&self, text: &str) -> Vec<String> {
        tokenize(text)
    }

    fn lexicon_forms(&self, tokens: &[String]) -> Vec<String> {
        tokens.iter().filter(|t| !is_punct(t)).map(|t| t.trim_start_matches('-').to_lowercase()).collect()
    }

    fn lattice(&self, tokens: &[String], analyses: &[Analysis]) -> Lattice {
        self.build_lattice(tokens, analyses)
    }
}

impl French {
    pub fn load() -> Result<Self, DataError> {
        Ok(Self {
            closed: closed::parse(CLOSED)?,
            drifted: word_list(DRIFTED),
            aspirated: word_list(ASPIRATED).into_iter().map(|w| w.to_lowercase()).collect(),
            grammar: Grammar::parse(GRAMMAR)?,
        })
    }

    fn build_lattice(&self, tokens: &[String], analyses: &[Analysis]) -> Lattice {
        let mut lattice = Lattice::new();
        let mut words = analyses.iter();
        for (i, token) in tokens.iter().enumerate() {
            if is_punct(token) {
                // A question mark is its own category: it makes a question.
                let cat = if token == "?" { "Question" } else { "Punct" };
                let morph = Morpheme { surface: token.clone(), lemma: token.clone(), cat: cat.to_owned(), feats: FeatMap::new() };
                lattice.push_token(token, vec![vec![morph]]);
                continue;
            }
            let analysis = words.next();
            // A pronoun after a hyphen (parlez-vous, donne-le-moi) is attached
            // to the verb before it: only its pronoun readings stand.
            if let Some(bare) = token.strip_prefix('-') {
                let lower = bare.to_lowercase();
                if DEMONSTRATIVE.contains(&lower.as_str()) {
                    let morph = Morpheme { surface: token.clone(), lemma: lower, cat: "DemSuffix".to_owned(), feats: FeatMap::new() };
                    lattice.push_token(token, vec![vec![morph]]);
                    continue;
                }
                let readings = self
                    .closed
                    .get(&lower)
                    .into_iter()
                    .flatten()
                    .filter(|r| r.len() == 1 && r[0].cat == "Pron" && r[0].feats.get("clitic").map(String::as_str) == Some("yes"))
                    .map(|r| {
                        let mut m = with_surface(r, token);
                        m[0].feats.insert("attached".to_owned(), "yes".to_owned());
                        m
                    })
                    .collect();
                lattice.push_token(token, readings);
                continue;
            }
            let lower = token.to_lowercase();
            let mut readings: Vec<Vec<Morpheme>> = Vec::new();
            let closed = self.closed.get(&lower);
            if let Some(closed) = closed {
                readings.extend(closed.iter().map(|r| with_surface(r, token)).map(|mut r| {
                    // An object pronoun written as its own word stands before
                    // the verb (the imperative's moi, toi only after it).
                    for m in &mut r {
                        if m.feats.get("clitic").map(String::as_str) == Some("yes") {
                            m.feats.insert("attached".to_owned(), "no".to_owned());
                        }
                    }
                    r
                }));
                readings.retain(|r| !r.iter().any(|m| m.feats.get("imperative").map(String::as_str) == Some("yes")));
            }
            let all_caps = token.chars().filter(|c| c.is_alphabetic()).all(char::is_uppercase);
            let capitalised = token.chars().next().is_some_and(char::is_uppercase);
            let lexicon: Vec<Reading> = analysis
                .map(|a| a.readings.clone())
                .unwrap_or_default()
                .into_iter()
                // The lexicon is asked in lower case, so it also answers with
                // acronyms (ait: AIT, ce: CE); one stands only for a word
                // written in capitals.
                .filter(|r| all_caps || !is_acronym(&r.lemma))
                // …and with names (mariage: Mariage); a name stands only for
                // a capitalised word.
                .filter(|r| capitalised || r.pos != PartOfSpeech::ProperNoun)
                .collect();
            let lexicon = prefer_traditional_spelling(
                lexicon
                    .into_iter()
                    .map(without_pronoun)
                    .map(|r| without_defaulted_features(r, &lower))
                    .map(invariable_number)
                    .collect(),
            );
            for reading in &lexicon {
                let cat = feats::category(&reading.pos);
                if closed.is_some() && OWNED_CATS.contains(&cat.as_str()) {
                    continue;
                }
                // The list already has this word (`pas` the noun, `bien` the
                // adverb): the list's readings win.
                let listed = closed.into_iter().flatten().any(|r| r.iter().any(|m| m.lemma == reading.lemma));
                if listed {
                    continue;
                }
                let morphs = vec![self.morpheme(token, reading)];
                if !readings.iter().any(|r| same(r, &morphs)) {
                    readings.push(morphs);
                }
            }
            merge_participle_adjectives(&mut readings, &self.drifted);
            // Only par introduces an agent (closed.txt); every other
            // preposition says it does not, for `vp.agent-phrase`.
            for m in readings.iter_mut().flatten().filter(|m| m.cat == "Prep") {
                m.feats.entry("agent".to_owned()).or_insert_with(|| "no".to_owned());
            }
            // Words from the lexicon carry the sound they start with, for the
            // determiner before them to check (le chat, l'ami, le héros).
            let first_sound = onset(&lower, &self.aspirated);
            for reading in &mut readings {
                if let Some(first) = reading.first_mut()
                    && !OWNED_CATS.contains(&first.cat.as_str())
                {
                    first.feats.insert("onset".to_owned(), first_sound.to_owned());
                }
            }
            // A number written in digits (3, 1966, 9,4 %), whatever else the
            // lexicon reads it as (20 as a noun, 2 as de).
            if token.chars().any(|c| c.is_ascii_digit()) {
                readings.push(vec![Morpheme { surface: token.clone(), lemma: lower.clone(), cat: "Num".to_owned(), feats: FeatMap::new() }]);
            }
            // A capitalised word is a name anywhere but at the start, where
            // it is one only if the lexicon knows it capitalised (Sami).
            let known_capitalised = lexicon.iter().any(|r| r.lemma.chars().next().is_some_and(char::is_uppercase));
            if capitalised && (i > 0 || readings.is_empty() || known_capitalised) {
                let mut feats = FeatMap::new();
                feats.insert("onset".to_owned(), first_sound.to_owned());
                readings.push(vec![Morpheme { surface: token.clone(), lemma: token.clone(), cat: "ProperNoun".to_owned(), feats }]);
            }
            lattice.push_token(token, readings);
        }
        lattice
    }

    fn morpheme(&self, surface: &str, reading: &Reading) -> Morpheme {
        let mut feats = feats::from_features(&reading.features);
        // The auxiliary the verb takes in compound tenses, when the lexicon
        // says: the grammar tells a compound tense (elle est partie) from a
        // state (la porte est fermée) by it. Unknown leaves both open.
        if reading.pos == PartOfSpeech::Verb {
            feats.insert("etre".to_owned(), if reading.lemma == "être" { "yes" } else { "no" }.to_owned());
        }
        if reading.pos == PartOfSpeech::Verb && !reading.features.auxiliaries.is_empty() {
            for (aux, name) in [("avoir", "avoir"), ("être", "etre")] {
                let takes = reading.features.auxiliaries.iter().any(|a| a == aux);
                feats.insert(format!("takes_{name}"), if takes { "yes" } else { "no" }.to_owned());
            }
        }
        Morpheme { surface: surface.to_owned(), lemma: reading.lemma.clone(), cat: feats::category(&reading.pos), feats }
    }
}

fn same(a: &[Morpheme], b: &[Morpheme]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.lemma == y.lemma && x.cat == y.cat && x.feats == y.feats)
}

/// A pronominal verb is its verb: `se lever` and `s'enfuir` are read as
/// `lever` and `enfuir`; the grammar sees the reflexive pronoun in the
/// sentence. The lexicon gives some forms both ways (`tiré`: tirer and se
/// tirer), which would only pose as two answers.
fn without_pronoun(mut reading: Reading) -> Reading {
    if reading.pos == PartOfSpeech::Verb
        && let Some(base) = reading.lemma.strip_prefix("se ").or_else(|| reading.lemma.strip_prefix("s'"))
    {
        reading.lemma = base.to_owned();
    }
    reading
}

/// A lemma of two or more letters, all capitals (AIT, CE, USA).
fn is_acronym(lemma: &str) -> bool {
    let letters: Vec<char> = lemma.chars().filter(|c| c.is_alphabetic()).collect();
    letters.len() >= 2 && letters.iter().all(|c| c.is_uppercase())
}

/// The API gives a French dictionary form (the form that is the lemma) the
/// masculine singular whether or not it has one: `facile`, `jeune`, `rouge`
/// are feminine too, `heureux`, `temps`, `bras` plural too. Such a reading has
/// no source tags; its gender (an adjective in -e) and number (a word in -s,
/// -x, -z) are left unknown, so agreement accepts both. A dictionary form
/// whose source does tag it (grand: masculine) keeps its features.
fn without_defaulted_features(mut reading: Reading, form: &str) -> Reading {
    let dictionary_form = reading.raw_tags.is_empty() && reading.lemma == form;
    if !dictionary_form || !matches!(reading.pos, PartOfSpeech::Adjective | PartOfSpeech::Noun) {
        return reading;
    }
    if reading.pos == PartOfSpeech::Adjective && form.ends_with('e') {
        reading.features.gender = None;
    }
    if form.ends_with(['s', 'x', 'z']) {
        reading.features.number = None;
    }
    reading
}

/// A noun the source tags `invariable` (un/des sursis) is singular and
/// plural, whatever number the API gives it.
fn invariable_number(mut reading: Reading) -> Reading {
    if reading.raw_tags.iter().any(|t| t == "invariable") {
        reading.features.number = None;
    }
    reading
}

/// Of two spellings of one verb, the traditional (apparaître) and the 1990
/// reform's (apparaitre), keep the traditional: they are one verb. A verb in
/// -aitre / -oitre alone is named by its traditional spelling (paraitre →
/// paraître), as the verb pages name it.
fn prefer_traditional_spelling(readings: Vec<Reading>) -> Vec<Reading> {
    let fold = |s: &str| s.replace('î', "i").replace('û', "u");
    let lemmas: HashSet<String> = readings.iter().map(|r| r.lemma.clone()).collect();
    readings
        .into_iter()
        .filter(|r| !lemmas.iter().any(|other| other != &r.lemma && fold(other) == r.lemma))
        .map(|mut r| {
            if r.pos == PartOfSpeech::Verb {
                for (reform, traditional) in [("aitre", "aître"), ("oitre", "oître")] {
                    if let Some(stem) = r.lemma.strip_suffix(reform) {
                        r.lemma = format!("{stem}{traditional}");
                    }
                }
            }
            r
        })
        .collect()
}

/// A past participle is one form of its verb, whether the sentence uses it
/// as a verb (j'ai fermé la porte) or like an adjective (la porte fermée);
/// the grammar's `ap.participle` rule covers the second use. So the
/// dictionary's separate adjective entry for the same form (fermé, écrit) is
/// dropped: it would only pose as a second answer. An adjective whose meaning
/// drifted from the verb (`drifted`) stays its own word, and its participle
/// gets `drifted=yes`, which `ap.participle` refuses. A present participle's
/// adjective (parlant, brillant) is kept: in French it is almost always an
/// adjective, and a verb only with the verb's complements.
///
/// Adjectives are marked `participle=<verb>` when the token is also that
/// verb's present participle, `participle=no` otherwise, so `ap.complement`
/// can leave a participle with complements to the participle phrase.
fn merge_participle_adjectives(readings: &mut Vec<Vec<Morpheme>>, drifted: &HashSet<String>) {
    let participle = |r: &Vec<Morpheme>, kind: &str| match r.as_slice() {
        [m] if m.cat == "Verb" && m.feats.get("form").map(String::as_str) == Some(kind) => Some(m.lemma.clone()),
        _ => None,
    };
    let past = readings.iter().find_map(|r| participle(r, "pastpart"));
    let present = readings.iter().find_map(|r| participle(r, "prespart"));
    let surface = readings.iter().find_map(|r| r.first()).map(|m| m.surface.to_lowercase()).unwrap_or_default();
    let is_drifted = past.is_some() && masculine_singulars(&surface).iter().any(|ms| drifted.contains(ms));
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

/// The masculine singular a participle form can come from: fermées → fermé,
/// écrite → écrit, pris → pris.
fn masculine_singulars(form: &str) -> Vec<String> {
    let mut out = vec![form.to_owned()];
    for end in ["es", "e", "s"] {
        if let Some(stem) = form.strip_suffix(end) {
            out.push(stem.to_owned());
        }
    }
    out
}

fn word_list(text: &str) -> HashSet<String> {
    text.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')).map(str::to_owned).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn function_word_list_loads() {
        let fr = French::load().unwrap();
        assert_eq!(fr.closed["du"].len(), 2);
        assert!(fr.closed["au"].iter().any(|r| r.len() == 2));
        assert!(fr.closed["le"].len() >= 2);
    }

    #[test]
    fn traditional_spelling_wins() {
        let r = |lemma: &str| Reading {
            lemma: lemma.to_owned(),
            pos: PartOfSpeech::Verb,
            features: Default::default(),
            clitic: false,
            raw_tags: vec![],
            glosses: vec![],
        };
        let kept = prefer_traditional_spelling(vec![r("apparaître"), r("apparaitre"), r("se tirer")]);
        assert_eq!(kept.iter().map(|r| r.lemma.as_str()).collect::<Vec<_>>(), ["apparaître", "se tirer"]);
        assert_eq!(prefer_traditional_spelling(vec![r("comparaitre")])[0].lemma, "comparaître");
        assert_eq!(without_pronoun(r("se tirer")).lemma, "tirer");
        assert_eq!(without_pronoun(r("s'enfuir")).lemma, "enfuir");
    }
}
