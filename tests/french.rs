#![cfg(feature = "wordholder")]
//! French sentences against the hand-written fixture
//! (`tests/fixtures/fr-handwritten.json`, in the Word Holder API's format).

use std::path::Path;

use gramalysis::lang::fr::French;
use gramalysis::wordholder::FixtureLexicon;
use gramalysis::{analyze, SentenceAnalysis};

async fn run(text: &str) -> SentenceAnalysis {
    let lexicon = FixtureLexicon::load(Path::new("tests/fixtures/fr-handwritten.json")).unwrap();
    analyze(&lexicon, &French::load().unwrap(), text).await.unwrap()
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

fn parses(a: &SentenceAnalysis) -> bool {
    a.parse_count > 0
}

#[tokio::test]
async fn suis_is_etre_with_a_subject() {
    // With an object it could be suivre too (je suis ton frère: I follow
    // your brother); with an adjective it can only be être.
    let a = run("Je suis heureux.").await;
    assert!(parses(&a));
    assert!(agreed(&a, "suis")[0].starts_with("être Verb"), "{:?}", agreed(&a, "suis"));
}

#[tokio::test]
async fn suis_is_suivre_in_an_order() {
    let a = run("Suis-moi !").await;
    assert!(parses(&a));
    let r = agreed(&a, "Suis");
    assert!(r[0].starts_with("suivre Verb") && r[0].contains("mood=imp"), "{r:?}");
}

#[tokio::test]
async fn elision_of_le_and_que() {
    let a = run("L'homme qu'il a vu est là.").await;
    assert!(parses(&a));
    assert!(agreed(&a, "L'")[0].starts_with("le Det"));
    assert!(agreed(&a, "vu")[0].starts_with("voir Verb"));
    assert!(agreed(&a, "a")[0].starts_with("avoir Verb"));
}

#[tokio::test]
async fn inversion_is_a_question_and_the_hyphenated_pronoun_its_subject() {
    let a = run("Parlez-vous français ?").await;
    assert!(parses(&a));
    assert!(agreed(&a, "Parlez")[0].contains("mood=ind"));
    assert!(agreed(&a, "-vous")[0].contains("case=nom"));
    let a = run("A-t-il fini ?").await;
    assert!(parses(&a));
    assert!(agreed(&a, "A")[0].starts_with("avoir Verb"));
    assert!(agreed(&a, "fini")[0].starts_with("finir Verb"));
}

#[tokio::test]
async fn hyphenated_pronoun_after_an_imperative() {
    let a = run("Asseyez-vous.").await;
    assert!(parses(&a));
    assert!(agreed(&a, "Asseyez")[0].contains("mood=imp"));
    assert!(agreed(&a, "-vous")[0].contains("case=refl"));
}

#[tokio::test]
async fn participle_after_avoir_agrees_only_with_an_object_before_it() {
    assert!(parses(&run("Je les ai vues.").await));
    assert!(!parses(&run("Il a mangées.").await));
    let a = run("Les pommes que j'ai mangées sont bonnes.").await;
    assert!(parses(&a));
    assert!(agreed(&a, "mangées")[0].starts_with("manger Verb"));
}

#[tokio::test]
async fn aspirated_h_takes_le_not_l() {
    assert!(parses(&run("Le héros est là.").await));
    assert!(!parses(&run("L'héros est là.").await));
    assert!(parses(&run("L'homme est là.").await));
    assert!(!parses(&run("Le homme est là.").await));
}

#[tokio::test]
async fn porte_is_the_noun_after_an_article_and_the_verb_in_an_order() {
    let a = run("Ferme la porte.").await;
    assert!(parses(&a));
    assert!(agreed(&a, "porte")[0].starts_with("porte Noun"));
    assert!(agreed(&a, "Ferme")[0].starts_with("fermer Verb"));
}

#[tokio::test]
async fn pronominal_verb_takes_etre() {
    let a = run("Elle s'est levée.").await;
    assert!(parses(&a));
    assert!(agreed(&a, "levée")[0].starts_with("lever Verb"));
}

#[tokio::test]
async fn etre_with_a_participle_is_open_while_the_auxiliary_is_unknown() {
    // fermer takes avoir, so this is a state (an adjective use); the API does
    // not give French auxiliaries yet, so the compound-tense reading stays.
    let a = run("La porte est fermée.").await;
    assert!(parses(&a));
    let c = a.chosen.iter().find(|c| c.token == "fermée").unwrap();
    assert!(c.uses.iter().flatten().any(|u| u == "ap.participle"));
}

#[tokio::test]
async fn polite_vous_takes_a_plural_verb_and_a_singular_predicate() {
    assert!(parses(&run("Vous êtes fatigué.").await));
    assert!(parses(&run("Vous êtes fatigués.").await));
    assert!(!parses(&run("Ils sont fatigué.").await));
}

#[tokio::test]
async fn etre_in_the_future_with_a_participle_is_a_passive() {
    let a = run("La porte sera fermée.").await;
    assert!(parses(&a));
    let c = a.chosen.iter().find(|c| c.token == "fermée").unwrap();
    assert!(!c.uses.iter().flatten().any(|u| u == "ap.participle"), "{:?}", c.uses);
}

#[tokio::test]
async fn rendez_vous_is_one_noun() {
    let a = run("J'ai un rendez-vous.").await;
    assert!(parses(&a));
    assert!(agreed(&a, "rendez-vous")[0].starts_with("rendez-vous Noun"));
}
