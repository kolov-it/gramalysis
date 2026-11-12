//! The flat feature map the grammar works on. Values are short strings
//! (`person=3`, `number=sg`, `form=pastpart`); a feature that is absent is
//! unknown, and matches any constraint.

use std::collections::BTreeMap;

use crate::morph::{FormKind, Features, Gender, Mood, Number, PartOfSpeech, Person, Tense};

pub type FeatMap = BTreeMap<String, String>;

/// The grammar's terminal category for a part of speech.
pub fn category(pos: &PartOfSpeech) -> String {
    match pos {
        PartOfSpeech::Verb => "Verb",
        PartOfSpeech::Noun => "Noun",
        PartOfSpeech::ProperNoun => "ProperNoun",
        PartOfSpeech::Adjective => "Adj",
        PartOfSpeech::Adverb => "Adv",
        PartOfSpeech::Pronoun => "Pron",
        PartOfSpeech::Determiner | PartOfSpeech::Article => "Det",
        PartOfSpeech::Preposition => "Prep",
        PartOfSpeech::Conjunction => "Conj",
        PartOfSpeech::Numeral => "Num",
        PartOfSpeech::Interjection => "Intj",
        PartOfSpeech::Particle => "Part",
        PartOfSpeech::Contraction => "Contraction",
        PartOfSpeech::Other(s) => s.as_str(),
    }
    .to_owned()
}

pub fn from_features(f: &Features) -> FeatMap {
    let mut m = FeatMap::new();
    let mut put = |k: &str, v: &str| {
        m.insert(k.to_owned(), v.to_owned());
    };
    if let Some(mood) = f.mood {
        put("mood", match mood {
            Mood::Indicative => "ind",
            Mood::Subjunctive => "sub",
            Mood::Conditional => "cnd",
            Mood::Imperative => "imp",
        });
        put("finite", "yes");
        // Conjugated: never matches a constraint asking for an infinitive,
        // gerund or participle.
        put("form", "fin");
    }
    if let Some(t) = f.tense {
        put("tense", match t {
            Tense::Present => "pres",
            Tense::Past => "past",
            Tense::Future => "fut",
            Tense::Imperfect => "impf",
            Tense::Preterite => "pret",
            Tense::Pluperfect => "pqp",
            Tense::Perfect => "perf",
        });
    }
    if let Some(p) = f.person {
        put("person", match p {
            Person::First => "1",
            Person::Second => "2",
            Person::Third => "3",
        });
    }
    if let Some(n) = f.number {
        put("number", match n {
            Number::Singular => "sg",
            Number::Plural => "pl",
        });
    }
    if let Some(g) = f.gender {
        put("gender", match g {
            Gender::Masculine => "m",
            Gender::Feminine => "f",
            Gender::Neuter => "n",
        });
    }
    if let Some(k) = f.form_kind {
        put("form", match k {
            FormKind::Gerund => "ger",
            FormKind::Infinitive => "inf",
            FormKind::Participle => "part",
            FormKind::PastParticiple => "pastpart",
            FormKind::PresentParticiple => "prespart",
        });
        put("finite", "no");
    }
    m
}

pub fn display(m: &FeatMap) -> String {
    m.iter()
        // lemma is shown separately; onset only steers the article.
        .filter(|(k, _)| !matches!(k.as_str(), "lemma" | "onset"))
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join(",")
}
