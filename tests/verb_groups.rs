#![cfg(feature = "wordholder")]
//! Each kept parse's verb groups (`SentenceAnalysis::verb_groups`), against
//! the recorded answers for the homograph sets (tests/fixtures/<lang>-set.json),
//! patched as the command line uses them.

use std::path::Path;

use syntango::lang;
use syntango::morph::Lang;
use syntango::patch::PatchedLexicon;
use syntango::wordholder::{self, FixtureLexicon};
use syntango::{SentenceAnalysis, VerbGroup, Voice, analyze};

async fn run(lang: Lang, text: &str) -> SentenceAnalysis {
    let fixture = FixtureLexicon::load(Path::new(&format!(
        "tests/fixtures/{}-set.json",
        lang.code()
    )))
    .unwrap();
    let lexicon = PatchedLexicon::new(Box::new(fixture), wordholder::patches().unwrap());
    let language = lang::load(lang).unwrap();
    let a = analyze(&lexicon, language.as_ref(), text).await.unwrap();
    assert!(!a.parses.is_empty(), "{text}: no parse");
    assert_eq!(
        a.verb_groups.len(),
        a.parses.len(),
        "{text}: one list per kept parse"
    );
    a
}

/// What a test expects of a group.
struct Expected<'a> {
    tokens: Vec<usize>,
    auxiliaries: Vec<usize>,
    participle: Option<usize>,
    verb: usize,
    lemma: &'a str,
    cell: Option<&'a str>,
    voice: Voice,
    progressive: bool,
}

fn check(g: &VerbGroup, e: &Expected) {
    assert_eq!(g.tokens, e.tokens, "{g:?}");
    assert_eq!(g.auxiliaries, e.auxiliaries, "{g:?}");
    assert_eq!(g.participle, e.participle, "{g:?}");
    assert_eq!(g.verb, e.verb, "{g:?}");
    assert_eq!(g.lemma, e.lemma, "{g:?}");
    assert_eq!(g.cell.as_deref(), e.cell, "{g:?}");
    assert_eq!(g.voice, e.voice, "{g:?}");
    assert_eq!(g.progressive, e.progressive, "{g:?}");
}

fn simple<'a>(token: usize, lemma: &'a str, cell: &'a str) -> Expected<'a> {
    Expected {
        tokens: vec![token],
        auxiliaries: Vec::new(),
        participle: None,
        verb: token,
        lemma,
        cell: Some(cell),
        voice: Voice::Active,
        progressive: false,
    }
}

/// Every kept parse has exactly these groups.
fn every_parse(a: &SentenceAnalysis, expected: &[Expected]) {
    for (p, groups) in a.verb_groups.iter().enumerate() {
        assert_eq!(groups.len(), expected.len(), "parse {p}: {groups:?}");
        for (g, e) in groups.iter().zip(expected) {
            check(g, e);
        }
    }
}

#[tokio::test]
async fn italian_compound_tense_is_one_group_named_after_the_auxiliary() {
    let a = run(Lang::It, "Ho dato il libro a Maria.").await;
    assert!(a.parses.len() > 1, "the test is about every parse");
    every_parse(
        &a,
        &[Expected {
            tokens: vec![0, 1],
            auxiliaries: vec![0],
            participle: Some(1),
            verb: 1,
            lemma: "dare",
            cell: Some("indicative.present_perfect.1s"),
            voice: Voice::Active,
            progressive: false,
        }],
    );
}

#[tokio::test]
async fn italian_simple_verb_is_a_one_token_group() {
    let a = run(Lang::It, "La ragazza ama il cane.").await;
    every_parse(&a, &[simple(2, "amare", "indicative.present.3s")]);
}

#[tokio::test]
async fn italian_compound_passive_has_both_auxiliaries() {
    let a = run(Lang::It, "Il libro è stato letto.").await;
    let [g] = a.verb_groups[0].as_slice() else {
        panic!("{:?}", a.verb_groups[0])
    };
    check(
        g,
        &Expected {
            tokens: vec![2, 3, 4],
            auxiliaries: vec![2, 3],
            participle: Some(4),
            verb: 4,
            lemma: "leggere",
            cell: Some("indicative.present_perfect.3s"),
            voice: Voice::Passive,
            progressive: false,
        },
    );
    // Another parse reads letto as an adjective: è stato is then essere's
    // own present perfect.
    assert!(
        a.verb_groups.iter().flatten().any(|g| g.lemma == "essere"
            && g.tokens == [2, 3]
            && g.cell.as_deref() == Some("indicative.present_perfect.3s")
            && g.voice == Voice::Active),
        "{:?}",
        a.verb_groups
    );
}

#[tokio::test]
async fn italian_passive_of_a_simple_tense_takes_the_auxiliary_cell() {
    let a = run(Lang::It, "Il libro fu letto.").await;
    let [g] = a.verb_groups[0].as_slice() else {
        panic!("{:?}", a.verb_groups[0])
    };
    check(
        g,
        &Expected {
            tokens: vec![2, 3],
            auxiliaries: vec![2],
            participle: Some(3),
            verb: 3,
            lemma: "leggere",
            cell: Some("indicative.past_historic.3s"),
            voice: Voice::Passive,
            progressive: false,
        },
    );
}

#[tokio::test]
async fn a_word_between_auxiliary_and_participle_is_not_in_the_group() {
    let a = run(Lang::It, "Avete mai letto una poesia cinese?").await;
    every_parse(
        &a,
        &[Expected {
            tokens: vec![0, 2],
            auxiliaries: vec![0],
            participle: Some(2),
            verb: 2,
            lemma: "leggere",
            cell: Some("indicative.present_perfect.2p"),
            voice: Voice::Active,
            progressive: false,
        }],
    );
}

#[tokio::test]
async fn coordinated_participles_share_their_auxiliary() {
    let a = run(Lang::It, "Sono nato e cresciuto in campagna.").await;
    let groups = &a.verb_groups[0];
    let shared = |verb: usize, lemma: &'static str| Expected {
        tokens: vec![0, verb],
        auxiliaries: vec![0],
        participle: Some(verb),
        verb,
        lemma,
        cell: Some("indicative.present_perfect.1s"),
        voice: Voice::Active,
        progressive: false,
    };
    let [nato, cresciuto] = groups.as_slice() else {
        panic!("{groups:?}")
    };
    check(nato, &shared(1, "nascere"));
    check(cresciuto, &shared(3, "crescere"));
}

#[tokio::test]
async fn italian_progressive_is_the_auxiliary_and_the_gerund() {
    let a = run(Lang::It, "I prezzi stanno crescendo.").await;
    let [g] = a.verb_groups[0].as_slice() else {
        panic!("{:?}", a.verb_groups[0])
    };
    check(
        g,
        &Expected {
            tokens: vec![2, 3],
            auxiliaries: vec![2],
            participle: None,
            verb: 3,
            lemma: "crescere",
            cell: Some("indicative.present.3p"),
            voice: Voice::Active,
            progressive: true,
        },
    );
}

#[tokio::test]
async fn spanish_compound_tense() {
    let a = run(Lang::Es, "Tom me ha dado un bolígrafo.").await;
    every_parse(
        &a,
        &[Expected {
            tokens: vec![2, 3],
            auxiliaries: vec![2],
            participle: Some(3),
            verb: 3,
            lemma: "dar",
            cell: Some("indicative.present_perfect.3s"),
            voice: Voice::Active,
            progressive: false,
        }],
    );
}

#[tokio::test]
async fn spanish_perfect_progressive_is_named_after_its_auxiliaries() {
    let a = run(Lang::Es, "Ha estado lloviendo desde ayer.").await;
    let [g] = a.verb_groups[0].as_slice() else {
        panic!("{:?}", a.verb_groups[0])
    };
    check(
        g,
        &Expected {
            tokens: vec![0, 1, 2],
            auxiliaries: vec![0, 1],
            participle: None,
            verb: 2,
            lemma: "llover",
            cell: Some("indicative.present_perfect.3s"),
            voice: Voice::Active,
            progressive: true,
        },
    );
}

#[tokio::test]
async fn french_compound_tense_with_a_clitic_before_the_auxiliary() {
    let a = run(Lang::Fr, "Je l'ai fait à sa demande.").await;
    let ai = a.tokens.iter().position(|t| t == "ai").unwrap();
    let fait = a.tokens.iter().position(|t| t == "fait").unwrap();
    every_parse(
        &a,
        &[Expected {
            tokens: vec![ai, fait],
            auxiliaries: vec![ai],
            participle: Some(fait),
            verb: fait,
            lemma: "faire",
            cell: Some("indicative.present_perfect.1s"),
            voice: Voice::Active,
            progressive: false,
        }],
    );
}

#[tokio::test]
async fn verb_groups_are_in_the_json() {
    let a = run(Lang::It, "Ho dato il libro a Maria.").await;
    let json = serde_json::to_value(&a).unwrap();
    assert_eq!(
        json["verb_groups"][0][0],
        serde_json::json!({
            "tokens": [0, 1],
            "auxiliaries": [0],
            "participle": 1,
            "verb": 1,
            "lemma": "dare",
            "cell": "indicative.present_perfect.1s",
            "voice": "active",
            "progressive": false,
        })
    );
}
