use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Lang {
    It,
    Es,
    Fr,
    De,
    Nl,
    En,
}

impl Lang {
    pub fn code(self) -> &'static str {
        match self {
            Lang::It => "it",
            Lang::Es => "es",
            Lang::Fr => "fr",
            Lang::De => "de",
            Lang::Nl => "nl",
            Lang::En => "en",
        }
    }
}

/// Every reading the lexicon knows for one surface form. The lexicon never
/// chooses between them: `ama` is both present 3sg and imperative 2sg of
/// `amare`, and only the sentence can tell which.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Analysis {
    pub form: String,
    pub readings: Vec<Reading>,
}

impl Analysis {
    pub fn is_known(&self) -> bool {
        !self.readings.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Reading {
    /// The dictionary entry: `fare` for `fece`.
    pub lemma: String,
    pub pos: PartOfSpeech,
    pub features: Features,
    /// The form carries attached clitics or is a contraction that was taken
    /// apart to reach the lemma (`prendendogliela`, `dámelo`).
    pub clitic: bool,
    /// The source's own tags, kept for explanations and for features this
    /// crate does not model yet.
    pub raw_tags: Vec<String>,
    pub glosses: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PartOfSpeech {
    Verb,
    Noun,
    ProperNoun,
    Adjective,
    Adverb,
    Pronoun,
    Determiner,
    Article,
    Preposition,
    Conjunction,
    Numeral,
    Interjection,
    Particle,
    Contraction,
    /// A tag this crate does not map yet, kept verbatim.
    Other(String),
}

impl PartOfSpeech {
    /// From the Wiktionary/Kaikki tag the Word Holder API and the patch files
    /// use: verb, noun, name, adj, adv, pron, det, article, prep, conj, num,
    /// intj, particle, contraction.
    pub fn from_tag(tag: &str) -> PartOfSpeech {
        match tag {
            "verb" => PartOfSpeech::Verb,
            "noun" => PartOfSpeech::Noun,
            "name" => PartOfSpeech::ProperNoun,
            "adj" => PartOfSpeech::Adjective,
            "adv" => PartOfSpeech::Adverb,
            "pron" => PartOfSpeech::Pronoun,
            "det" => PartOfSpeech::Determiner,
            "article" => PartOfSpeech::Article,
            "prep" => PartOfSpeech::Preposition,
            "conj" => PartOfSpeech::Conjunction,
            "num" => PartOfSpeech::Numeral,
            "intj" => PartOfSpeech::Interjection,
            "particle" => PartOfSpeech::Particle,
            "contraction" => PartOfSpeech::Contraction,
            other => PartOfSpeech::Other(other.to_owned()),
        }
    }
}

/// Grammatical features of one reading. Values follow the source's
/// distinctions (e.g. `Preterite` vs `Past`) so nothing is lost; mapping to
/// Universal Dependencies happens at export.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Features {
    pub mood: Option<Mood>,
    pub tense: Option<Tense>,
    pub person: Option<Person>,
    pub number: Option<Number>,
    pub gender: Option<Gender>,
    pub form_kind: Option<FormKind>,
    /// For a verb: the auxiliaries it takes in compound tenses, as the
    /// lexicon gives them (Italian ["avere"], ["essere"] or both). Empty when
    /// the lexicon does not say.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub auxiliaries: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Mood {
    Indicative,
    Subjunctive,
    Conditional,
    Imperative,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Tense {
    Present,
    Past,
    Future,
    Imperfect,
    Preterite,
    Pluperfect,
    Perfect,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Person {
    First,
    Second,
    Third,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Number {
    Singular,
    Plural,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Gender {
    Masculine,
    Feminine,
    Neuter,
}

/// Non-finite forms.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FormKind {
    Gerund,
    Infinitive,
    Participle,
    PastParticiple,
    PresentParticiple,
}

#[derive(Debug, thiserror::Error)]
pub enum LexiconError {
    #[error("lexicon request failed: {0}")]
    Transport(Box<dyn std::error::Error + Send + Sync>),
    #[error("lexicon returned HTTP {status}: {body}")]
    Http { status: u16, body: String },
    #[error("lexicon response is not what was expected: {0}")]
    Protocol(String),
}

/// Form → readings. Implementations answer many forms per call so that a
/// sentence costs one round trip, not one per word.
#[async_trait]
pub trait Lexicon: Send + Sync {
    /// One `Analysis` per input form, in input order. An unknown form gets an
    /// `Analysis` with no readings, not an error.
    async fn analyze(&self, lang: Lang, forms: &[&str]) -> Result<Vec<Analysis>, LexiconError>;
}
