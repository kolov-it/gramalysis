//! Patches for a lexicon's known defects. Every lexicon has some: a reading
//! it lacks (scorsero has no scorrere), a lemma it names wrongly (aver for
//! avere). A [`PatchedLexicon`] wraps any [`Lexicon`] and applies a patch file
//! to its answers, so corrections are visible data, one line each with its
//! reason, not code inside the parser.
//!
//! Patch file syntax, one operation per line (`#` starts a comment):
//!
//! ```text
//! scorsero + scorrere verb mood=indicative,tense=preterite,person=third,number=plural
//! aver     = avere verb form=infinitive
//! dei      - dio noun
//! ```
//!
//! - `+` adds a reading the lexicon lacks (skipped if it is already there);
//! - `=` replaces all the lexicon's readings of the form with the listed ones
//!   (several `=` lines for one form give several readings);
//! - `-` removes the lexicon's readings with that lemma (and part of speech,
//!   if given).
//!
//! Features use the names of [`Features`]: mood, tense, person, number,
//! gender, form (gerund, infinitive, participle, past-participle,
//! present-participle), aux (avere|essere), and `clitic=yes`.

use std::collections::HashMap;

use async_trait::async_trait;

use crate::morph::{
    Analysis, FormKind, Features, Gender, Lang, Lexicon, LexiconError, Mood, Number, PartOfSpeech, Person, Reading,
    Tense,
};

#[derive(Debug, Clone, PartialEq)]
enum Op {
    Add(Reading),
    Replace(Reading),
    Remove { lemma: String, pos: Option<PartOfSpeech> },
}

#[derive(Debug, Default)]
pub struct Patches {
    by_form: HashMap<(Lang, String), Vec<Op>>,
}

#[derive(Debug, thiserror::Error)]
#[error("{source_name} line {line}: {message}")]
pub struct PatchError {
    pub source_name: String,
    pub line: usize,
    pub message: String,
}

impl Patches {
    /// Reads one patch file for `lang`. `source_name` only labels errors.
    pub fn parse(lang: Lang, source_name: &str, text: &str) -> Result<Patches, PatchError> {
        let mut patches = Patches::default();
        patches.add_file(lang, source_name, text)?;
        Ok(patches)
    }

    pub fn add_file(&mut self, lang: Lang, source_name: &str, text: &str) -> Result<(), PatchError> {
        for (i, raw) in text.lines().enumerate() {
            let err = |message: String| PatchError { source_name: source_name.to_owned(), line: i + 1, message };
            let line = raw.split('#').next().unwrap_or("").trim();
            if line.is_empty() {
                continue;
            }
            let mut parts = line.split_whitespace();
            let (Some(form), Some(op), Some(lemma)) = (parts.next(), parts.next(), parts.next()) else {
                return Err(err("expected `form op lemma [pos [features]]`".to_owned()));
            };
            let pos = parts.next().map(PartOfSpeech::from_tag);
            let features = parts.next();
            if parts.next().is_some() {
                return Err(err("too many fields (features are one comma-separated field)".to_owned()));
            }
            let reading = |pos: Option<PartOfSpeech>| -> Result<Reading, PatchError> {
                let pos = pos.ok_or_else(|| err(format!("`{op}` needs a part of speech")))?;
                let (features, clitic) = parse_features(features.unwrap_or("")).map_err(&err)?;
                Ok(Reading { lemma: lemma.to_owned(), pos, features, clitic, raw_tags: Vec::new(), glosses: Vec::new() })
            };
            let op = match op {
                "+" => Op::Add(reading(pos)?),
                "=" => Op::Replace(reading(pos)?),
                "-" => {
                    if features.is_some() {
                        return Err(err("`-` takes a lemma and an optional part of speech, no features".to_owned()));
                    }
                    Op::Remove { lemma: lemma.to_owned(), pos }
                }
                other => return Err(err(format!("unknown operation `{other}`; use +, = or -"))),
            };
            self.by_form.entry((lang, form.to_lowercase())).or_default().push(op);
        }
        Ok(())
    }

    pub fn is_empty(&self) -> bool {
        self.by_form.is_empty()
    }

    /// Applies the patches for `analysis.form`.
    pub fn apply(&self, lang: Lang, analysis: &mut Analysis) {
        let Some(ops) = self.by_form.get(&(lang, analysis.form.to_lowercase())) else { return };
        if ops.iter().any(|op| matches!(op, Op::Replace(_))) {
            analysis.readings.clear();
        }
        for op in ops {
            match op {
                Op::Add(r) | Op::Replace(r) => {
                    let present = analysis
                        .readings
                        .iter()
                        .any(|x| x.lemma == r.lemma && x.pos == r.pos && x.features == r.features && x.clitic == r.clitic);
                    if !present {
                        analysis.readings.push(r.clone());
                    }
                }
                Op::Remove { lemma, pos } => {
                    analysis.readings.retain(|x| !(x.lemma == *lemma && pos.as_ref().is_none_or(|p| *p == x.pos)));
                }
            }
        }
    }
}

fn parse_features(text: &str) -> Result<(Features, bool), String> {
    let mut f = Features::default();
    let mut clitic = false;
    for pair in text.split(',').filter(|p| !p.is_empty()) {
        let (key, value) = pair.split_once('=').ok_or_else(|| format!("expected `name=value`, got `{pair}`"))?;
        let bad = || format!("unknown {key} `{value}`");
        match key {
            "mood" => {
                f.mood = Some(match value {
                    "indicative" => Mood::Indicative,
                    "subjunctive" => Mood::Subjunctive,
                    "conditional" => Mood::Conditional,
                    "imperative" => Mood::Imperative,
                    _ => return Err(bad()),
                })
            }
            "tense" => {
                f.tense = Some(match value {
                    "present" => Tense::Present,
                    "past" => Tense::Past,
                    "future" => Tense::Future,
                    "imperfect" => Tense::Imperfect,
                    "preterite" => Tense::Preterite,
                    "pluperfect" => Tense::Pluperfect,
                    "perfect" => Tense::Perfect,
                    _ => return Err(bad()),
                })
            }
            "person" => {
                f.person = Some(match value {
                    "first" => Person::First,
                    "second" => Person::Second,
                    "third" => Person::Third,
                    _ => return Err(bad()),
                })
            }
            "number" => {
                f.number = Some(match value {
                    "singular" => Number::Singular,
                    "plural" => Number::Plural,
                    _ => return Err(bad()),
                })
            }
            "gender" => {
                f.gender = Some(match value {
                    "masculine" => Gender::Masculine,
                    "feminine" => Gender::Feminine,
                    "neuter" => Gender::Neuter,
                    _ => return Err(bad()),
                })
            }
            "form" => {
                f.form_kind = Some(match value {
                    "gerund" => FormKind::Gerund,
                    "infinitive" => FormKind::Infinitive,
                    "participle" => FormKind::Participle,
                    "past-participle" => FormKind::PastParticiple,
                    "present-participle" => FormKind::PresentParticiple,
                    _ => return Err(bad()),
                })
            }
            "aux" => f.auxiliaries = value.split('|').map(str::to_owned).collect(),
            "clitic" => {
                clitic = match value {
                    "yes" => true,
                    "no" => false,
                    _ => return Err(bad()),
                }
            }
            _ => return Err(format!("unknown feature `{key}`")),
        }
    }
    Ok((f, clitic))
}

/// A lexicon with patches applied to every answer.
pub struct PatchedLexicon {
    inner: Box<dyn Lexicon>,
    patches: Patches,
}

impl PatchedLexicon {
    pub fn new(inner: Box<dyn Lexicon>, patches: Patches) -> Self {
        Self { inner, patches }
    }
}

#[async_trait]
impl Lexicon for PatchedLexicon {
    async fn analyze(&self, lang: Lang, forms: &[&str]) -> Result<Vec<Analysis>, LexiconError> {
        let mut out = self.inner.analyze(lang, forms).await?;
        for a in &mut out {
            self.patches.apply(lang, a);
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn analysis(form: &str, readings: Vec<Reading>) -> Analysis {
        Analysis { form: form.to_owned(), readings }
    }

    fn reading(lemma: &str, pos: PartOfSpeech) -> Reading {
        Reading { lemma: lemma.to_owned(), pos, features: Features::default(), clitic: false, raw_tags: vec![], glosses: vec![] }
    }

    #[test]
    fn adds_replaces_and_removes() {
        let p = Patches::parse(
            Lang::It,
            "test",
            "scorsero + scorrere verb mood=indicative,tense=preterite,person=third,number=plural  # missing\n\
             aver = avere verb form=infinitive\n\
             dei - dio noun\n",
        )
        .unwrap();

        let mut a = analysis("scorsero", vec![reading("scorgere", PartOfSpeech::Verb)]);
        p.apply(Lang::It, &mut a);
        assert_eq!(a.readings.len(), 2);
        assert_eq!(a.readings[1].features.tense, Some(Tense::Preterite));
        p.apply(Lang::It, &mut a);
        assert_eq!(a.readings.len(), 2, "adding twice keeps one");

        let mut a = analysis("aver", vec![reading("aver", PartOfSpeech::Verb)]);
        p.apply(Lang::It, &mut a);
        assert_eq!(a.readings.len(), 1);
        assert_eq!(a.readings[0].lemma, "avere");
        assert_eq!(a.readings[0].features.form_kind, Some(FormKind::Infinitive));

        let mut a = analysis("dei", vec![reading("dio", PartOfSpeech::Noun), reading("di", PartOfSpeech::Preposition)]);
        p.apply(Lang::It, &mut a);
        assert_eq!(a.readings.iter().map(|r| r.lemma.as_str()).collect::<Vec<_>>(), ["di"]);
    }

    #[test]
    fn rejects_bad_lines_with_their_number() {
        let e = Patches::parse(Lang::It, "test", "ok + ok verb\nbad ? x verb\n").unwrap_err();
        assert_eq!(e.line, 2);
        assert!(Patches::parse(Lang::It, "test", "x + y verb mood=optative\n").is_err());
    }
}
