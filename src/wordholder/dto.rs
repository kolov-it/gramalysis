//! The API's wire format (`/v1/lemmatize:batch`) and its conversion to
//! [`Reading`]s.

use serde::{Deserialize, Serialize};

use crate::morph::{
    Analysis, FormKind, Features, Gender, LexiconError, Mood, Number, PartOfSpeech, Person, Reading, Tense,
};

/// The API's most lemmas per form. A form with this many readings may have
/// more that were cut off.
pub(super) const MAX_READINGS: u8 = 10;

pub(super) fn analyses_from_batch(forms: &[&str], body: BatchResponse) -> Result<Vec<Analysis>, LexiconError> {
    if body.results.len() != forms.len() {
        return Err(LexiconError::Protocol(format!(
            "sent {} forms, got {} results",
            forms.len(),
            body.results.len()
        )));
    }
    let mut slots: Vec<Option<Analysis>> = vec![None; forms.len()];
    for result in body.results {
        let index: usize = result
            .id
            .parse()
            .ok()
            .filter(|i| *i < forms.len())
            .ok_or_else(|| LexiconError::Protocol(format!("unexpected result id {:?}", result.id)))?;
        let readings = result.lemmas.into_iter().map(reading_from_entry).collect();
        slots[index] = Some(Analysis { form: forms[index].to_owned(), readings });
    }
    slots
        .into_iter()
        .enumerate()
        .map(|(i, a)| a.ok_or_else(|| LexiconError::Protocol(format!("no result for form {i}"))))
        .collect()
}

pub(super) fn reading_from_entry(entry: LemmaEntry) -> Reading {
    let (features, clitic, raw_tags) = match entry.grammar {
        Some(g) => (
            Features {
                mood: g.mood,
                tense: g.tense,
                person: g.person,
                number: g.number,
                gender: g.gender,
                form_kind: g.form_kind,
                auxiliaries: g.auxiliaries,
            },
            g.clitic,
            g.raw_tags,
        ),
        // The surface form is the lemma itself and the source has no tags.
        None => (Features::default(), false, Vec::new()),
    };
    Reading {
        lemma: entry.lemma,
        pos: PartOfSpeech::from_tag(&entry.part_of_speech),
        features,
        clitic,
        raw_tags,
        glosses: entry.glosses,
    }
}


#[derive(Serialize)]
pub(super) struct BatchRequest<'a> {
    pub(super) language: &'static str,
    pub(super) items: Vec<BatchItem<'a>>,
    pub(super) limit: u8,
}

#[derive(Serialize)]
pub(super) struct BatchItem<'a> {
    pub(super) id: String,
    pub(super) text: &'a str,
}

#[derive(Deserialize)]
pub(super) struct BatchResponse {
    pub(super) results: Vec<BatchResult>,
}

#[derive(Deserialize)]
pub(super) struct BatchResult {
    pub(super) id: String,
    pub(super) lemmas: Vec<LemmaEntry>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct LemmaEntry {
    pub(super) lemma: String,
    pub(super) part_of_speech: String,
    #[serde(default)]
    pub(super) glosses: Vec<String>,
    pub(super) grammar: Option<Grammar>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Grammar {
    pub(super) raw_tags: Vec<String>,
    pub(super) mood: Option<Mood>,
    pub(super) tense: Option<Tense>,
    pub(super) person: Option<Person>,
    pub(super) number: Option<Number>,
    pub(super) gender: Option<Gender>,
    pub(super) form_kind: Option<FormKind>,
    #[serde(default)]
    pub(super) auxiliaries: Vec<String>,
    pub(super) clitic: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn parse(forms: &[&str], body: serde_json::Value) -> Result<Vec<Analysis>, LexiconError> {
        analyses_from_batch(forms, serde_json::from_value(body).unwrap())
    }

    #[test]
    fn maps_grammar_to_features_in_input_order() {
        let body = json!({"language": "it", "results": [
            {"id": "1", "input": "xyz", "status": "unknown", "known": false, "lemmas": []},
            {"id": "0", "input": "prendendogliela", "status": "analyzed", "known": true, "lemmas": [{
                "surface": "prendendogliela", "lemma": "prendere", "partOfSpeech": "verb",
                "glosses": ["to take"], "confidence": "high", "source": "kaikki",
                "grammar": {"rawTags": ["gerund"], "formKind": "gerund", "clitic": true}
            }]}
        ]});
        let got = parse(&["prendendogliela", "xyz"], body).unwrap();
        assert_eq!(got[0].form, "prendendogliela");
        let r = &got[0].readings[0];
        assert_eq!((r.lemma.as_str(), &r.pos, r.clitic), ("prendere", &PartOfSpeech::Verb, true));
        assert_eq!(r.features.form_kind, Some(FormKind::Gerund));
        assert_eq!(got[1].form, "xyz");
        assert!(!got[1].is_known());
    }

    #[test]
    fn finite_form_features() {
        let body = json!({"language": "it", "results": [{"id": "0", "input": "fece", "status": "analyzed",
            "known": true, "lemmas": [{"surface": "fece", "lemma": "fare", "partOfSpeech": "verb",
            "confidence": "high", "source": "kaikki", "grammar": {
                "rawTags": ["historic", "past", "singular", "third-person"], "mood": "indicative",
                "tense": "past", "person": "third", "number": "singular", "clitic": false}}]}]});
        let f = &parse(&["fece"], body).unwrap()[0].readings[0].features;
        assert_eq!(
            (f.mood, f.tense, f.person, f.number),
            (Some(Mood::Indicative), Some(Tense::Past), Some(Person::Third), Some(Number::Singular))
        );
    }

    #[test]
    fn missing_result_is_an_error() {
        let body = json!({"language": "it", "results": [
            {"id": "0", "input": "a", "status": "unknown", "known": false, "lemmas": []},
            {"id": "0", "input": "a", "status": "unknown", "known": false, "lemmas": []}
        ]});
        assert!(parse(&["a", "b"], body).is_err());
    }

    #[test]
    fn unknown_grammar_value_is_an_error() {
        let body = json!({"language": "it", "results": [{"id": "0", "input": "x", "status": "analyzed",
            "known": true, "lemmas": [{"surface": "x", "lemma": "x", "partOfSpeech": "verb",
            "confidence": "high", "source": "kaikki", "grammar": {"rawTags": [], "mood": "optative", "clitic": false}}]}]});
        assert!(serde_json::from_value::<BatchResponse>(body).is_err());
    }
}
