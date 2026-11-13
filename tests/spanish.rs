#![cfg(feature = "wordholder")]
//! Spanish sentences against the hand-written fixture
//! (`tests/fixtures/es-handwritten.json`, in the Word Holder API's format),
//! with the lexicon patches applied as the command line applies them.

use std::path::Path;

use gramalysis::lang::es::Spanish;
use gramalysis::patch::PatchedLexicon;
use gramalysis::wordholder::{self, FixtureLexicon};
use gramalysis::{analyze, SentenceAnalysis};

async fn run(text: &str) -> SentenceAnalysis {
    let fixture = FixtureLexicon::load(Path::new("tests/fixtures/es-handwritten.json")).unwrap();
    let lexicon = PatchedLexicon::new(Box::new(fixture), wordholder::patches().unwrap());
    analyze(&lexicon, &Spanish::load().unwrap(), text).await.unwrap()
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

/// The rules that used `token`'s single reading.
fn uses(a: &SentenceAnalysis, token: &str) -> Vec<String> {
    let c = a.chosen.iter().find(|c| c.token == token).unwrap();
    assert!(c.agreed, "{token} read differently across parses: {:?}", c.readings);
    c.uses[0].clone()
}

#[tokio::test]
async fn fue_with_a_destination_is_ir() {
    let a = run("Fue a Madrid ayer.").await;
    assert!(!a.parses.is_empty());
    assert!(agreed(&a, "Fue")[0].starts_with("ir Verb"));
}

#[tokio::test]
async fn fue_with_an_adjective_is_ser() {
    let a = run("Fue difícil.").await;
    assert!(!a.parses.is_empty());
    assert!(agreed(&a, "Fue")[0].starts_with("ser Verb"));
}

#[tokio::test]
async fn ser_and_a_participle_is_the_passive() {
    let a = run("La casa fue construida por los romanos.").await;
    assert!(!a.parses.is_empty());
    assert!(agreed(&a, "construida")[0].starts_with("construir Verb"));
    assert!(!uses(&a, "construida").iter().any(|u| u == "ap.participle"));
}

#[tokio::test]
async fn estar_and_a_participle_is_a_state() {
    let a = run("La puerta está cerrada.").await;
    assert!(!a.parses.is_empty());
    assert!(agreed(&a, "cerrada")[0].starts_with("cerrar Verb"));
    assert_eq!(uses(&a, "cerrada"), ["ap.participle"]);
}

#[tokio::test]
async fn haber_and_an_invariable_participle_is_a_compound_tense() {
    let a = run("He cerrado la puerta.").await;
    assert!(!a.parses.is_empty());
    assert_eq!(uses(&a, "cerrado"), ["v.perfect"]);
    // After haber the participle never agrees.
    assert!(run("He cerrada la puerta.").await.parses.is_empty());
}

#[tokio::test]
async fn dámelo_is_da_me_lo() {
    let a = run("¡Dámelo!").await;
    assert!(!a.parses.is_empty());
    let r = agreed(&a, "Dámelo");
    assert!(r[0].starts_with("dar Verb") && r[0].contains("mood=imp"), "{r:?}");
    assert!(r[1].starts_with("me Pron") && r[1].contains("case=dat"));
    assert!(r[2].starts_with("lo Pron") && r[2].contains("case=acc"));
}

#[tokio::test]
async fn accents_keep_words_apart() {
    let a = run("Tú tienes tu libro y él tiene el suyo.").await;
    assert!(!a.parses.is_empty());
    assert!(agreed(&a, "Tú")[0].starts_with("tú Pron"));
    assert!(agreed(&a, "tu")[0].starts_with("tu Poss"));
    assert!(agreed(&a, "él")[0].starts_with("él Pron"));
    assert!(agreed(&a, "el")[0].starts_with("el Det"));
}

#[tokio::test]
async fn se_is_the_clitic_and_sé_the_verb() {
    let a = run("Se lo di a mi hermana.").await;
    assert!(!a.parses.is_empty());
    assert!(agreed(&a, "Se")[0].starts_with("se Pron"));
    let a = run("Sé que vienes.").await;
    assert!(!a.parses.is_empty());
    assert!(a.chosen.iter().find(|c| c.token == "Sé").unwrap().readings.iter().all(|r| r[0].lemma != "se"));
}

#[tokio::test]
async fn como_before_bread_is_comer() {
    let a = run("Como pan todos los días.").await;
    assert!(!a.parses.is_empty());
    assert!(agreed(&a, "Como")[0].starts_with("comer Verb"));
}

#[tokio::test]
async fn feminine_stressed_a_takes_el() {
    assert!(!run("¿Dónde está el agua?").await.parses.is_empty());
    assert!(run("¿Dónde está la agua?").await.parses.is_empty());
}

#[tokio::test]
async fn clitics_before_a_negative_command() {
    let a = run("No lo hagas.").await;
    assert!(!a.parses.is_empty());
    assert!(agreed(&a, "hagas")[0].contains("mood=sub"));
}

#[tokio::test]
async fn deme_restores_the_accent_of_dé() {
    let a = run("Deme el libro.").await;
    assert!(!a.parses.is_empty());
    let c = a.chosen.iter().find(|c| c.token == "Deme").unwrap();
    assert!(c.readings.iter().all(|r| r[0].lemma == "dar"));
}

#[tokio::test]
async fn participles_of_different_verbs_coordinate() {
    let a = run("La puerta está abierta o cerrada.").await;
    assert!(!a.parses.is_empty());
    assert_eq!(uses(&a, "abierta"), ["ap.participle"]);
    assert_eq!(uses(&a, "cerrada"), ["ap.participle"]);
}

#[tokio::test]
async fn lo_stands_for_the_predicate_of_ser() {
    let a = run("Ella lo es.").await;
    assert!(!a.parses.is_empty());
    assert!(agreed(&a, "es")[0].starts_with("ser Verb"));
    // ser stays a reading of fue with attributive lo.
    let a = run("Ella lo fue.").await;
    let c = a.chosen.iter().find(|c| c.token == "fue").unwrap();
    assert!(c.readings.iter().any(|r| r[0].lemma == "ser"));
}

#[tokio::test]
async fn a_stressed_possessive_is_a_predicate_of_ser() {
    assert!(!run("El libro es mío.").await.parses.is_empty());
    assert!(run("El libro es mía.").await.parses.is_empty());
}

#[tokio::test]
async fn coordinated_destinations_keep_fue_as_ir() {
    let a = run("Fue a Madrid y a Roma.").await;
    assert!(!a.parses.is_empty());
    assert!(agreed(&a, "Fue")[0].starts_with("ir Verb"));
}

#[tokio::test]
async fn ser_takes_a_clock_time_with_a() {
    let a = run("La reunión fue a las cinco.").await;
    let c = a.chosen.iter().find(|c| c.token == "fue").unwrap();
    assert!(c.readings.iter().any(|r| r[0].lemma == "ser"), "{:?}", c.readings);
}

#[tokio::test]
async fn siendo_in_a_progressive() {
    let a = run("La casa está siendo construida.").await;
    assert!(!a.parses.is_empty());
    assert!(agreed(&a, "construida")[0].starts_with("construir Verb"));
    assert!(!uses(&a, "construida").iter().any(|u| u == "ap.participle"));
    assert!(!run("Está siendo difícil.").await.parses.is_empty());
}

#[tokio::test]
async fn listed_words_keep_their_other_categories() {
    assert!(!run("Estoy a salvo.").await.parses.is_empty());
    let a = run("Toco el bajo.").await;
    let c = a.chosen.iter().find(|c| c.token == "bajo").unwrap();
    assert!(c.readings.iter().any(|r| r[0].cat == "Noun"), "{:?}", c.readings);
}

#[tokio::test]
async fn denme_is_den_me() {
    let a = run("Denme el libro.").await;
    assert!(!a.parses.is_empty());
    let c = a.chosen.iter().find(|c| c.token == "Denme").unwrap();
    assert!(c.readings.iter().all(|r| r[0].lemma == "dar" && r.len() == 2), "{:?}", c.readings);
}

#[tokio::test]
async fn a_participle_before_its_noun_is_an_adjective_use() {
    let a = run("El esperado resultado es bueno.").await;
    assert!(!a.parses.is_empty());
    assert_eq!(uses(&a, "esperado"), ["ap.participle"]);
}

#[tokio::test]
async fn mantente_is_mantén_te() {
    let a = run("Mantente alejado de mí.").await;
    assert!(!a.parses.is_empty());
    assert!(agreed(&a, "Mantente")[0].starts_with("mantener Verb"));
}

#[tokio::test]
async fn a_participle_says_how_the_subject_arrived() {
    assert!(!run("Él llegó cargado.").await.parses.is_empty());
    let a = run("Él llegó cargado con flores y regalos.").await;
    assert!(!a.parses.is_empty());
}

#[tokio::test]
async fn a_progressive_linking_verb_keeps_its_complement() {
    let a = run("Me estoy sintiendo cansado.").await;
    assert!(!a.parses.is_empty());
}

#[tokio::test]
async fn adónde_asks_for_a_destination_so_fue_is_ir() {
    for q in ["¿Adónde fue?", "¿A dónde fue?"] {
        let a = run(q).await;
        assert!(!a.parses.is_empty(), "{q}");
        assert!(agreed(&a, "fue")[0].starts_with("ir Verb"), "{q}");
    }
    let a = run("¿De dónde eres?").await;
    assert!(agreed(&a, "eres")[0].starts_with("ser Verb"));
}

#[tokio::test]
async fn copular_questions_inside_the_sentence() {
    assert!(!run("No sé quién eres.").await.parses.is_empty());
    assert!(!run("No sé dónde está.").await.parses.is_empty());
    let a = run("No sé quién fue.").await;
    let c = a.chosen.iter().find(|c| c.token == "fue").unwrap();
    assert!(c.readings.iter().any(|r| r[0].lemma == "ser"), "{:?}", c.readings);
}

#[tokio::test]
async fn démoselo_is_demos_se_lo() {
    let a = run("Démoselo.").await;
    assert!(!a.parses.is_empty());
    assert!(agreed(&a, "Démoselo")[0].starts_with("dar Verb"));
}

#[tokio::test]
async fn an_attached_dative_keeps_ser_a_copula() {
    let a = run("Puede serme útil.").await;
    assert!(!a.parses.is_empty());
    assert!(agreed(&a, "serme")[0].starts_with("ser Verb"));
}

#[tokio::test]
async fn a_relative_pronoun_as_ser_predicate() {
    assert!(!run("Yo soy la persona que soy.").await.parses.is_empty());
    let a = run("Yo soy la persona que fue.").await;
    let c = a.chosen.iter().find(|c| c.token == "fue").unwrap();
    assert!(c.readings.iter().any(|r| r[0].lemma == "ser"), "{:?}", c.readings);
}

#[tokio::test]
async fn a_preposition_and_question_pronoun_as_ser_predicate() {
    assert!(!run("¿De quién es?").await.parses.is_empty());
    assert!(!run("No sé de quién es.").await.parses.is_empty());
    let a = run("¿De quién fue?").await;
    let c = a.chosen.iter().find(|c| c.token == "fue").unwrap();
    assert!(c.readings.iter().any(|r| r[0].lemma == "ser"), "{:?}", c.readings);
}

#[tokio::test]
async fn an_adverb_keeps_the_clitic_object() {
    assert!(!run("Lo compré ayer.").await.parses.is_empty());
    assert!(run("Lo compré ayer un libro.").await.parses.is_empty());
}

#[tokio::test]
async fn a_clitic_on_the_gerund_is_the_object() {
    assert!(!run("Estoy leyéndolo.").await.parses.is_empty());
    assert!(run("Estoy leyéndolo un libro.").await.parses.is_empty());
}

#[tokio::test]
async fn an_attached_dative_on_a_linking_verb() {
    assert!(!run("Puede estarle agradecido.").await.parses.is_empty());
}

#[tokio::test]
async fn singular_subjects_joined_by_o() {
    let a = run("Tom o Mary fue a Madrid.").await;
    assert!(!a.parses.is_empty());
    assert!(agreed(&a, "fue")[0].starts_with("ir Verb"));
}

#[tokio::test]
async fn sentir_takes_an_adjective_about_its_object() {
    assert!(!run("Él la siente fría.").await.parses.is_empty());
}

#[tokio::test]
async fn adverbs_between_ser_and_the_participle() {
    let a = run("Fue muy bien recibido.").await;
    assert!(!a.parses.is_empty());
    // bien is also a noun (el bien), so "bien recibido" can be a bare noun
    // phrase too; the passive reading must be there.
    let c = a.chosen.iter().find(|c| c.token == "recibido").unwrap();
    assert!(c.uses.iter().flatten().any(|u| u == "vp.passive-adverb"), "{:?}", c.uses);
}

#[tokio::test]
async fn existential_haber_in_progress() {
    assert!(!run("Está habiendo problemas.").await.parses.is_empty());
}

#[tokio::test]
async fn a_climbed_object_clitic_is_the_infinitive_object() {
    assert!(!run("Lo puedo leer mañana.").await.parses.is_empty());
    assert!(!run("Puedo leer un libro.").await.parses.is_empty());
    assert!(run("Lo puedo leer un libro.").await.parses.is_empty());
}

#[tokio::test]
async fn a_perception_verb_object_leaves_the_infinitive_its_own() {
    assert!(!run("Lo vi comprar un libro.").await.parses.is_empty());
}

#[tokio::test]
async fn clitics_on_haber_keep_the_copula() {
    assert!(!run("Puede haberme sido útil.").await.parses.is_empty());
    assert!(!run("Puede haberse quedado cansado.").await.parses.is_empty());
}

#[tokio::test]
async fn todos_keeps_the_pronoun_person() {
    assert!(!run("Todos nosotros podemos leer.").await.parses.is_empty());
}

#[tokio::test]
async fn the_compound_keeps_its_own_readings() {
    let a = run("¡Iros!").await;
    assert!(!a.parses.is_empty());
    assert!(agreed(&a, "Iros")[0].contains("mood=imp"));
}

#[tokio::test]
async fn haber_que_in_a_compound_tense() {
    assert!(!run("Ha habido que esperar.").await.parses.is_empty());
}

#[tokio::test]
async fn copular_perfects_take_the_invariable_participle() {
    assert!(!run("He estado cansado.").await.parses.is_empty());
    assert!(run("He estados cansado.").await.parses.is_empty());
}

#[tokio::test]
async fn adverbs_in_coordinated_passives() {
    let a = run("Fue gravemente herido y atendido.").await;
    assert!(!a.parses.is_empty());
    assert!(agreed(&a, "herido")[0].starts_with("herir Verb"));
}

#[tokio::test]
async fn impersonal_se_with_ser() {
    assert!(!run("Aquí se es feliz.").await.parses.is_empty());
    assert!(!run("Se es feliz.").await.parses.is_empty());
}

#[tokio::test]
async fn cuántos_stands_alone() {
    for q in ["¿Cuántos tienes?", "¿Cuántas son?", "¿Cuántos fueron a Madrid?"] {
        assert!(!run(q).await.parses.is_empty(), "{q}");
    }
}
