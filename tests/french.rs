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
    assert!(parses(&run("J'ai mangé.").await));
    // que before avoir is the participle's object: it must agree
    assert!(!parses(&run("Les pommes que j'ai mangé sont bonnes.").await));
    assert!(!parses(&run("Les lettres que je n'ai pas lu sont là.").await));
    assert!(parses(&run("Les lettres que je n'ai pas lues sont là.").await));
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
    // a direct reflexive agrees with the subject, inverted too
    assert!(!parses(&run("Elle s'est levé.").await));
    assert!(parses(&run("S'est-elle levée ?").await));
    // an untagged duplicate reading of parlé does not bypass agreement
    assert!(!parses(&run("Elles se sont parlé.").await));
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
    // the verb after vous stays plural
    assert!(!parses(&run("Vous as un rendez-vous.").await));
}

#[tokio::test]
async fn an_inverted_subject_agrees_with_the_adjective() {
    assert!(parses(&run("Est-elle heureuse ?").await));
    assert!(!parses(&run("Est-il heureuse ?").await));
    assert!(!parses(&run("N'est-il pas heureuse ?").await));
    assert!(parses(&run("Deviendra-t-elle heureuse ?").await));
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

#[tokio::test]
async fn inverted_future_copula_takes_an_adjective() {
    let a = run("Sera-t-elle belle ?").await;
    assert!(parses(&a));
    // The bare noun stays possible after être (il est médecin); the point is
    // that the adjective is not lost.
    let c = a.chosen.iter().find(|c| c.token == "belle").unwrap();
    assert!(c.readings.iter().any(|r| r[0].lemma == "beau" && r[0].cat == "Adj"), "{:?}", c.readings);
}

#[tokio::test]
async fn digits_are_a_number_whatever_else_the_lexicon_says() {
    assert!(parses(&run("J'ai 20 ans.").await));
}

#[tokio::test]
async fn par_and_an_agent_make_etre_and_a_participle_a_passive_even_after_an_adverb() {
    let a = run("La porte est fermée ici par le gardien.").await;
    assert!(parses(&a));
    let c = a.chosen.iter().find(|c| c.token == "fermée").unwrap();
    assert!(!c.uses.iter().flatten().any(|u| u == "ap.participle"), "{:?}", c.uses);
}

#[tokio::test]
async fn par_and_an_agent_make_etre_and_a_participle_a_passive() {
    let a = run("La porte est fermée par le gardien.").await;
    assert!(parses(&a));
    let c = a.chosen.iter().find(|c| c.token == "fermée").unwrap();
    assert!(!c.uses.iter().flatten().any(|u| u == "ap.participle"), "{:?}", c.uses);
}

#[tokio::test]
async fn inverted_compound_tense_with_an_object_pronoun_and_an_adverb() {
    let a = run("L'as-tu déjà vu ?").await;
    assert!(parses(&a));
    assert!(agreed(&a, "vu")[0].starts_with("voir Verb"));
    assert!(parses(&run("Ne l'as-tu pas vu ?").await));
}

#[tokio::test]
async fn object_and_reflexive_pronouns_after_an_order() {
    assert!(parses(&run("Donne-le-moi !").await));
    assert!(parses(&run("Donne-le-toi !").await));
}

#[tokio::test]
async fn inverted_etre_compound_tense_with_an_adverb_keeps_the_verb() {
    let a = run("Est-il déjà venu ?").await;
    assert!(parses(&a));
    let c = a.chosen.iter().find(|c| c.token == "venu").unwrap();
    assert!(c.uses.iter().flatten().any(|u| u == "v.inverted-perfect-etre-adverb"), "{:?}", c.uses);
}

#[tokio::test]
async fn inverted_compound_passive_agrees_with_its_subject() {
    let a = run("A-t-elle été fermée ?").await;
    assert!(parses(&a));
    assert!(agreed(&a, "fermée")[0].starts_with("fermer Verb"));
    assert!(!parses(&run("A-t-elle été fermé ?").await));
}

#[tokio::test]
async fn other_linking_verb_in_a_compound_tense_with_pas() {
    assert!(parses(&run("Il n'est pas devenu heureux.").await));
}

#[tokio::test]
async fn an_indirect_pronoun_before_inverted_avoir_leaves_the_participle_unagreed() {
    assert!(parses(&run("Lui as-tu écrit ?").await));
    assert!(!parses(&run("Lui as-tu écrite ?").await));
    assert!(parses(&run("L'as-tu écrite ?").await));
}

#[tokio::test]
async fn a_linking_verb_with_etre_agrees_in_its_compound_tense() {
    assert!(!parses(&run("Elle est devenu heureuse.").await));
    assert!(parses(&run("Il est devenu heureux.").await));
}

#[tokio::test]
async fn a_subject_relative_keeps_predicate_agreement() {
    assert!(parses(&run("La femme qui est fatiguée est là.").await));
    assert!(!parses(&run("La femme qui est fatigué est là.").await));
}

#[tokio::test]
async fn past_infinitive_of_etre_takes_an_adjective() {
    let a = run("Elle semble avoir été heureuse.").await;
    assert!(parses(&a));
    assert!(agreed(&a, "heureuse")[0].starts_with("heureux Adj"));
}

#[tokio::test]
async fn coordinated_verb_phrases_both_agree_with_the_subject() {
    assert!(parses(&run("Elle est heureuse et est fatiguée.").await));
    assert!(!parses(&run("Elle est heureuse et est fatigué.").await));
}

#[tokio::test]
async fn liaison_s_before_an_attached_y_is_the_imperative() {
    let a = run("Vas-y !").await;
    assert!(parses(&a));
    assert!(agreed(&a, "Vas")[0].contains("mood=imp"));
    // allons and faites keep their own s; donnes-en is donne + liaison s
    assert!(agreed(&run("Allons-y !").await, "Allons")[0].contains("mood=imp"));
    assert!(agreed(&run("Faites-en !").await, "Faites")[0].contains("mood=imp"));
    assert!(agreed(&run("Donnes-en !").await, "Donnes")[0].contains("mood=imp"));
}

#[tokio::test]
async fn an_imperative_before_attached_pronouns_is_finite() {
    // faites is also faire's feminine plural participle
    assert!(agreed(&run("Faites-le !").await, "Faites")[0].contains("mood=imp"));
}

#[tokio::test]
async fn reflexive_with_les_agrees_with_les() {
    assert!(parses(&run("Elle se les est écrites.").await));
    assert!(!parses(&run("Elle se les est écrit.").await));
}

#[tokio::test]
async fn an_adverb_before_a_prenominal_adjective() {
    assert!(parses(&run("C'est un très grand homme.").await));
}

#[tokio::test]
async fn qui_with_etre_and_a_plural_noun_phrase() {
    assert!(parses(&run("Qui sont ces hommes ?").await));
}
