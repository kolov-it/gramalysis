//! Verb tables (`data/<lang>/verbs.json`) and their cells, named as the
//! tables name them: `indicative.past_historic.3s`, `non_finite.gerund`.

use std::collections::{BTreeMap, BTreeSet, HashSet};

use serde::Deserialize;

use crate::lang::{es, it};
use crate::morph::Lang;
use crate::syntax::feats::FeatMap;
use crate::verbs::base_verb;

/// The contents of a `data/<lang>/verbs.json`.
#[derive(Debug, Deserialize)]
pub struct VerbTables {
    pub verbs: Vec<VerbTable>,
}

#[derive(Debug, Deserialize)]
pub struct VerbTable {
    pub infinitive: String,
    /// mood → tense → forms.
    pub conjugation: BTreeMap<String, BTreeMap<String, TenseForms>>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum TenseForms {
    /// `1s` … `3p` → form.
    Persons(BTreeMap<String, String>),
    /// A non-finite form.
    Single(String),
}

#[derive(Debug, thiserror::Error)]
pub enum QueryError {
    #[error("no verb table for `{0}`")]
    UnknownVerb(String),
    #[error("`{infinitive}` has no `{tense}`; its tenses: {}", available.join(", "))]
    UnknownTense {
        infinitive: String,
        tense: String,
        available: Vec<String>,
    },
}

/// What to look for: a verb, and optionally a tense (`indicative.past_historic`)
/// or a single cell (`indicative.past_historic.3s`, `non_finite.gerund`).
#[derive(Debug, Clone)]
pub struct Query {
    pub infinitive: String,
    pub tense: Option<String>,
}

/// A query resolved against the verb's table.
#[derive(Debug)]
pub struct Target {
    pub lang: Lang,
    pub infinitive: String,
    /// The verb as a parse's lemma is compared with it ([`page_verb`]).
    pub base: String,
    /// The cells asked for, each with its forms (lower case).
    pub cells: BTreeMap<String, Vec<String>>,
    forms: HashSet<String>,
    /// Spanish: the forms without accents, for hosts the clitics put an
    /// accent on (da → dámelo).
    unaccented: HashSet<String>,
}

impl Target {
    pub fn new(tables: &VerbTables, lang: Lang, query: &Query) -> Result<Self, QueryError> {
        let table = tables
            .verbs
            .iter()
            .find(|v| v.infinitive == query.infinitive)
            .ok_or_else(|| QueryError::UnknownVerb(query.infinitive.clone()))?;
        let mut all: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for (mood, tenses) in &table.conjugation {
            for (tense, forms) in tenses {
                match forms {
                    TenseForms::Persons(persons) => {
                        for (person, form) in persons {
                            all.insert(format!("{mood}.{tense}.{person}"), words(form));
                        }
                    }
                    TenseForms::Single(form) => {
                        all.insert(format!("{mood}.{tense}"), words(form));
                    }
                }
            }
        }
        let cells: BTreeMap<String, Vec<String>> = match &query.tense {
            None => all,
            Some(tense) => {
                let prefix = format!("{tense}.");
                let chosen: BTreeMap<String, Vec<String>> = all
                    .iter()
                    .filter(|(c, _)| *c == tense || c.starts_with(&prefix))
                    .map(|(c, f)| (c.clone(), f.clone()))
                    .collect();
                if chosen.is_empty() {
                    let available: BTreeSet<&str> = all.keys().map(|c| tense_of(c)).collect();
                    return Err(QueryError::UnknownTense {
                        infinitive: query.infinitive.clone(),
                        tense: tense.clone(),
                        available: available.into_iter().map(str::to_owned).collect(),
                    });
                }
                chosen
            }
        };
        let forms: HashSet<String> = cells.values().flatten().cloned().collect();
        let unaccented = forms.iter().map(|f| es::strip_accent(f)).collect();
        Ok(Self {
            lang,
            infinitive: query.infinitive.clone(),
            base: page_verb(&query.infinitive, lang),
            cells,
            forms,
            unaccented,
        })
    }

    /// Whether a token (lower case) may be one of the forms: the form itself,
    /// or the form with clitics attached (farlo, dámelo). Only a prefilter;
    /// the parse decides.
    pub fn may_be(&self, token: &str) -> bool {
        if self.forms.contains(token) {
            return true;
        }
        match self.lang {
            Lang::It => it::split_enclitics(token).is_some_and(|(host, _)| {
                // far → fare, por → porre: an infinitive drops its -e before clitics.
                self.forms.contains(&host)
                    || self.forms.contains(&format!("{host}e"))
                    || self.forms.contains(&format!("{host}re"))
            }),
            Lang::Es => es::split_enclitics(token)
                .iter()
                .flat_map(|s| &s.hosts)
                .any(|h| self.unaccented.contains(&es::strip_accent(h))),
            _ => false,
        }
    }
}

impl Target {
    /// Whether a verb morpheme's surface (lower case) is how the table writes
    /// `cell`: the form itself, or the host the clitics left of it (far in
    /// farlo, Dá in Dámelo, Démo in Démonos, senta in sentaos). An inflected
    /// participle (fatti, mangiata) is not: the cell holds the masculine
    /// singular only.
    pub fn written_as(&self, cell: &str, surface: &str) -> bool {
        let Some(forms) = self.cells.get(cell) else {
            return false;
        };
        forms.iter().any(|f| match self.lang {
            Lang::It => {
                *f == surface || *f == format!("{surface}e") || *f == format!("{surface}re")
            }
            Lang::Es => {
                let (f, s) = (es::strip_accent(f), es::strip_accent(surface));
                f == s || f == format!("{s}s") || f == format!("{s}d")
            }
            _ => f == surface,
        })
    }
}

/// `indicative.present.3s` → `indicative.present`; `non_finite.gerund` stays.
fn tense_of(cell: &str) -> &str {
    match cell.rsplit_once('.') {
        Some((tense, _)) if tense.contains('.') => tense,
        _ => cell,
    }
}

/// The verb words of a table cell: `abandonara / abandonase` is two forms,
/// `si fa` (farsi) is `fa`, `nous enfuyons` is `enfuyons`, `s'enfuit` is
/// `enfuit`, `enfuis-toi` is `enfuis`.
fn words(cell: &str) -> Vec<String> {
    cell.split('/')
        .filter_map(|alt| alt.split_whitespace().last())
        .map(|w| w.rsplit('\'').next().unwrap_or(w))
        .map(|w| w.split('-').next().unwrap_or(w))
        .filter(|w| !w.is_empty())
        .map(str::to_lowercase)
        .collect()
}

/// A lemma as a verb page names the verb: its base verb ([`base_verb`]),
/// and in French without the reflexive pronoun (s'enfuir → enfuir).
pub fn page_verb(lemma: &str, lang: Lang) -> String {
    let lemma = if lang == Lang::Fr {
        lemma
            .strip_prefix("se ")
            .or_else(|| lemma.strip_prefix("s'"))
            .unwrap_or(lemma)
    } else {
        lemma
    };
    base_verb(lemma, lang)
}

/// The table cell a verb morpheme's features name, if they name one.
/// Conditional and imperative have one tense, which the lexicon leaves out.
/// French tables have one cell for the -ant form, `gerund`, and the lexicon
/// reads it both as a gerund and as a present participle.
pub fn cell_of(feats: &FeatMap, lang: Lang) -> Option<String> {
    let get = |k: &str| feats.get(k).map(String::as_str);
    let non_finite = match get("form") {
        Some("inf") => Some("infinitive"),
        Some("ger") => Some("gerund"),
        Some("pastpart") => Some("past_participle"),
        Some("prespart") if lang == Lang::Fr => Some("gerund"),
        Some("prespart") => Some("present_participle"),
        _ => None,
    };
    if let Some(form) = non_finite {
        return Some(format!("non_finite.{form}"));
    }
    let (mood, tense) = match get("mood")? {
        "ind" => ("indicative", get("tense")?),
        "sub" => ("subjunctive", get("tense")?),
        "cnd" => ("conditional", "pres"),
        "imp" => ("imperative", "pres"),
        _ => return None,
    };
    let tense = match tense {
        "pres" => "present",
        "impf" => "imperfect",
        "fut" => "future",
        "pret" if lang == Lang::Es => "preterite",
        "pret" => "past_historic",
        _ => return None,
    };
    let number = match get("number")? {
        "sg" => 's',
        "pl" => 'p',
        _ => return None,
    };
    Some(format!("{mood}.{tense}.{}{number}", get("person")?))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tables() -> VerbTables {
        serde_json::from_value(serde_json::json!({"verbs": [
            {"infinitive": "fare", "conjugation": {
                "indicative": {"past_historic": {"3s": "fece", "3p": "fecero"}, "present": {"3s": "fa"}},
                "imperative": {"present": {"2s": "fa"}},
                "non_finite": {"infinitive": "fare", "gerund": "facendo"}}},
            {"infinitive": "farsi", "conjugation": {"indicative": {"present": {"3s": "si fa"}}}}
        ]}))
        .unwrap()
    }

    fn target(lang: Lang, infinitive: &str, tense: Option<&str>) -> Result<Target, QueryError> {
        Target::new(
            &tables(),
            lang,
            &Query {
                infinitive: infinitive.to_owned(),
                tense: tense.map(str::to_owned),
            },
        )
    }

    #[test]
    fn a_tense_selects_its_cells() {
        let t = target(Lang::It, "fare", Some("indicative.past_historic")).unwrap();
        assert_eq!(
            t.cells.keys().collect::<Vec<_>>(),
            ["indicative.past_historic.3p", "indicative.past_historic.3s"]
        );
        let t = target(Lang::It, "fare", Some("non_finite.gerund")).unwrap();
        assert_eq!(t.cells.keys().collect::<Vec<_>>(), ["non_finite.gerund"]);
        let t = target(Lang::It, "fare", None).unwrap();
        assert_eq!(t.cells.len(), 6);
    }

    #[test]
    fn an_unknown_tense_lists_the_tenses() {
        let e = target(Lang::It, "fare", Some("subjunctive.present"))
            .unwrap_err()
            .to_string();
        assert!(
            e.contains("indicative.past_historic, indicative.present"),
            "{e}"
        );
        assert!(e.contains("non_finite.gerund"), "{e}");
        assert!(target(Lang::It, "dare", None).is_err());
    }

    #[test]
    fn prefilter_allows_attached_clitics() {
        let t = target(Lang::It, "fare", None).unwrap();
        for yes in ["fece", "farlo", "fallo", "facendolo", "fa"] {
            assert!(t.may_be(yes), "{yes}");
        }
        for no in ["fame", "fatto", "ferro"] {
            assert!(!t.may_be(no), "{no}");
        }
        assert_eq!(
            target(Lang::It, "farsi", None).unwrap().cells["indicative.present.3s"],
            ["fa"]
        );
    }

    #[test]
    fn cell_names_from_features() {
        let feats = |s: &str| -> FeatMap {
            s.split(',')
                .map(|kv| kv.split_once('=').unwrap())
                .map(|(k, v)| (k.to_owned(), v.to_owned()))
                .collect()
        };
        assert_eq!(
            cell_of(&feats("mood=ind,tense=pret,person=3,number=sg"), Lang::It).unwrap(),
            "indicative.past_historic.3s"
        );
        assert_eq!(
            cell_of(&feats("mood=ind,tense=pret,person=1,number=pl"), Lang::Es).unwrap(),
            "indicative.preterite.1p"
        );
        assert_eq!(
            cell_of(&feats("mood=cnd,person=1,number=sg"), Lang::Es).unwrap(),
            "conditional.present.1s"
        );
        assert_eq!(
            cell_of(&feats("mood=imp,person=2,number=sg"), Lang::It).unwrap(),
            "imperative.present.2s"
        );
        assert_eq!(
            cell_of(
                &feats("form=pastpart,gender=f,number=sg,finite=no"),
                Lang::It
            )
            .unwrap(),
            "non_finite.past_participle"
        );
        assert_eq!(
            cell_of(&feats("form=prespart"), Lang::It).unwrap(),
            "non_finite.present_participle"
        );
        assert_eq!(
            cell_of(&feats("form=prespart"), Lang::Fr).unwrap(),
            "non_finite.gerund"
        );
        assert_eq!(
            cell_of(&feats("mood=ind,person=3,number=sg"), Lang::It),
            None
        );
        assert_eq!(page_verb("s'enfuir", Lang::Fr), "enfuir");
        assert_eq!(page_verb("mettersi", Lang::It), "mettere");
    }
}
