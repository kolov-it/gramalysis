//! The tenses a language has, by the identifiers examples use: the simple
//! ones as the verb tables (`data/<lang>/verbs.json`) name them, the compound
//! ones named here after the auxiliary's tense they are built from.
//!
//! A cell is a tense and, for a finite one, a person: `indicative.present.3s`,
//! `indicative.present_perfect.3s`, `non_finite.perfect_infinitive`.

use serde::Serialize;

use crate::morph::Lang;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Simple,
    /// An auxiliary in `auxiliary_tense` + the past participle.
    Compound,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct TenseInfo {
    pub id: &'static str,
    /// What the language's grammars call it.
    pub native: &'static str,
    pub english: &'static str,
    pub kind: Kind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auxiliary_tense: Option<&'static str>,
}

const fn simple(id: &'static str, native: &'static str, english: &'static str) -> TenseInfo {
    TenseInfo {
        id,
        native,
        english,
        kind: Kind::Simple,
        auxiliary_tense: None,
    }
}

const fn compound(
    id: &'static str,
    native: &'static str,
    english: &'static str,
    aux: &'static str,
) -> TenseInfo {
    TenseInfo {
        id,
        native,
        english,
        kind: Kind::Compound,
        auxiliary_tense: Some(aux),
    }
}

const ITALIAN: &[TenseInfo] = &[
    simple("indicative.present", "presente", "present"),
    simple("indicative.imperfect", "imperfetto", "imperfect"),
    simple(
        "indicative.past_historic",
        "passato remoto",
        "past historic",
    ),
    simple("indicative.future", "futuro semplice", "future"),
    simple(
        "subjunctive.present",
        "congiuntivo presente",
        "present subjunctive",
    ),
    simple(
        "subjunctive.imperfect",
        "congiuntivo imperfetto",
        "imperfect subjunctive",
    ),
    simple(
        "conditional.present",
        "condizionale presente",
        "present conditional",
    ),
    simple("imperative.present", "imperativo", "imperative"),
    simple("non_finite.infinitive", "infinito", "infinitive"),
    simple("non_finite.gerund", "gerundio", "gerund"),
    simple(
        "non_finite.past_participle",
        "participio passato",
        "past participle",
    ),
    simple(
        "non_finite.present_participle",
        "participio presente",
        "present participle",
    ),
    compound(
        "indicative.present_perfect",
        "passato prossimo",
        "present perfect",
        "indicative.present",
    ),
    compound(
        "indicative.pluperfect",
        "trapassato prossimo",
        "pluperfect",
        "indicative.imperfect",
    ),
    compound(
        "indicative.past_anterior",
        "trapassato remoto",
        "past anterior",
        "indicative.past_historic",
    ),
    compound(
        "indicative.future_perfect",
        "futuro anteriore",
        "future perfect",
        "indicative.future",
    ),
    compound(
        "subjunctive.perfect",
        "congiuntivo passato",
        "perfect subjunctive",
        "subjunctive.present",
    ),
    compound(
        "subjunctive.pluperfect",
        "congiuntivo trapassato",
        "pluperfect subjunctive",
        "subjunctive.imperfect",
    ),
    compound(
        "conditional.perfect",
        "condizionale passato",
        "perfect conditional",
        "conditional.present",
    ),
    compound(
        "non_finite.perfect_infinitive",
        "infinito passato",
        "perfect infinitive",
        "non_finite.infinitive",
    ),
    compound(
        "non_finite.perfect_gerund",
        "gerundio passato",
        "perfect gerund",
        "non_finite.gerund",
    ),
];

const SPANISH: &[TenseInfo] = &[
    simple("indicative.present", "presente", "present"),
    simple("indicative.imperfect", "pretérito imperfecto", "imperfect"),
    simple("indicative.preterite", "pretérito indefinido", "preterite"),
    simple("indicative.future", "futuro simple", "future"),
    simple(
        "subjunctive.present",
        "presente de subjuntivo",
        "present subjunctive",
    ),
    simple(
        "subjunctive.imperfect",
        "pretérito imperfecto de subjuntivo",
        "imperfect subjunctive",
    ),
    simple(
        "subjunctive.future",
        "futuro de subjuntivo",
        "future subjunctive",
    ),
    simple("conditional.present", "condicional simple", "conditional"),
    simple("imperative.present", "imperativo", "imperative"),
    simple("non_finite.infinitive", "infinitivo", "infinitive"),
    simple("non_finite.gerund", "gerundio", "gerund"),
    simple(
        "non_finite.past_participle",
        "participio",
        "past participle",
    ),
    compound(
        "indicative.present_perfect",
        "pretérito perfecto compuesto",
        "present perfect",
        "indicative.present",
    ),
    compound(
        "indicative.pluperfect",
        "pretérito pluscuamperfecto",
        "pluperfect",
        "indicative.imperfect",
    ),
    compound(
        "indicative.past_anterior",
        "pretérito anterior",
        "past anterior",
        "indicative.preterite",
    ),
    compound(
        "indicative.future_perfect",
        "futuro compuesto",
        "future perfect",
        "indicative.future",
    ),
    compound(
        "subjunctive.perfect",
        "pretérito perfecto de subjuntivo",
        "perfect subjunctive",
        "subjunctive.present",
    ),
    compound(
        "subjunctive.pluperfect",
        "pretérito pluscuamperfecto de subjuntivo",
        "pluperfect subjunctive",
        "subjunctive.imperfect",
    ),
    compound(
        "subjunctive.future_perfect",
        "futuro compuesto de subjuntivo",
        "future perfect subjunctive",
        "subjunctive.future",
    ),
    compound(
        "conditional.perfect",
        "condicional compuesto",
        "perfect conditional",
        "conditional.present",
    ),
    compound(
        "non_finite.perfect_infinitive",
        "infinitivo compuesto",
        "perfect infinitive",
        "non_finite.infinitive",
    ),
    compound(
        "non_finite.perfect_gerund",
        "gerundio compuesto",
        "perfect gerund",
        "non_finite.gerund",
    ),
];

const FRENCH: &[TenseInfo] = &[
    simple("indicative.present", "présent", "present"),
    simple("indicative.imperfect", "imparfait", "imperfect"),
    simple("indicative.past_historic", "passé simple", "past historic"),
    simple("indicative.future", "futur simple", "future"),
    simple(
        "subjunctive.present",
        "subjonctif présent",
        "present subjunctive",
    ),
    simple(
        "subjunctive.imperfect",
        "subjonctif imparfait",
        "imperfect subjunctive",
    ),
    simple(
        "conditional.present",
        "conditionnel présent",
        "present conditional",
    ),
    simple("imperative.present", "impératif", "imperative"),
    simple("non_finite.infinitive", "infinitif", "infinitive"),
    simple(
        "non_finite.gerund",
        "participe présent",
        "present participle",
    ),
    simple(
        "non_finite.past_participle",
        "participe passé",
        "past participle",
    ),
    compound(
        "indicative.present_perfect",
        "passé composé",
        "present perfect",
        "indicative.present",
    ),
    compound(
        "indicative.pluperfect",
        "plus-que-parfait",
        "pluperfect",
        "indicative.imperfect",
    ),
    compound(
        "indicative.past_anterior",
        "passé antérieur",
        "past anterior",
        "indicative.past_historic",
    ),
    compound(
        "indicative.future_perfect",
        "futur antérieur",
        "future perfect",
        "indicative.future",
    ),
    compound(
        "subjunctive.perfect",
        "subjonctif passé",
        "perfect subjunctive",
        "subjunctive.present",
    ),
    compound(
        "subjunctive.pluperfect",
        "subjonctif plus-que-parfait",
        "pluperfect subjunctive",
        "subjunctive.imperfect",
    ),
    compound(
        "conditional.perfect",
        "conditionnel passé",
        "perfect conditional",
        "conditional.present",
    ),
    compound(
        "non_finite.perfect_infinitive",
        "infinitif passé",
        "perfect infinitive",
        "non_finite.infinitive",
    ),
    compound(
        "non_finite.perfect_gerund",
        "participe passé composé",
        "perfect participle",
        "non_finite.gerund",
    ),
];

/// The tenses of `lang`, simple first. Empty for a language not supported.
pub fn tenses(lang: Lang) -> &'static [TenseInfo] {
    match lang {
        Lang::It => ITALIAN,
        Lang::Es => SPANISH,
        Lang::Fr => FRENCH,
        _ => &[],
    }
}

/// The compound tense built on an auxiliary in `auxiliary_tense`.
pub fn compound_of(lang: Lang, auxiliary_tense: &str) -> Option<&'static TenseInfo> {
    tenses(lang)
        .iter()
        .find(|t| t.auxiliary_tense == Some(auxiliary_tense))
}

/// The verbs that form compound tenses and passives.
pub fn auxiliaries(lang: Lang) -> &'static [&'static str] {
    match lang {
        Lang::It => &["avere", "essere", "venire"],
        Lang::Es => &["haber", "ser"],
        Lang::Fr => &["avoir", "être"],
        _ => &[],
    }
}

/// `indicative.present.3s` → `indicative.present`; `non_finite.gerund` stays.
pub fn tense_of(cell: &str) -> &str {
    match cell.rsplit_once('.') {
        Some((tense, _)) if tense.contains('.') => tense,
        _ => cell,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_language_lists_its_tenses_once() {
        for lang in [Lang::It, Lang::Es, Lang::Fr] {
            let ts = tenses(lang);
            let mut ids: Vec<&str> = ts.iter().map(|t| t.id).collect();
            ids.sort_unstable();
            ids.dedup();
            assert_eq!(ids.len(), ts.len(), "{lang:?}");
            for t in ts.iter().filter(|t| t.kind == Kind::Compound) {
                let aux = t.auxiliary_tense.unwrap();
                assert!(
                    ts.iter().any(|s| s.id == aux && s.kind == Kind::Simple),
                    "{lang:?} {}",
                    t.id
                );
            }
        }
        assert_eq!(
            compound_of(Lang::It, "indicative.past_historic")
                .unwrap()
                .native,
            "trapassato remoto"
        );
        assert_eq!(
            compound_of(Lang::Es, "indicative.present").unwrap().native,
            "pretérito perfecto compuesto"
        );
        assert_eq!(
            compound_of(Lang::Fr, "indicative.present").unwrap().native,
            "passé composé"
        );
        assert_eq!(tense_of("indicative.present.3s"), "indicative.present");
        assert_eq!(tense_of("non_finite.gerund"), "non_finite.gerund");
    }
}
