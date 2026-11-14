#![cfg(feature = "wordholder")]
//! The lemmatize contract as a library user sees it: a response built in
//! process, without JSON, converted and patched.

use async_trait::async_trait;

use syntango::morph::{Analysis, Lang, Lexicon, LexiconError, Mood, PartOfSpeech, Person};
use syntango::patch::PatchedLexicon;
use syntango::wordholder::{self, analyses_from_batch, BatchRequest, BatchResponse, BatchResult, Grammar, LemmaEntry};

#[test]
fn response_built_in_process_converts() {
    let forms = ["fece", "xyz"];
    let request = BatchRequest::new(Lang::It, &forms);
    let mut grammar = Grammar::default();
    grammar.mood = Some(Mood::Indicative);
    grammar.person = Some(Person::Third);
    let mut entry = LemmaEntry::new("fare".to_owned(), "verb".to_owned());
    entry.grammar = Some(grammar);
    let response = BatchResponse::new(vec![
        BatchResult::new(request.items[1].id.clone(), Vec::new()),
        BatchResult::new(request.items[0].id.clone(), vec![entry]),
    ]);
    let got = analyses_from_batch(&forms, response).unwrap();
    let r = &got[0].readings[0];
    assert_eq!((r.lemma.as_str(), &r.pos, r.features.mood), ("fare", &PartOfSpeech::Verb, Some(Mood::Indicative)));
    assert!(!got[1].is_known());
}

/// `aver` is in data/it/lexicon-patches/wordholder.txt as `avere verb
/// form=infinitive`; the patches apply to readings a converted response holds.
#[tokio::test]
async fn patches_apply_to_converted_readings() {
    struct InProcess;
    #[async_trait]
    impl Lexicon for InProcess {
        async fn analyze(&self, _lang: Lang, forms: &[&str]) -> Result<Vec<Analysis>, LexiconError> {
            let results = (0..forms.len()).map(|i| BatchResult::new(i.to_string(), Vec::new())).collect();
            analyses_from_batch(forms, BatchResponse::new(results))
        }
    }
    let lexicon = PatchedLexicon::new(Box::new(InProcess), wordholder::patches().unwrap());
    let got = lexicon.analyze(Lang::It, &["aver"]).await.unwrap();
    assert!(got[0].readings.iter().any(|r| r.lemma == "avere"), "{:?}", got[0].readings);
}
