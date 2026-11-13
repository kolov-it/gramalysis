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
    // coordination keeps the check
    assert!(!parses(&run("Les lettres que j'ai lu et ai écrit sont là.").await));
    assert!(parses(&run("Les lettres que j'ai lues et ai écrites sont là.").await));
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
    // a consonantal y counts as a consonant
    assert!(parses(&run("Le yacht est là.").await));
    assert!(!parses(&run("L'yacht est là.").await));
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

#[tokio::test]
async fn que_before_avoir_and_a_noun_subject_after_it_agrees() {
    assert!(parses(&run("Les pommes qu'a mangées Pierre sont bonnes.").await));
    assert!(!parses(&run("Les pommes qu'a mangé Pierre sont bonnes.").await));
}

#[tokio::test]
async fn les_before_en() {
    assert!(parses(&run("Il les en empêche.").await));
}

#[tokio::test]
async fn complex_inversion_checks_the_pronoun_against_the_noun() {
    assert!(parses(&run("La femme est-elle heureuse ?").await));
    assert!(!parses(&run("La femme est-il heureuse ?").await));
    assert!(!parses(&run("La femme parle-t-il ?").await));
}

#[tokio::test]
async fn a_question_phrase_subject_agrees_with_its_verb() {
    assert!(parses(&run("Quel train part ?").await));
    assert!(!parses(&run("Quel train partent ?").await));
}

#[tokio::test]
async fn ne_pas_before_an_infinitive_linking_verb() {
    let a = run("Elle semble ne pas être heureuse.").await;
    assert!(parses(&a));
    assert!(agreed(&a, "heureuse")[0].starts_with("heureux Adj"));
}

#[tokio::test]
async fn one_auxiliary_for_two_participles() {
    assert!(parses(&run("J'ai mangé et bu.").await));
}

#[tokio::test]
async fn coordinated_third_person_subjects_take_a_third_person_verb() {
    assert!(parses(&run("Le chat et le chien sont là.").await));
    assert!(!parses(&run("Le chat et le chien êtes là.").await));
}

#[tokio::test]
async fn past_infinitive_with_avoir_does_not_agree() {
    assert!(parses(&run("Elle veut avoir mangé.").await));
    assert!(!parses(&run("Elle veut avoir mangées.").await));
}

#[tokio::test]
async fn question_phrase_as_the_subject_of_an_indirect_question() {
    assert!(parses(&run("Je sais quel train part.").await));
}

#[tokio::test]
async fn reflexive_object_before_a_predicative_adjective() {
    let a = run("Je me vois fatigué.").await;
    assert!(parses(&a));
    assert!(agreed(&a, "fatigué")[0].starts_with("fatiguer Verb"));
}

#[tokio::test]
async fn a_predicative_adjective_agrees_with_the_object_pronoun() {
    assert!(parses(&run("Je la vois heureuse.").await));
    assert!(!parses(&run("Je la vois heureux.").await));
}

#[tokio::test]
async fn a_question_phrase_object_makes_the_participle_agree() {
    let a = run("Quelles lettres as-tu écrites ?").await;
    assert!(parses(&a));
    assert!(agreed(&a, "écrites")[0].starts_with("écrire Verb"));
}

#[tokio::test]
async fn afin_de_and_an_infinitive() {
    assert!(parses(&run("Je travaille afin de vivre.").await));
}

#[tokio::test]
async fn the_reflexive_pronoun_agrees_with_the_verb_in_number() {
    assert!(parses(&run("Tu te laves.").await));
    assert!(parses(&run("Vous vous lavez.").await));
    assert!(!parses(&run("Tu vous laves.").await));
}

#[tokio::test]
async fn a_plural_object_can_share_the_subjects_person() {
    assert!(parses(&run("Je nous ai vus.").await));
}

#[tokio::test]
async fn negated_object_question_agrees() {
    assert!(parses(&run("Quelles lettres n'as-tu pas écrites ?").await));
}

#[tokio::test]
async fn auquel_in_a_question() {
    assert!(parses(&run("Auquel penses-tu ?").await));
}

#[tokio::test]
async fn object_pronoun_before_ayant() {
    assert!(parses(&run("En les ayant vus, je suis heureux.").await));
}

#[tokio::test]
async fn object_pronoun_agreement_through_infinitives() {
    assert!(parses(&run("Je veux la rendre heureuse.").await));
    assert!(!parses(&run("Je veux la rendre heureux.").await));
}

#[tokio::test]
async fn negated_reflexive_predicative() {
    assert!(parses(&run("Elle ne se rend pas heureuse.").await));
}

#[tokio::test]
async fn cru_meaning_raw_stays_an_adjective() {
    let a = run("Je mange du poisson cru.").await;
    assert!(parses(&a));
    assert!(agreed(&a, "cru")[0].starts_with("cru Adj"));
}

#[tokio::test]
async fn a_question_pronoun_object_makes_the_participle_agree() {
    let a = run("Qu'as-tu fait ?").await;
    assert!(parses(&a));
    assert!(agreed(&a, "fait")[0].starts_with("faire Verb"));
}

#[tokio::test]
async fn coordinated_feminine_subjects_take_a_feminine_predicate() {
    assert!(parses(&run("La femme et la fille sont heureuses.").await));
    assert!(!parses(&run("La femme et la fille sont heureux.").await));
    assert!(parses(&run("Le chat et la fille sont heureux.").await));
}

#[tokio::test]
async fn object_agreement_through_ne_pas_before_an_infinitive() {
    assert!(parses(&run("Je veux ne pas la rendre heureuse.").await));
    assert!(!parses(&run("Je veux ne pas la rendre heureux.").await));
}

#[tokio::test]
async fn subject_controls_an_infinitive_predicate() {
    assert!(parses(&run("Elle veut être heureuse.").await));
    // heureux is also a noun, but bare after être it is the adjective
    assert!(!parses(&run("Elle veut être heureux.").await));
    assert!(!parses(&run("La femme est heureux.").await));
}

#[tokio::test]
async fn bare_nouns_after_avoir_stay_but_participle_nouns_need_a_determiner() {
    assert!(parses(&run("Vous avez froid.").await));
    // écrit is also a noun (un écrit), but bare it is the participle
    assert!(parses(&run("Quelles lettres lui as-tu écrites ?").await));
    assert!(!parses(&run("Quelles lettres lui as-tu écrit ?").await));
}

#[tokio::test]
async fn object_control_after_laisser() {
    assert!(parses(&run("Elle le laisse être heureux.").await));
    assert!(!parses(&run("Elle le laisse être heureuse.").await));
}

#[tokio::test]
async fn a_reflexive_infinitive_takes_the_subjects_person() {
    assert!(parses(&run("Je veux me laver.").await));
    assert!(!parses(&run("Je veux se laver.").await));
}

#[tokio::test]
async fn the_digit_one_takes_a_singular_noun() {
    assert!(parses(&run("J'ai 1 lettre.").await));
}

#[tokio::test]
async fn a_subjunctive_clause_cannot_stand_alone_even_after_an_adverb() {
    assert!(!parses(&run("Il soit heureux.").await));
    assert!(!parses(&run("Demain il soit heureux.").await));
}

#[tokio::test]
async fn a_reflexive_predicative_in_an_infinitive() {
    assert!(parses(&run("Elle veut se rendre heureuse.").await));
    assert!(!parses(&run("Elle veut se rendre heureux.").await));
}

#[tokio::test]
async fn coordinated_subject_pronouns_stay_subjects() {
    assert!(parses(&run("Il et elle sont là.").await));
    assert!(!parses(&run("Je vois il et elle.").await));
}

#[tokio::test]
async fn object_question_with_a_noun_subject_and_inversion_agrees() {
    assert!(parses(&run("Quelles lettres la femme a-t-elle écrites ?").await));
    assert!(!parses(&run("Quelles lettres la femme a-t-elle écrit ?").await));
}

#[tokio::test]
async fn ne_pas_keeps_the_reflexive_controller_check() {
    assert!(parses(&run("Je veux ne pas me laver.").await));
    assert!(!parses(&run("Je veux ne pas se laver.").await));
}

#[tokio::test]
async fn an_etre_based_infinitive_agrees_with_its_controller() {
    assert!(parses(&run("Elle veut avoir été fermée.").await));
    assert!(!parses(&run("Elle veut avoir été fermé.").await));
}

#[tokio::test]
async fn a_predicative_adjective_agrees_with_an_object_pronoun_in_a_compound_tense() {
    assert!(parses(&run("Je les ai vus fatigués.").await));
    assert!(!parses(&run("Je les ai vus fatigué.").await));
}

#[tokio::test]
async fn nested_infinitives_keep_the_reflexive_controller_check() {
    assert!(parses(&run("Je veux pouvoir me laver.").await));
    assert!(!parses(&run("Je veux pouvoir se laver.").await));
}

#[tokio::test]
async fn d_abord_inside_a_compound_tense() {
    let a = run("J'ai d'abord écrit la lettre.").await;
    assert!(parses(&a));
    assert!(agreed(&a, "écrit")[0].starts_with("écrire Verb"));
}

#[tokio::test]
async fn an_attached_object_pronoun_agrees_with_a_predicative_adjective() {
    assert!(parses(&run("Rends-la heureuse.").await));
    assert!(!parses(&run("Rends-la heureux.").await));
}

#[tokio::test]
async fn ne_pas_keeps_predicate_agreement_in_an_infinitive() {
    assert!(parses(&run("Elle veut ne pas avoir été fermée.").await));
    assert!(!parses(&run("Elle veut ne pas avoir été fermé.").await));
}

#[tokio::test]
async fn an_explicit_object_controls_the_infinitive() {
    assert!(parses(&run("Elle laisse la femme être heureuse.").await));
    assert!(!parses(&run("Elle laisse la femme être heureux.").await));
}

#[tokio::test]
async fn pour_and_an_infinitive_take_the_clause_subject() {
    assert!(parses(&run("Je suis venu pour me laver.").await));
    assert!(!parses(&run("Je suis venu pour se laver.").await));
}

#[tokio::test]
async fn coordinated_verb_phrases_keep_the_mood_check() {
    assert!(!parses(&run("Il est heureux et soit heureux.").await));
}

#[tokio::test]
async fn time_expressions_agree() {
    assert!(parses(&run("Elle est heureuse ce matin.").await));
    assert!(!parses(&run("Elle est heureuse cette matin.").await));
}

#[tokio::test]
async fn object_agreement_in_a_negative_imperative() {
    assert!(parses(&run("Ne la rends pas heureuse.").await));
    assert!(!parses(&run("Ne la rends pas heureux.").await));
}

#[tokio::test]
async fn causative_fait_stays_invariable_before_an_infinitive() {
    assert!(parses(&run("Elle les a fait partir.").await));
    assert!(!parses(&run("Elle les a faits partir.").await));
    assert!(parses(&run("Elle les fait partir.").await));
    assert!(parses(&run("Elle ne les a pas fait partir.").await));
    assert!(parses(&run("Les a-t-elle fait partir ?").await));
    assert!(!parses(&run("Elle les a fait.").await));
}

#[tokio::test]
async fn etre_cense_keeps_the_controller_checks() {
    assert!(parses(&run("Je suis censé me laver.").await));
    assert!(!parses(&run("Je suis censé se laver.").await));
}

#[tokio::test]
async fn moi_as_the_direct_object_of_an_order() {
    assert!(parses(&run("Rends-moi heureux.").await));
}

#[tokio::test]
async fn an_object_pronoun_controls_a_reflexive_infinitive() {
    assert!(parses(&run("Elle me laisse me laver.").await));
    assert!(!parses(&run("Elle me laisse se laver.").await));
}

#[tokio::test]
async fn a_subordinate_clause_does_not_license_a_subjunctive_main_clause() {
    assert!(!parses(&run("Il soit heureux quand il est là.").await));
}

#[tokio::test]
async fn a_reflexive_order_with_an_adjective() {
    assert!(parses(&run("Rends-toi heureux.").await));
}

#[tokio::test]
async fn quel_with_etre_agrees_with_the_noun_phrase() {
    assert!(parses(&run("Quel est ton chat ?").await));
    assert!(!parses(&run("Quel sont ton chat ?").await));
}

#[tokio::test]
async fn indicative_and_conditional_clauses_coordinate() {
    assert!(parses(&run("Il est heureux et elle serait heureuse.").await));
    assert!(parses(&run("Elle serait heureuse et il est heureux.").await));
    assert!(!parses(&run("Il est heureux et elle soit heureuse.").await));
}

#[tokio::test]
async fn a_reflexive_predicative_in_a_compound_tense() {
    assert!(parses(&run("Je me suis rendu heureux.").await));
    assert!(parses(&run("Elle s'est rendue heureuse.").await));
    assert!(!parses(&run("Elle s'est rendue heureux.").await));
}
