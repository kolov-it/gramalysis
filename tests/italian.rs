#![cfg(feature = "wordholder")]
//! Sentences against the hand-written fixture. The fixture follows the Word
//! Holder API's format, plus noun gender on lemma forms (`ragazza`, `cane`),
//! which the API does not return yet (doc/lexicon-api-needs.md, need 2).

use std::path::Path;

use syntango::lang::it::Italian;
use syntango::wordholder::FixtureLexicon;
use syntango::{analyze, SentenceAnalysis};

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
        .map(|m| format!("{} {} {}", m.lemma, m.cat, syntango::syntax::feats::display(&m.feats)))
        .collect()
}

#[tokio::test]
async fn fece_is_fare_third_singular() {
    let a = run("Tom fece del volontariato.").await;
    assert!(!a.parses.is_empty());
    assert_eq!(agreed(&a, "fece"), ["fare Verb finite=yes,form=fin,mood=ind,number=sg,person=3,tense=past"]);
}

#[tokio::test]
async fn subject_agreement_picks_present_over_imperative() {
    let a = run("La ragazza ama il cane.").await;
    assert!(!a.parses.is_empty());
    assert_eq!(agreed(&a, "ama"), ["amare Verb finite=yes,form=fin,mood=ind,number=sg,person=3,tense=pres"]);
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
    assert_eq!(r[0], "dare Verb finite=yes,form=fin,mood=imp,number=sg,person=2");
    assert!(r[1].starts_with("mi Pron") && r[1].contains("case=dat"));
    assert!(r[2].starts_with("lo Pron") && r[2].contains("case=acc"));
}

#[tokio::test]
async fn ricordati_is_the_imperative_here() {
    let a = run("Ricordati di me.").await;
    assert!(!a.parses.is_empty());
    assert_eq!(agreed(&a, "Ricordati")[0], "ricordare Verb finite=yes,form=fin,mood=imp,number=sg,person=2");
}

#[tokio::test]
async fn che_clause_negation_and_infinitive() {
    let a = run("Disse che non riusciva a trovare un lavoro.").await;
    assert!(!a.parses.is_empty());
    assert_eq!(agreed(&a, "lavoro"), ["lavoro Noun gender=m,number=sg"]);
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

/// Each reading keeps the rules that used it: in "È ferito." the noun is used
/// as a noun and the participle only like an adjective, so no reading is a
/// verb use (Codex review on #3).
#[tokio::test]
async fn uses_are_per_reading() {
    let a = run("È ferito.").await;
    let c = a.chosen.iter().find(|c| c.token == "ferito").unwrap();
    assert_eq!(c.readings.len(), c.uses.len());
    for (reading, uses) in c.readings.iter().zip(&c.uses) {
        match reading[0].cat.as_str() {
            "Verb" => assert_eq!(uses, &["ap.participle"]),
            "Noun" => assert!(!uses.iter().any(|u| u == "ap.participle"), "{uses:?}"),
            other => panic!("unexpected reading {other}"),
        }
    }
}

/// Whether some parse reads `token` as `lemma`'s past participle in a verb use
/// (a participle clause), not only like an adjective.
fn participle_as_verb(a: &SentenceAnalysis, token: &str, lemma: &str) -> bool {
    let c = a.chosen.iter().find(|c| c.token == token).unwrap();
    c.readings.iter().zip(&c.uses).any(|(reading, uses)| {
        let m = &reading[0];
        m.lemma == lemma
            && m.cat == "Verb"
            && m.feats.get("form").map(String::as_str) == Some("pastpart")
            && uses.iter().any(|u| u != "ap.participle")
    })
}

/// Participle clauses (#16): the participle's verb reading is among the parses,
/// next to any other reading of the form (giunto: also giuntare).
#[tokio::test]
async fn participle_clauses() {
    for (sentence, token, lemma) in [
        (
            "Giunto dinanzi a casa, trovò l'uscio di strada socchiuso.",
            "Giunto",
            "giungere",
        ),
        ("Noi amiamo e dopo amato usciamo.", "amato", "amare"),
        ("Giunta la sera, tornammo a casa.", "Giunta", "giungere"),
        ("Arrivato a casa, ho mangiato.", "Arrivato", "arrivare"),
        ("Finito il lavoro, siamo usciti.", "Finito", "finire"),
        ("Dopo mangiato, dormo.", "mangiato", "mangiare"),
    ] {
        let a = run(sentence).await;
        assert!(a.unknown.is_empty(), "{sentence}: unknown {:?}", a.unknown);
        assert!(
            participle_as_verb(&a, token, lemma),
            "{sentence}: {:?}",
            a.chosen.iter().find(|c| c.token == token)
        );
    }
}

/// An adverb with its own phrase completes a participle (#18): messa fuori di
/// casa is a participle phrase, a verb use.
#[tokio::test]
async fn participle_with_an_adverb_phrase() {
    for (sentence, token, lemma) in [
        ("Era una sedia messa fuori di casa.", "messa", "mettere"),
        (
            "Ho trovato una bici lasciata fuori di casa.",
            "lasciata",
            "lasciare",
        ),
    ] {
        let a = run(sentence).await;
        assert!(
            participle_as_verb(&a, token, lemma),
            "{sentence}: {:?}",
            a.chosen.iter().find(|c| c.token == token)
        );
    }
}

/// The participle must agree with the noun phrase of its absolute clause.
#[tokio::test]
async fn absolute_participle_agrees_with_its_noun_phrase() {
    let a = run("Finito il lavoro, siamo usciti.").await;
    assert!(participle_as_verb(&a, "Finito", "finire"));
    let a = run("Finito la sera, siamo usciti.").await;
    assert!(!participle_as_verb(&a, "Finito", "finire"));
}

/// Whether some parse reads `token` like an adjective: as the adjective
/// `lemma`, or as `lemma`'s present participle used like one.
fn present_participle_as_adjective(a: &SentenceAnalysis, token: &str, lemma: &str) -> bool {
    let c = a.chosen.iter().find(|c| c.token == token).unwrap();
    c.readings.iter().zip(&c.uses).any(|(reading, uses)| {
        let m = &reading[0];
        m.lemma == lemma
            && (m.cat == "Adj"
                || (m.cat == "Verb"
                    && m.feats.get("form").map(String::as_str) == Some("prespart")
                    && uses.iter().any(|u| u == "ap.present-participle")))
    })
}

/// A present participle after a noun (#18): like an adjective, a state
/// (il treno fuggente), also without an adjective entry in the lexicon.
#[tokio::test]
async fn present_participle_after_a_noun() {
    for (sentence, token, lemma) in [
        ("Guardava il treno fuggente.", "fuggente", "fuggire"),
        (
            "Guardavo le foglie tremanti nel vento.",
            "tremanti",
            "tremante",
        ),
        (
            "Salutava la folla sorridente dal balcone.",
            "sorridente",
            "sorridente",
        ),
        (
            "Salutava la folla sorridente dal balcone.",
            "sorridente",
            "sorridere",
        ),
        (
            "Ciao! — al treno fuggente sul ponte lontano, dove si vedevano ancora ondeggiare fuor dei vagoni i pennacchi dei bersaglieri.",
            "fuggente",
            "fuggire",
        ),
        ("Vedo un bambino piangente.", "piangente", "piangente"),
        ("Vedo un bambino piangente.", "piangente", "piangere"),
    ] {
        let a = run(sentence).await;
        assert!(a.unknown.is_empty(), "{sentence}: unknown {:?}", a.unknown);
        assert!(
            present_participle_as_adjective(&a, token, lemma),
            "{sentence}: {lemma}: {:?}",
            a.chosen.iter().find(|c| c.token == token)
        );
    }
}

/// With its own complement the present participle is also a verb use, a
/// participle phrase (sorridente dal balcone).
#[tokio::test]
async fn present_participle_with_a_complement() {
    let a = run("Salutava la folla sorridente dal balcone.").await;
    let c = a.chosen.iter().find(|c| c.token == "sorridente").unwrap();
    assert!(c.readings.iter().zip(&c.uses).any(|(reading, uses)| {
        reading[0].lemma == "sorridere" && uses.iter().any(|u| u == "partp.present")
    }));
}

/// The present participle agrees with the noun in number: -nte is singular.
#[tokio::test]
async fn present_participle_agrees_in_number() {
    assert!(!run("Guardava il treno fuggente.").await.parses.is_empty());
    assert!(run("Guardava le foglie fuggente.").await.parses.is_empty());
}

/// A phrase right after a participle belongs to the participle or to the noun,
/// and both readings are returned: un suono soffocato di pianti is a sound
/// suffocated by crying or a muffled sound of crying.
#[tokio::test]
async fn phrase_after_a_participle_has_both_readings() {
    let lexicon = FixtureLexicon::load(Path::new("tests/fixtures/it-literature.json")).unwrap();
    let sentence = "A questo punto si sentì nella camera un suono soffocato di pianti e singhiozzi.";
    let a = analyze(&lexicon, &Italian::load().unwrap(), sentence).await.unwrap();
    assert!(a.unknown.is_empty(), "unknown {:?}", a.unknown);
    let c = a.chosen.iter().find(|c| c.token == "soffocato").unwrap();
    assert!(participle_as_verb(&a, "soffocato", "soffocare"), "{c:?}");
    assert!(
        c.uses.iter().any(|uses| uses.iter().any(|u| u == "ap.participle"))
            || c.readings.iter().any(|r| r[0].cat == "Adj"),
        "{c:?}"
    );
}
