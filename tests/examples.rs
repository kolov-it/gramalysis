#![cfg(feature = "wordholder")]
//! Example extraction on Tatoeba sentences, against the recorded answers for
//! the Italian homograph set (tests/fixtures/it-set.json).

use std::io::{BufRead, BufReader};
use std::path::Path;

use flate2::read::GzDecoder;

use syntango::examples::align::token_spans;
use syntango::examples::{Example, Extractor, Options, Query, Sentence, Status, VerbTables};
use syntango::lang;
use syntango::lang::it::Italian;
use syntango::morph::Lang;
use syntango::wordholder::FixtureLexicon;

fn tables(lang: &str) -> VerbTables {
    let file = std::fs::File::open(format!("data/{lang}/verbs.json")).unwrap();
    serde_json::from_reader(BufReader::new(file)).unwrap()
}

fn sentence(id: u64, text: &str) -> Sentence {
    Sentence {
        source: "tatoeba".to_owned(),
        id,
        author: Some("test".to_owned()),
        text: text.to_owned(),
    }
}

async fn run(
    infinitive: &str,
    tense: Option<&str>,
    texts: &[(u64, &str)],
    options: Options,
) -> (Vec<Example>, u64) {
    let lexicon = FixtureLexicon::load(Path::new("tests/fixtures/it-set.json")).unwrap();
    let italian = Italian::load().unwrap();
    let query = Query {
        infinitive: infinitive.to_owned(),
        tense: tense.map(str::to_owned),
    };
    let extractor = Extractor::new(&lexicon, &italian, &tables("it"), &query).unwrap();
    let mut found = Vec::new();
    let sentences = texts.iter().map(|(id, t)| Ok(sentence(*id, t)));
    let stats = extractor
        .run(sentences, &options, |e| {
            found.push(e.clone());
            Ok(())
        })
        .await
        .unwrap();
    (found, stats.sentences)
}

fn all() -> Options {
    Options {
        limit: None,
        include_ambiguous: true,
    }
}

#[tokio::test]
async fn passato_remoto_of_essere_with_its_span() {
    let (found, _) = run(
        "essere",
        Some("indicative.past_historic"),
        &[(375428, "Mi fu chiesto di riparare la bicicletta.")],
        all(),
    )
    .await;
    let [e] = found.as_slice() else {
        panic!("{found:?}")
    };
    assert_eq!(e.cells, ["indicative.past_historic.3s"]);
    assert_eq!(e.status, Status::Decided);
    assert_eq!(
        (e.before.as_str(), e.form.as_str(), e.after.as_str()),
        ("Mi ", "fu", " chiesto di riparare la bicicletta.")
    );
    assert_eq!((e.char_start, e.char_end, e.token), (3, 5, 1));
    assert_eq!((e.id, e.author.as_deref()), (375428, Some("test")));
}

#[tokio::test]
async fn a_form_another_cell_shares_is_ambiguous() {
    let texts = [(1, "Cosa direste se foste al mio posto?")];
    let (found, _) = run("essere", Some("indicative.past_historic"), &texts, all()).await;
    let [e] = found.as_slice() else {
        panic!("{found:?}")
    };
    assert_eq!(e.status, Status::Ambiguous);
    assert_eq!(e.cells, ["indicative.past_historic.2p"]);
    assert_eq!(e.other_readings, ["Verb essere subjunctive.imperfect.2p"]);
    let (found, _) = run(
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
    let (found, _) = run(
        "fare",
        None,
        &[(4381, "Per farlo, bisogna correre dei rischi.")],
        all(),
    )
    .await;
    let [e] = found.as_slice() else {
        panic!("{found:?}")
    };
    assert_eq!(
        (e.form.as_str(), e.verb.as_deref(), e.clitics.as_slice()),
        ("farlo", Some("far"), ["lo".to_owned()].as_slice())
    );
    assert_eq!(e.cells, ["non_finite.infinitive"]);
    assert_eq!(e.status, Status::Decided);
}

#[tokio::test]
async fn sentences_without_the_form_are_not_parsed_and_limit_stops() {
    let texts = [
        (1, "Il gatto dorme."),
        (2, "Fu condannato a morte."),
        (3, "Tutti furono d'accordo con la sua idea."),
        (4, "Mi fu chiesto di riparare la bicicletta."),
    ];
    let (found, read) = run(
        "essere",
        Some("indicative.past_historic"),
        &texts,
        Options {
            limit: Some(2),
            include_ambiguous: false,
        },
    )
    .await;
    assert_eq!(
        found
            .iter()
            .map(|e| (e.id, e.form.as_str()))
            .collect::<Vec<_>>(),
        [(2, "Fu"), (3, "furono")]
    );
    assert_eq!(read, 3);
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
