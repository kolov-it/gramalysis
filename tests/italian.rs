#![cfg(feature = "wordholder")]
//! Sentences against the hand-written fixture. The fixture follows the Word
//! Holder API's format, plus noun gender on lemma forms (`ragazza`, `cane`),
//! which the API does not return yet (doc/lexicon-api-needs.md, need 2).

use std::path::Path;

use gramalysis::lang::it::Italian;
use gramalysis::wordholder::FixtureLexicon;
use gramalysis::{analyze, SentenceAnalysis};

async fn run(text: &str) -> SentenceAnalysis {
    let lexicon = FixtureLexicon::load(Path::new("tests/fixtures/it-handwritten.json")).unwrap();
    analyze(&lexicon, &Italian::load().unwrap(), text).await.unwrap()
}

/// The single reading every parse gave `token`, as "lemma Cat feats" per morpheme.
fn agreed(a: &SentenceAnalysis, token: &str) -> Vec<String> {
    let c = a.chosen.iter().find(|c| c.token == token).unwrap();
    assert!(c.agreed, "{token} read differently across parses: {:?}", c.readings);
    c.readings[0]
        .iter()
        .map(|m| format!("{} {} {}", m.lemma, m.cat, gramalysis::syntax::feats::display(&m.feats)))
        .collect()
}

#[tokio::test]
async fn fece_is_fare_third_singular() {
    let a = run("Tom fece del volontariato.").await;
    assert!(!a.parses.is_empty());
    assert_eq!(agreed(&a, "fece"), ["fare Verb finite=yes,mood=ind,number=sg,person=3,tense=past"]);
}

#[tokio::test]
async fn subject_agreement_picks_present_over_imperative() {
    let a = run("La ragazza ama il cane.").await;
    assert!(!a.parses.is_empty());
    assert_eq!(agreed(&a, "ama"), ["amare Verb finite=yes,mood=ind,number=sg,person=3,tense=pres"]);
    assert_eq!(agreed(&a, "La"), ["il Det definite=yes,gender=f,next=c,number=sg"]);
}

#[tokio::test]
async fn participle_agrees_after_essere() {
    assert!(!run("La ragazza è arrivata.").await.parses.is_empty());
    assert!(run("La ragazza è arrivato.").await.parses.is_empty());
}

#[tokio::test]
async fn enclitics_on_an_imperative() {
    let a = run("Dammelo!").await;
    assert!(!a.parses.is_empty());
    let r = agreed(&a, "Dammelo");
    assert_eq!(r[0], "dare Verb finite=yes,mood=imp,number=sg,person=2");
    assert!(r[1].starts_with("mi Pron case=dat"));
    assert!(r[2].starts_with("lo Pron case=acc"));
}

#[tokio::test]
async fn ricordati_is_the_imperative_here() {
    let a = run("Ricordati di me.").await;
    assert!(!a.parses.is_empty());
    assert_eq!(agreed(&a, "Ricordati")[0], "ricordare Verb finite=yes,mood=imp,number=sg,person=2");
}

#[tokio::test]
async fn che_clause_negation_and_infinitive() {
    let a = run("Disse che non riusciva a trovare un lavoro.").await;
    assert!(!a.parses.is_empty());
    assert_eq!(agreed(&a, "lavoro"), ["lavoro Noun number=sg"]);
    assert_eq!(agreed(&a, "trovare"), ["trovare Verb finite=no,form=inf"]);
}

#[tokio::test]
async fn clitic_before_a_compound_tense() {
    let a = run("L'ho fatto.").await;
    assert!(!a.parses.is_empty());
    assert_eq!(agreed(&a, "fatto"), ["fare Verb finite=no,form=pastpart,gender=m,number=sg"]);
}

#[tokio::test]
async fn article_form_follows_the_next_sound() {
    assert!(!run("La ragazza ama il cane.").await.parses.is_empty());
    assert!(run("La ragazza ama lo cane.").await.parses.is_empty());
    assert!(!run("La ragazza ama lo zio.").await.parses.is_empty());
    assert!(run("La ragazza ama il zio.").await.parses.is_empty());
}
