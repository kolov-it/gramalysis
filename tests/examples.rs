#![cfg(feature = "wordholder")]
//! Example extraction on Tatoeba sentences, against the recorded answers for
//! the homograph sets (tests/fixtures/<lang>-set.json).

use std::collections::BTreeSet;
use std::io::{BufRead, BufReader};
use std::path::Path;

use flate2::read::GzDecoder;

use syntango::examples::align::token_spans;
use syntango::examples::tenses::{Kind, tense_of, tenses};
use syntango::examples::{
    Example, Extractor, Options, Query, Role, Sentence, Status, VerbTables, Voice,
};
use syntango::lang;
use syntango::morph::Lang;
use syntango::wordholder::FixtureLexicon;

fn tables(lang: &str) -> VerbTables {
    let file = std::fs::File::open(format!("data/{lang}/verbs.json")).unwrap();
    serde_json::from_reader(BufReader::new(file)).unwrap()
}

async fn run_in(
    code: &str,
    infinitive: &str,
    tense: Option<&str>,
    texts: &[&str],
    options: Options,
) -> (Vec<Example>, u64) {
    let lexicon =
        FixtureLexicon::load(Path::new(&format!("tests/fixtures/{code}-set.json"))).unwrap();
    let language = lang::load(code.parse::<Lang>().unwrap()).unwrap();
    let query = Query {
        infinitive: infinitive.to_owned(),
        tense: tense.map(str::to_owned),
    };
    let extractor = Extractor::new(&lexicon, language.as_ref(), &tables(code), &query).unwrap();
    let mut found = Vec::new();
    let sentences = texts.iter().enumerate().map(|(i, t)| {
        Ok(Sentence {
            source: "tatoeba".to_owned(),
            id: i as u64 + 1,
            author: Some("test".to_owned()),
            text: (*t).to_owned(),
        })
    });
    let stats = extractor
        .run(sentences, &options, |e| {
            found.push(e.clone());
            Ok(())
        })
        .await
        .unwrap();
    (found, stats.sentences)
}

async fn one(code: &str, infinitive: &str, tense: Option<&str>, text: &str) -> Example {
    let (found, _) = run_in(code, infinitive, tense, &[text], all()).await;
    let [e] = found.as_slice() else {
        panic!("{found:?}")
    };
    // The segments are the sentence; the highlighted ones are the parts.
    assert_eq!(
        e.segments
            .iter()
            .map(|s| s.text.as_str())
            .collect::<String>(),
        e.text
    );
    let highlighted: Vec<&str> = e
        .segments
        .iter()
        .filter(|s| s.highlight)
        .map(|s| s.text.as_str())
        .collect();
    assert_eq!(
        highlighted,
        e.parts.iter().map(|p| p.text.as_str()).collect::<Vec<_>>()
    );
    for p in &e.parts {
        assert_eq!(
            e.text
                .chars()
                .skip(p.char_start)
                .take(p.char_end - p.char_start)
                .collect::<String>(),
            p.text
        );
    }
    e.clone()
}

/// The sentence with the parts in brackets.
fn marked(e: &Example) -> String {
    e.segments
        .iter()
        .map(|s| {
            if s.highlight {
                format!("[{}]", s.text)
            } else {
                s.text.clone()
            }
        })
        .collect()
}

fn roles(e: &Example) -> Vec<Role> {
    e.parts.iter().map(|p| p.role).collect()
}

fn all() -> Options {
    Options {
        limit: None,
        include_ambiguous: true,
    }
}

#[tokio::test]
async fn simple_tense_with_its_span() {
    let e = one(
        "it",
        "essere",
        Some("indicative.past_historic"),
        "Tutti furono d'accordo con la sua idea.",
    )
    .await;
    assert_eq!(
        (e.cells.as_slice(), e.status, e.voice),
        (
            ["indicative.past_historic.3p".to_owned()].as_slice(),
            Status::Decided,
            Voice::Active
        )
    );
    assert_eq!(marked(&e), "Tutti [furono] d'accordo con la sua idea.");
    assert_eq!(
        (
            e.parts[0].char_start,
            e.parts[0].char_end,
            e.parts[0].token,
            e.parts[0].role
        ),
        (6, 12, 1, Role::Verb)
    );
}

#[tokio::test]
async fn a_form_another_cell_shares_is_ambiguous() {
    let texts = ["Cosa direste se foste al mio posto?"];
    let e = one("it", "essere", Some("indicative.past_historic"), texts[0]).await;
    assert_eq!(e.status, Status::Ambiguous);
    assert_eq!(e.cells, ["indicative.past_historic.2p"]);
    assert_eq!(e.other_readings, ["Verb essere subjunctive.imperfect.2p"]);
    let (found, _) = run_in(
        "it",
        "essere",
        Some("indicative.past_historic"),
        &texts,
        Options::default(),
    )
    .await;
    assert!(found.is_empty(), "{found:?}");
}

#[tokio::test]
async fn enclitic_verb_names_its_verb_part() {
    let e = one("it", "fare", None, "Per farlo, bisogna correre dei rischi.").await;
    assert_eq!(marked(&e), "Per [farlo], bisogna correre dei rischi.");
    assert_eq!(
        (e.parts[0].verb.as_deref(), e.parts[0].clitics.as_slice()),
        (Some("far"), ["lo".to_owned()].as_slice())
    );
    assert_eq!(
        (e.cells.as_slice(), e.status),
        (
            ["non_finite.infinitive".to_owned()].as_slice(),
            Status::Decided
        )
    );
}

#[tokio::test]
async fn italian_compound_tense_skips_the_adverb() {
    let e = one("it", "mangiare", None, "Non ho mai mangiato la pizza.").await;
    assert_eq!(marked(&e), "Non [ho] mai [mangiato] la pizza.");
    assert_eq!(roles(&e), [Role::Auxiliary, Role::Participle]);
    assert_eq!(
        (e.cells.as_slice(), e.status, e.voice),
        (
            ["indicative.present_perfect.1s".to_owned()].as_slice(),
            Status::Decided,
            Voice::Active
        )
    );
    // The participle in a compound tense is not the bare participle cell.
    let (found, _) = run_in(
        "it",
        "mangiare",
        Some("non_finite.past_participle"),
        &["Non ho mai mangiato la pizza."],
        all(),
    )
    .await;
    assert!(found.is_empty(), "{found:?}");
}

#[tokio::test]
async fn italian_auxiliary_mood_ambiguity_is_reported() {
    let e = one(
        "it",
        "mangiare",
        Some("indicative.present_perfect"),
        "Abbiamo già mangiato.",
    )
    .await;
    assert_eq!(marked(&e), "[Abbiamo] già [mangiato].");
    assert_eq!(e.status, Status::Ambiguous);
    let e = one("it", "mangiare", None, "Abbiamo già mangiato.").await;
    assert_eq!(
        e.cells,
        ["indicative.present_perfect.1p", "subjunctive.perfect.1p"]
    );
}

#[tokio::test]
async fn italian_clitic_before_the_auxiliary() {
    let e = one("it", "fare", None, "Non so se l'avrebbe fatto per me.").await;
    assert_eq!(marked(&e), "Non so se l'[avrebbe] [fatto] per me.");
    assert_eq!(e.cells, ["conditional.perfect.3s"]);
}

#[tokio::test]
async fn italian_passive_of_a_simple_tense() {
    let e = one("it", "scrivere", None, "Il libro fu scritto da lui.").await;
    assert_eq!(marked(&e), "Il libro [fu] [scritto] da lui.");
    assert_eq!(
        (e.cells.as_slice(), e.voice),
        (
            ["indicative.past_historic.3s".to_owned()].as_slice(),
            Voice::Passive
        )
    );
    // Some parses read fu as the auxiliary, others as the copula before an
    // adjective: for essere it is ambiguous.
    let e = one("it", "essere", None, "Il libro fu scritto da lui.").await;
    assert_eq!(e.status, Status::Ambiguous);
    assert!(
        e.other_readings
            .iter()
            .any(|r| r.starts_with("auxiliary of scrivere")),
        "{:?}",
        e.other_readings
    );
}

#[tokio::test]
async fn spanish_perfect_infinitive_with_an_enclitic() {
    let e = one(
        "es",
        "hacer",
        None,
        "No podría haberlo hecho sin ti. Gracias.",
    )
    .await;
    assert_eq!(marked(&e), "No podría [haberlo] [hecho] sin ti. Gracias.");
    assert_eq!(
        (e.parts[0].verb.as_deref(), e.parts[0].clitics.as_slice()),
        (Some("haber"), ["lo".to_owned()].as_slice())
    );
    assert_eq!(
        (e.cells.as_slice(), e.status),
        (
            ["non_finite.perfect_infinitive".to_owned()].as_slice(),
            Status::Decided
        )
    );
}

#[tokio::test]
async fn spanish_passive_and_proclitic_perfect() {
    let e = one(
        "es",
        "construir",
        None,
        "El puente fue construido por los romanos.",
    )
    .await;
    assert_eq!(marked(&e), "El puente [fue] [construido] por los romanos.");
    assert_eq!(
        (e.cells.as_slice(), e.voice),
        (
            ["indicative.preterite.3s".to_owned()].as_slice(),
            Voice::Passive
        )
    );
    let e = one("es", "escribir", None, "No, aún no lo he escrito.").await;
    assert_eq!(marked(&e), "No, aún no lo [he] [escrito].");
    assert_eq!(e.cells, ["indicative.present_perfect.1s"]);
}

#[tokio::test]
async fn french_compound_tenses_with_adverbs() {
    let e = one(
        "fr",
        "faire",
        None,
        "Ça m'a presque fait peur de ne pas te voir connectée pendant toute une journée.",
    )
    .await;
    assert_eq!(
        marked(&e),
        "Ça m'[a] presque [fait] peur de ne pas te voir connectée pendant toute une journée."
    );
    assert_eq!(e.cells, ["indicative.present_perfect.3s"]);
    let e = one(
        "fr",
        "faire",
        None,
        "Les humains n'ont jamais été faits pour vivre éternellement.",
    )
    .await;
    assert_eq!(
        marked(&e),
        "Les humains n'[ont] jamais [été] [faits] pour vivre éternellement."
    );
    assert_eq!(
        roles(&e),
        [Role::Auxiliary, Role::Auxiliary, Role::Participle]
    );
    assert_eq!(
        (e.cells.as_slice(), e.voice),
        (
            ["indicative.present_perfect.3p".to_owned()].as_slice(),
            Voice::Passive
        )
    );
    let e = one(
        "fr",
        "faire",
        None,
        "Comment vas-tu ? As-tu fait un bon voyage ?",
    )
    .await;
    assert_eq!(
        marked(&e),
        "Comment vas-tu ? [As]-tu [fait] un bon voyage ?"
    );
}

#[tokio::test]
async fn sentences_without_the_form_are_not_parsed_and_limit_stops() {
    let texts = [
        "Il gatto dorme.",
        "Tutti furono d'accordo con la sua idea.",
        "Fui molto contenta di vederlo ritornare.",
        "Cosa direste se foste al mio posto?",
    ];
    let options = Options {
        limit: Some(1),
        include_ambiguous: false,
    };
    let (found, read) = run_in(
        "it",
        "essere",
        Some("indicative.past_historic"),
        &texts,
        options,
    )
    .await;
    assert_eq!(
        found
            .iter()
            .map(|e| (e.id, e.parts[0].text.as_str()))
            .collect::<Vec<_>>(),
        [(2, "furono")]
    );
    assert_eq!(read, 2);
}

/// The tense list names every tense the verb tables have, and no other
/// simple tense.
#[test]
fn tense_list_matches_the_tables() {
    for code in ["it", "es", "fr"] {
        let lang: Lang = code.parse().unwrap();
        let mut in_tables = BTreeSet::new();
        for verb in tables(code).verbs {
            for (mood, ts) in verb.conjugation {
                for tense in ts.keys() {
                    in_tables.insert(format!("{mood}.{tense}"));
                }
            }
        }
        let simple: BTreeSet<String> = tenses(lang)
            .iter()
            .filter(|t| t.kind == Kind::Simple)
            .map(|t| t.id.to_owned())
            .collect();
        assert_eq!(simple, in_tables, "{code}");
        for t in tenses(lang).iter().filter(|t| t.kind == Kind::Compound) {
            assert!(
                in_tables.contains(t.auxiliary_tense.unwrap()),
                "{code} {}",
                t.id
            );
            assert_eq!(tense_of(t.id), t.id);
        }
    }
}

/// Every sentence of the three Tatoeba pools aligns: each token is found in
/// the original text, in order.
#[test]
fn every_tatoeba_pool_sentence_aligns() {
    for code in ["it", "es", "fr"] {
        let language = lang::load(code.parse::<Lang>().unwrap()).unwrap();
        let file = std::fs::File::open(format!("data/{code}/tatoeba-pool.jsonl.gz")).unwrap();
        let mut n = 0;
        for line in BufReader::new(GzDecoder::new(file)).lines() {
            let row: serde_json::Value = serde_json::from_str(&line.unwrap()).unwrap();
            let text = row["text"].as_str().unwrap();
            let tokens = language.tokenize(text);
            let spans = token_spans(text, &tokens).unwrap_or_else(|e| panic!("{code}: {e}"));
            assert!(
                spans.windows(2).all(|w| w[0].end <= w[1].start),
                "{code}: {text}"
            );
            n += 1;
        }
        assert!(n > 10_000, "{code}: {n}");
    }
}
