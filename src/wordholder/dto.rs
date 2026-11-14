//! Word Holder's lemmatize contract: the wire format of
//! `/v1/lemmatize:batch` and its conversion to [`Reading`]s.

use serde::{Deserialize, Serialize};

use crate::morph::{
    Analysis, FormKind, Features, Gender, Lang, LexiconError, Mood, Number, PartOfSpeech, Person, Reading, Tense,
};

/// The API's most lemmas per form, and the `limit` [`BatchRequest::new`]
/// asks for. A form with this many readings may have more that were cut off.
pub const MAX_READINGS: u8 = 10;

/// Converts the answer to a [`BatchRequest`] built by [`BatchRequest::new`]
/// to one [`Analysis`] per form, in the order of `forms`. Result ids are the
/// forms' indexes; a missing, repeated or unknown id is an error, as is a
/// result count that differs from the form count.
pub fn analyses_from_batch(forms: &[&str], body: BatchResponse) -> Result<Vec<Analysis>, LexiconError> {
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

/// One lemma entry as a [`Reading`]: what the conversion and
/// [`FixtureLexicon`](super::FixtureLexicon) do for each entry.
pub fn reading_from_entry(entry: LemmaEntry) -> Reading {
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

/// The body of a `/v1/lemmatize:batch` request.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[non_exhaustive]
pub struct BatchRequest<'a> {
    pub language: &'static str,
    pub items: Vec<BatchItem<'a>>,
    /// Most lemmas per form.
    pub limit: u8,
}

impl<'a> BatchRequest<'a> {
    /// A request for `forms`, with ids `"0"`, `"1"`, … (their indexes, which
    /// [`analyses_from_batch`] expects back) and `limit` [`MAX_READINGS`].
    pub fn new(lang: Lang, forms: &[&'a str]) -> Self {
        Self {
            language: lang.code(),
            items: forms.iter().enumerate().map(|(i, text)| BatchItem { id: i.to_string(), text }).collect(),
            limit: MAX_READINGS,
        }
    }
}

/// One form to lemmatize.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[non_exhaustive]
pub struct BatchItem<'a> {
    pub id: String,
    pub text: &'a str,
}

/// The body of a `/v1/lemmatize:batch` answer. Fields the conversion does not
/// read (`language`, and per result `input`, `status`, `known`) are ignored.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[non_exhaustive]
pub struct BatchResponse {
    pub results: Vec<BatchResult>,
}

impl BatchResponse {
    pub fn new(results: Vec<BatchResult>) -> Self {
        Self { results }
    }
}

/// The answer for one [`BatchItem`]: every lemma the form can belong to,
/// none for an unknown form.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[non_exhaustive]
pub struct BatchResult {
    /// The [`BatchItem::id`] it answers.
    pub id: String,
    pub lemmas: Vec<LemmaEntry>,
}

impl BatchResult {
    pub fn new(id: String, lemmas: Vec<LemmaEntry>) -> Self {
        Self { id, lemmas }
    }
}

/// One reading of a form: its lemma, part of speech and, unless the form is
/// the lemma itself and the source has no tags, its grammar.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct LemmaEntry {
    pub lemma: String,
    /// The source's tag, as [`PartOfSpeech::from_tag`] reads it (`verb`,
    /// `noun`, …).
    pub part_of_speech: String,
    #[serde(default)]
    pub glosses: Vec<String>,
    pub grammar: Option<Grammar>,
}

impl LemmaEntry {
    /// An entry without glosses or grammar; set them on the result.
    pub fn new(lemma: String, part_of_speech: String) -> Self {
        Self { lemma, part_of_speech, glosses: Vec::new(), grammar: None }
    }
}

/// The form's features, as the API names them.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct Grammar {
    /// The source's tags, unparsed.
    pub raw_tags: Vec<String>,
    pub mood: Option<Mood>,
    pub tense: Option<Tense>,
    pub person: Option<Person>,
    pub number: Option<Number>,
    pub gender: Option<Gender>,
    pub form_kind: Option<FormKind>,
    #[serde(default)]
    pub auxiliaries: Vec<String>,
    /// The form carries attached clitic pronouns (`prendendogliela`).
    pub clitic: bool,
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
    fn request_ids_are_indexes() {
        let request = BatchRequest::new(Lang::Es, &["a", "b"]);
        assert_eq!(
            serde_json::to_value(&request).unwrap(),
            json!({"language": "es", "items": [{"id": "0", "text": "a"}, {"id": "1", "text": "b"}], "limit": 10})
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
