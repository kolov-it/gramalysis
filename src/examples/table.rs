//! Verb tables (`data/<lang>/verbs.json`) and their cells, named as the
//! tables name them (`indicative.past_historic.3s`, `non_finite.gerund`),
//! plus the compound tenses the tables leave out ([`super::tenses`]).

use std::collections::{BTreeMap, HashSet};

use serde::Deserialize;

use super::tenses::{Kind, auxiliaries, tense_of, tenses};
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
    #[error("`{tense}` is not a tense of this language; the tenses: {}", available.join(", "))]
    UnknownTense {
        tense: String,
        available: Vec<String>,
    },
    #[error("the table of `{infinitive}` has no `{tense}`")]
    NotInTable { infinitive: String, tense: String },
}

/// What to look for: a verb, and optionally a tense (`indicative.past_historic`,
/// `indicative.present_perfect`) or a single cell (`indicative.past_historic.3s`,
/// `non_finite.gerund`).
#[derive(Debug, Clone)]
pub struct Query {
    pub infinitive: String,
    pub tense: Option<String>,
}

/// Checks a `--tense` value: a tense of `lang` ([`tenses`]), or one of its
/// cells (the tense and a person, `indicative.present.3s`).
pub fn check_tense(lang: Lang, tense: &str) -> Result<(), QueryError> {
    let known = tenses(lang)
        .iter()
        .any(|t| t.id == tense || t.id == tense_of(tense));
    if known {
        return Ok(());
    }
    Err(QueryError::UnknownTense {
        tense: tense.to_owned(),
        available: tenses(lang).iter().map(|t| t.id.to_owned()).collect(),
    })
}

/// Word forms to match tokens against, clitics attached allowed.
#[derive(Debug, Default)]
struct Forms {
    forms: HashSet<String>,
    /// Spanish: the forms without accents, for hosts the clitics put an
    /// accent on (da → dámelo).
    unaccented: HashSet<String>,
}

impl Forms {
    /// A form that carries clitics itself (arrepintiéndose, farsi) also
    /// counts by its host, so the same verb with other clitics matches
    /// (arrepintiéndome, farmi).
    fn new(lang: Lang, forms: HashSet<String>) -> Self {
        let mut forms = forms;
        let mut unaccented: HashSet<String> = forms.iter().map(|f| es::strip_accent(f)).collect();
        match lang {
            Lang::It => {
                let hosts: Vec<String> = forms.iter().filter_map(|f| it::split_enclitics(f)).map(|(h, _)| h).collect();
                forms.extend(hosts);
            }
            Lang::Es => {
                for f in forms.iter() {
                    for split in es::split_enclitics(f) {
                        unaccented.extend(split.hosts.iter().map(|h| es::strip_accent(h)));
                    }
                }
            }
            _ => {}
        }
        Self { forms, unaccented }
    }

    /// Whether a token (lower case) may be one of the forms: the form itself,
    /// or the form with clitics attached (farlo, dammelo, dámelo).
    fn may_be(&self, lang: Lang, token: &str) -> bool {
        if self.forms.contains(token) {
            return true;
        }
        // Italian drops an infinitive's -e at will (far così, aver detto).
        if lang == Lang::It
            && (self.forms.contains(&format!("{token}e"))
                || self.forms.contains(&format!("{token}re")))
        {
            return true;
        }
        match lang {
            Lang::It => it::split_enclitics(token).is_some_and(|(host, _)| {
                it_host_spellings(&host)
                    .iter()
                    .any(|f| self.forms.contains(f))
            }),
            Lang::Es => es::split_enclitics(token)
                .iter()
                .flat_map(|s| &s.hosts)
                .any(|h| self.unaccented.contains(&es::strip_accent(h))),
            // An imperative takes an -s before y and en (vas-y, donnes-en).
            Lang::Fr => token
                .strip_suffix('s')
                .is_some_and(|t| self.forms.contains(t)),
            _ => false,
        }
    }
}

/// How an Italian table may write the host clitics were split from: far →
/// fare, por → porre, and the short imperatives da → dai, da', di → di'.
fn it_host_spellings(host: &str) -> Vec<String> {
    let mut out = vec![host.to_owned(), format!("{host}e"), format!("{host}re")];
    if ["da", "fa", "sta", "va", "di"].contains(&host) {
        out.push(format!("{host}i"));
        out.push(format!("{host}'"));
    }
    out
}

/// A query resolved against the verb's table.
#[derive(Debug)]
pub struct Target {
    pub lang: Lang,
    pub infinitive: String,
    /// The verb as a parse's lemma is compared with it ([`page_verb`]).
    pub base: String,
    /// The cells asked for. A simple cell has its forms (lower case); a
    /// compound one has none (it is the auxiliary + the participle).
    pub cells: BTreeMap<String, Vec<String>>,
    simple: Forms,
    /// The past participle in every gender and number, for the prefilter;
    /// in a parse, the lexicon's reading decides.
    participles: HashSet<String>,
    /// Every form of the auxiliaries.
    auxiliary: Forms,
}

impl Target {
    pub fn new(tables: &VerbTables, lang: Lang, query: &Query) -> Result<Self, QueryError> {
        if let Some(tense) = &query.tense {
            check_tense(lang, tense)?;
        }
        let table = find(tables, &query.infinitive)?;
        let mut all = table_cells(table);
        // A pronominal verb's table writes the participle with its clitic
        // (fattosi); in a compound tense it stands alone (si è fatto).
        let pronominal = base_verb(&query.infinitive, lang) != query.infinitive;
        let participles: HashSet<String> = all
            .get("non_finite.past_participle")
            .into_iter()
            .flatten()
            .map(|p| match it::split_enclitics(p) {
                Some((host, _)) if lang == Lang::It && pronominal => host,
                _ => p.clone(),
            })
            .flat_map(|p| inflected_participles(lang, &p))
            .collect();
        let mut auxiliary = HashSet::new();
        for aux in auxiliaries(lang) {
            auxiliary.extend(table_cells(find(tables, aux)?).into_values().flatten());
            // Italian drops the infinitive's -e at will: aver commesso,
            // esser partito.
            if lang == Lang::It {
                auxiliary.insert(aux.strip_suffix('e').unwrap_or(aux).to_owned());
            }
        }
        // A compound tense has the persons of its auxiliary's tense.
        let aux_cells = table_cells(find(tables, auxiliaries(lang)[0])?);
        for t in tenses(lang).iter().filter(|t| t.kind == Kind::Compound) {
            let aux = t.auxiliary_tense.unwrap_or_default();
            for cell in aux_cells.keys().filter(|c| tense_of(c) == aux) {
                let person = &cell[aux.len()..];
                all.insert(format!("{}{person}", t.id), Vec::new());
            }
        }
        let cells: BTreeMap<String, Vec<String>> = match &query.tense {
            None => all,
            Some(tense) => {
                let chosen: BTreeMap<String, Vec<String>> = all
                    .into_iter()
                    .filter(|(c, _)| c == tense || tense_of(c) == tense)
                    .collect();
                if chosen.is_empty() {
                    return Err(QueryError::NotInTable {
                        infinitive: query.infinitive.clone(),
                        tense: tense.clone(),
                    });
                }
                chosen
            }
        };
        Ok(Self {
            lang,
            infinitive: query.infinitive.clone(),
            base: page_verb(&query.infinitive, lang),
            simple: Forms::new(lang, cells.values().flatten().cloned().collect()),
            cells,
            participles,
            auxiliary: Forms::new(lang, auxiliary),
        })
    }

    /// Whether a token (lower case) may be one of the simple forms asked
    /// for. Only a prefilter; the parse decides.
    pub fn may_be(&self, token: &str) -> bool {
        self.simple.may_be(self.lang, token)
    }

    /// Whether a token (lower case) may be the verb's past participle, in any
    /// gender and number: the heart of a compound tense or a passive.
    pub fn may_be_participle(&self, token: &str) -> bool {
        self.participles.contains(token)
    }

    /// Whether a token (lower case) may be a form of an auxiliary.
    pub fn may_be_auxiliary(&self, token: &str) -> bool {
        self.auxiliary.may_be(self.lang, token)
    }

    /// Whether a verb morpheme's surface (lower case) is how the table writes
    /// `cell`. `token` (lower case) is given when clitics were split off the
    /// word: then the whole word may be the table's spelling (fatti,
    /// arrepintámonos), or the host what the clitics left of it (far in
    /// farlo, da in dammelo, dá in dámelo, démo in démonos). An inflected
    /// participle (fatti, mangiata) is not the `past_participle` cell, which
    /// holds the masculine singular only.
    pub fn written_as(&self, cell: &str, surface: &str, token: Option<&str>) -> bool {
        let Some(forms) = self.cells.get(cell) else {
            return false;
        };
        forms.iter().any(|f| match (self.lang, token) {
            _ if f == surface => true,
            (_, Some(t)) if f == t => true,
            (Lang::It, Some(_)) => {
                it_host_spellings(surface).contains(f)
                    || it::split_enclitics(f).is_some_and(|(h, _)| h == surface)
            }
            (Lang::Es, Some(_)) => {
                let (f, s) = (es::strip_accent(f), es::strip_accent(surface));
                f == s
                    || f == format!("{s}s")
                    || f == format!("{s}d")
                    || es::split_enclitics(&f).iter().flat_map(|split| &split.hosts).any(|h| es::strip_accent(h) == s)
            }
            // far così: the infinitive without its -e (porre: por).
            (Lang::It, None) => {
                cell == "non_finite.infinitive"
                    && (*f == format!("{surface}e") || *f == format!("{surface}re"))
            }
            // vas-y, donnes-en: the imperative's liaison -s.
            (Lang::Fr, _) => cell.starts_with("imperative.") && surface == format!("{f}s"),
            _ => false,
        })
    }
}

fn find<'t>(tables: &'t VerbTables, infinitive: &str) -> Result<&'t VerbTable, QueryError> {
    tables
        .verbs
        .iter()
        .find(|v| v.infinitive == infinitive)
        .ok_or_else(|| QueryError::UnknownVerb(infinitive.to_owned()))
}

/// Cell → the verb words written in it.
fn table_cells(table: &VerbTable) -> BTreeMap<String, Vec<String>> {
    let mut all = BTreeMap::new();
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
    all
}

/// A past participle in every gender and number, from the masculine
/// singular the table gives: fatto → fatta, fatti, fatte; hecho → hecha,
/// hechos, hechas; vu → vue, vus, vues; pris → prise, prises.
fn inflected_participles(lang: Lang, ms: &str) -> Vec<String> {
    let mut out = vec![ms.to_owned()];
    match lang {
        Lang::It => {
            if let Some(stem) = ms.strip_suffix('o') {
                out.extend(["a", "i", "e"].iter().map(|e| format!("{stem}{e}")));
            }
        }
        Lang::Es => {
            if let Some(stem) = ms.strip_suffix('o') {
                out.extend(["a", "os", "as"].iter().map(|e| format!("{stem}{e}")));
            }
        }
        Lang::Fr => {
            // dû, mû, crû lose the circumflex when they agree (due, dus);
            // absous, dissous take a t (absoute).
            let base = match ms.strip_suffix('û') {
                Some(stem) => format!("{stem}u"),
                None => ms.to_owned(),
            };
            let feminine = match base.strip_suffix("ous") {
                Some(stem) => format!("{stem}oute"),
                None => format!("{base}e"),
            };
            out.push(format!("{feminine}s"));
            out.push(feminine);
            if !base.ends_with(['s', 'x']) {
                out.push(format!("{base}s"));
            }
        }
        _ => {}
    }
    out
}

/// The verb words of a table cell: `abandonara / abandonase` is two forms,
/// `si fa` (farsi) is `fa`, `nous enfuyons` is `enfuyons`, `s'enfuit` is
/// `enfuit`, `enfuis-toi` is `enfuis`.
fn words(cell: &str) -> Vec<String> {
    cell.split('/')
        .filter_map(|alt| alt.split_whitespace().last())
        // An elided pronoun before the verb goes (s'enfuit); an apostrophe
        // that ends the verb stays (di', fa').
        .map(|w| match w.rsplit_once('\'') {
            Some((_, verb)) if !verb.is_empty() => verb,
            _ => w,
        })
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
        let persons =
            |f: &str| serde_json::json!({"1s": f, "2s": f, "3s": f, "1p": f, "2p": f, "3p": f});
        serde_json::from_value(serde_json::json!({"verbs": [
            {"infinitive": "fare", "conjugation": {
                "indicative": {"past_historic": {"3s": "fece", "3p": "fecero"}, "present": {"3s": "fa"}},
                "imperative": {"present": {"2s": "fai"}},
                "non_finite": {"infinitive": "fare", "gerund": "facendo", "past_participle": "fatto"}}},
            {"infinitive": "farsi", "conjugation": {"indicative": {"present": {"3s": "si fa"}},
                "imperative": {"present": {"2s": "fatti"}}, "non_finite": {"infinitive": "farsi"}}},
            {"infinitive": "dire", "conjugation": {"imperative": {"present": {"2s": "di'"}}}},
            {"infinitive": "avere", "conjugation": {"indicative": {"present": persons("ha")},
                "non_finite": {"infinitive": "avere", "gerund": "avendo"}}},
            {"infinitive": "essere", "conjugation": {"indicative": {"present": persons("è")}}},
            {"infinitive": "venire", "conjugation": {"indicative": {"present": persons("viene")}}}
        ]}))
        .unwrap()
    }

    fn target(infinitive: &str, tense: Option<&str>) -> Result<Target, QueryError> {
        Target::new(
            &tables(),
            Lang::It,
            &Query {
                infinitive: infinitive.to_owned(),
                tense: tense.map(str::to_owned),
            },
        )
    }

    #[test]
    fn a_tense_selects_its_cells() {
        let t = target("fare", Some("indicative.past_historic")).unwrap();
        assert_eq!(
            t.cells.keys().collect::<Vec<_>>(),
            ["indicative.past_historic.3p", "indicative.past_historic.3s"]
        );
        let t = target("fare", Some("non_finite.gerund")).unwrap();
        assert_eq!(t.cells.keys().collect::<Vec<_>>(), ["non_finite.gerund"]);
        let t = target("fare", Some("indicative.present_perfect")).unwrap();
        assert_eq!(t.cells.len(), 6);
        assert!(t.cells.contains_key("indicative.present_perfect.3s"));
        let t = target("fare", Some("non_finite.perfect_infinitive")).unwrap();
        assert_eq!(
            t.cells.keys().collect::<Vec<_>>(),
            ["non_finite.perfect_infinitive"]
        );
        let t = target("fare", Some("indicative.past_historic.3s")).unwrap();
        assert_eq!(
            t.cells.keys().collect::<Vec<_>>(),
            ["indicative.past_historic.3s"]
        );
    }

    #[test]
    fn an_unknown_tense_lists_the_tenses() {
        let e = target("fare", Some("indicative.passato_prossimo"))
            .unwrap_err()
            .to_string();
        assert!(e.contains("indicative.present_perfect"), "{e}");
        assert!(e.contains("non_finite.gerund"), "{e}");
        assert!(matches!(
            target("fare", Some("subjunctive.present")),
            Err(QueryError::NotInTable { .. })
        ));
        assert!(matches!(
            target("dare", None),
            Err(QueryError::UnknownVerb(_))
        ));
    }

    #[test]
    fn prefilter_allows_attached_clitics_and_inflected_participles() {
        let t = target("fare", None).unwrap();
        for yes in ["fece", "farlo", "fallo", "facendolo", "fa"] {
            assert!(t.may_be(yes), "{yes}");
        }
        for no in ["fame", "ferro"] {
            assert!(!t.may_be(no), "{no}");
        }
        for p in ["fatto", "fatta", "fatti", "fatte"] {
            assert!(t.may_be_participle(p), "{p}");
        }
        assert!(
            t.may_be_auxiliary("ha")
                && t.may_be_auxiliary("avendolo")
                && !t.may_be_auxiliary("casa")
        );
        assert!(t.may_be_auxiliary("aver") && t.may_be_auxiliary("esser"));
    }

    #[test]
    fn hosts_and_clitic_bearing_cells() {
        let t = target("fare", None).unwrap();
        assert!(t.written_as("non_finite.infinitive", "far", Some("farlo")));
        // Bare far is the infinitive too (far così), but no other cell.
        assert!(t.written_as("non_finite.infinitive", "far", None));
        assert!(!t.written_as("indicative.present.3s", "f", None));
        assert!(t.may_be("far") && !t.may_be("fa'r"));
        assert!(t.written_as("imperative.present.2s", "fa", Some("fallo")));
        let farsi = target("farsi", None).unwrap();
        assert!(farsi.written_as("imperative.present.2s", "fa", Some("fatti")));
        assert!(farsi.written_as("non_finite.infinitive", "far", Some("farsi")));
        assert!(!farsi.written_as("imperative.present.2s", "fa", None));
        assert_eq!(
            target("dire", None).unwrap().cells["imperative.present.2s"],
            ["di'"]
        );
        assert!(target("dire", None).unwrap().written_as(
            "imperative.present.2s",
            "di",
            Some("dillo")
        ));
        assert_eq!(
            target("farsi", None).unwrap().cells["indicative.present.3s"],
            ["fa"]
        );
    }

    #[test]
    fn participles_in_every_gender_and_number() {
        assert_eq!(
            inflected_participles(Lang::Es, "hecho"),
            ["hecho", "hecha", "hechos", "hechas"]
        );
        assert_eq!(
            inflected_participles(Lang::Fr, "pris"),
            ["pris", "prises", "prise"]
        );
        assert_eq!(
            inflected_participles(Lang::Fr, "vu"),
            ["vu", "vues", "vue", "vus"]
        );
        assert_eq!(
            inflected_participles(Lang::Fr, "dû"),
            ["dû", "dues", "due", "dus"]
        );
        assert_eq!(
            inflected_participles(Lang::Fr, "absous"),
            ["absous", "absoutes", "absoute"]
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
