#![cfg(feature = "wordholder")]
//! Grammatical roles per token, against recorded API answers: the examples of
//! issue #28 and of `examples/it.txt`.

use std::path::Path;

use syntango::lang::it::Italian;
use syntango::wordholder::FixtureLexicon;
use syntango::{Role, SentenceAnalysis, TokenRoles, analyze};

use Role::*;

async fn run(fixture: &str, text: &str) -> SentenceAnalysis {
    let lexicon = FixtureLexicon::load(Path::new(fixture)).unwrap();
    analyze(&lexicon, &Italian::load().unwrap(), text)
        .await
        .unwrap()
}

async fn set(text: &str) -> SentenceAnalysis {
    run("tests/fixtures/it-set.json", text).await
}

async fn recorded(text: &str) -> SentenceAnalysis {
    run("tests/fixtures/it-recorded.json", text).await
}

fn token<'a>(a: &'a SentenceAnalysis, token: &str) -> &'a TokenRoles {
    a.roles
        .iter()
        .find(|r| r.token == token)
        .unwrap_or_else(|| panic!("no token {token}"))
}

/// Every kept parse gives each word the role listed (`None`: no role).
fn agreed(a: &SentenceAnalysis, expected: &[(&str, Option<Role>)]) {
    assert!(!a.parses.is_empty(), "no parse");
    for (word, role) in expected {
        let t = token(a, word);
        assert!(t.agreed, "{word}: parses differ: {:?}", t.roles);
        assert_eq!(t.roles, [*role], "{word}");
    }
}

/// The parses differ on `word`; the first gives `first` and some parse gives each of `also`.
fn differs(a: &SentenceAnalysis, word: &str, first: Role, also: &[Role]) {
    let t = token(a, word);
    assert!(
        !t.agreed,
        "{word}: expected parses to differ, got {:?}",
        t.roles
    );
    assert_eq!(t.roles[0], Some(first), "{word}: {:?}", t.roles);
    for r in also {
        assert!(
            t.roles.contains(&Some(*r)),
            "{word}: {r} missing from {:?}",
            t.roles
        );
    }
}

#[tokio::test]
async fn subject_verb_object() {
    let a = set("La ragazza ama il cane.").await;
    agreed(
        &a,
        &[
            ("La", Some(Subject)),
            ("ragazza", Some(Subject)),
            ("ama", Some(Verb)),
            ("il", Some(DirectObject)),
            ("cane", Some(DirectObject)),
        ],
    );
}

#[tokio::test]
async fn subject_after_the_verb() {
    let a = set("È arrivato Marco.").await;
    agreed(
        &a,
        &[
            ("È", Some(Verb)),
            ("arrivato", Some(Verb)),
            ("Marco", Some(Subject)),
        ],
    );
}

#[tokio::test]
async fn compound_and_progressive_verb_groups() {
    agreed(
        &set("Ho mangiato.").await,
        &[("Ho", Some(Verb)), ("mangiato", Some(Verb))],
    );
    agreed(
        &set("Tom sta leggendo.").await,
        &[
            ("Tom", Some(Subject)),
            ("sta", Some(Verb)),
            ("leggendo", Some(Verb)),
        ],
    );
    agreed(
        &set("Tom ha mangiato la mela.").await,
        &[
            ("ha", Some(Verb)),
            ("mangiato", Some(Verb)),
            ("la", Some(DirectObject)),
            ("mela", Some(DirectObject)),
        ],
    );
}

#[tokio::test]
async fn passive_with_an_agent() {
    let a = set("Tom è amato da tutti.").await;
    agreed(&a, &[("Tom", Some(Subject)), ("è", Some(Verb))]);
    // The grammar also reads amato as an adjective (a state) and da tutti as a
    // phrase of place or manner; every reading is reported.
    differs(&a, "amato", Verb, &[SubjectComplement]);
    differs(&a, "da", Agent, &[Adverbial]);
    differs(&a, "tutti", Agent, &[Adverbial]);
}

#[tokio::test]
async fn clitics_take_their_role_from_their_case() {
    agreed(
        &recorded("L'ho fatto.").await,
        &[
            ("L'", Some(DirectObject)),
            ("ho", Some(Verb)),
            ("fatto", Some(Verb)),
        ],
    );
    agreed(
        &set("Gli ho detto la verità.").await,
        &[
            ("Gli", Some(IndirectObject)),
            ("ho", Some(Verb)),
            ("detto", Some(Verb)),
            ("verità", Some(DirectObject)),
        ],
    );
    agreed(
        &set("Lo sto facendo.").await,
        &[("Lo", Some(DirectObject)), ("sto", Some(Verb))],
    );
    // mi is acc (it gave me) or dat (it gave to me): both parses are kept.
    let a = set("Mi ha dato un libro.").await;
    let mi = token(&a, "Mi");
    assert!(!mi.agreed);
    assert!(
        mi.roles.contains(&Some(DirectObject)) && mi.roles.contains(&Some(IndirectObject)),
        "{:?}",
        mi.roles
    );
}

#[tokio::test]
async fn a_word_with_attached_clitics_is_the_verb_group() {
    agreed(&recorded("Dammelo!").await, &[("Dammelo", Some(Verb))]);
    agreed(
        &recorded("Ricordati di me.").await,
        &[
            ("Ricordati", Some(Verb)),
            ("di", Some(Adverbial)),
            ("me", Some(Adverbial)),
        ],
    );
}

#[tokio::test]
async fn subject_complements() {
    agreed(
        &set("Le luci sono accese.").await,
        &[
            ("Le", Some(Subject)),
            ("luci", Some(Subject)),
            ("sono", Some(Verb)),
            ("accese", Some(SubjectComplement)),
        ],
    );
    let a = set("È un graffio.").await;
    agreed(&a, &[("È", Some(Verb))]);
    assert!(
        token(&a, "graffio")
            .roles
            .contains(&Some(SubjectComplement))
    );
}

#[tokio::test]
async fn object_complement() {
    let a = set("Non voglio rendere le cose peggiori.").await;
    agreed(
        &a,
        &[
            ("Non", Some(Adverbial)),
            ("voglio", Some(Verb)),
            ("rendere", Some(Verb)),
            ("le", Some(DirectObject)),
            ("cose", Some(DirectObject)),
        ],
    );
    differs(&a, "peggiori", ObjectComplement, &[DirectObject]);
}

#[tokio::test]
async fn adverbials() {
    agreed(
        &set("Ieri sono andato a casa.").await,
        &[
            ("Ieri", Some(Adverbial)),
            ("sono", Some(Verb)),
            ("andato", Some(Verb)),
            ("a", Some(Adverbial)),
            ("casa", Some(Adverbial)),
        ],
    );
    agreed(
        &set("Tom non è a casa.").await,
        &[
            ("non", Some(Adverbial)),
            ("a", Some(Adverbial)),
            ("casa", Some(Adverbial)),
        ],
    );
}

#[tokio::test]
async fn innermost_role_in_a_nested_clause() {
    let a = recorded("Disse che non riusciva a trovare un lavoro.").await;
    agreed(
        &a,
        &[
            ("Disse", Some(Verb)),
            ("che", None),
            ("non", Some(Adverbial)),
            ("riusciva", Some(Verb)),
            ("trovare", Some(Verb)),
        ],
    );
    // lavoro is trovare's object, or the subject after the verb; never part
    // of an object of disse.
    let lavoro = token(&a, "lavoro");
    assert!(
        lavoro.roles.contains(&Some(DirectObject)),
        "{:?}",
        lavoro.roles
    );
    assert!(lavoro.roles.iter().all(Option::is_some));
}

#[tokio::test]
async fn no_parse_no_roles() {
    let a = recorded("La ragazza è arrivato.").await;
    assert!(a.parses.is_empty());
    assert!(a.roles.iter().all(|r| r.roles.is_empty() && !r.agreed));
    assert_eq!(a.roles.len(), a.tokens.len());
}

#[tokio::test]
async fn serialized_in_kebab_case() {
    let a = set("Gli ho detto la verità.").await;
    assert_eq!(
        serde_json::to_value(token(&a, "Gli")).unwrap(),
        serde_json::json!({"token": "Gli", "roles": ["indirect-object"], "agreed": true})
    );
    let a = recorded("Disse che non riusciva a trovare un lavoro.").await;
    assert_eq!(
        serde_json::to_value(token(&a, "che")).unwrap()["roles"],
        serde_json::json!([null])
    );
}

#[tokio::test]
async fn spanish_and_french_have_no_roles_yet() {
    use syntango::lang::{es::Spanish, fr::French};
    let lexicon = FixtureLexicon::load(Path::new("tests/fixtures/es-recorded.json")).unwrap();
    let a = analyze(&lexicon, &Spanish::load().unwrap(), "¡Dámelo!")
        .await
        .unwrap();
    assert!(!a.parses.is_empty());
    assert!(a.roles.iter().all(|r| r.roles == [None]));
    let lexicon = FixtureLexicon::load(Path::new("tests/fixtures/fr-recorded.json")).unwrap();
    let text = std::fs::read_to_string("examples/fr.txt").unwrap();
    let first = text
        .lines()
        .find(|l| !l.trim().is_empty() && !l.starts_with('#'))
        .unwrap();
    let a = analyze(&lexicon, &French::load().unwrap(), first)
        .await
        .unwrap();
    assert!(a.roles.iter().all(|r| r.roles.iter().all(Option::is_none)));
}

#[tokio::test]
async fn contracted_prepositions_keep_their_phrase_role() {
    // dalla = da + la: the preposition and the article sit in two children
    // marked agent, and the word is the agent.
    let a = set("Tom è amato dalla ragazza.").await;
    differs(&a, "dalla", Agent, &[Adverbial]);
    differs(&a, "ragazza", Agent, &[Adverbial]);
    assert!(token(&a, "dalla").roles.iter().all(Option::is_some));
    agreed(
        &set("Tom è nella casa.").await,
        &[("nella", Some(Adverbial)), ("casa", Some(Adverbial))],
    );
    agreed(
        &set("Tom è alla stazione.").await,
        &[("alla", Some(Adverbial)), ("stazione", Some(Adverbial))],
    );
    // a + il: an adverbial phrase, or part of the object (il libro al ragazzo).
    let a = set("Ho dato il libro al ragazzo.").await;
    differs(&a, "al", Adverbial, &[DirectObject]);
    assert!(token(&a, "al").roles.iter().all(Option::is_some));
}

/// Issue #10: a verb's di phrase, a free di phrase, a postposed partitive
/// subject, and a noun phrase standing as the whole sentence (no role).
#[tokio::test]
async fn di_phrases_partitives_and_verbless_sentences() {
    let lit = |t| run("tests/fixtures/it-literature.json", t);
    agreed(&lit("Parla di me.").await, &[("Parla", Some(Verb)), ("me", Some(Adverbial))]);
    agreed(&lit("Esce di notte.").await, &[("Esce", Some(Verb)), ("notte", Some(Adverbial))]);
    agreed(&lit("Sono arrivati dei ragazzi.").await, &[("ragazzi", Some(Subject))]);
    agreed(&lit("La forza del destino!").await, &[("forza", None), ("destino", None)]);
}

/// Issue #33: an a phrase after a verb that takes a dative
/// (data/it/dative-verbs.tsv) is an indirect object, as well as an adverbial;
/// after any other verb it is only an adverbial.
#[tokio::test]
async fn a_phrase_after_a_dative_verb_is_an_indirect_object() {
    let has = |a: &SentenceAnalysis, word: &str, role: Role| {
        let t = token(a, word);
        assert!(
            t.roles.contains(&Some(role)),
            "{word}: {role} missing from {:?}",
            t.roles
        );
    };
    let a = set("Ho dato il libro a Maria.").await;
    has(&a, "a", IndirectObject);
    has(&a, "Maria", IndirectObject);
    has(&a, "Maria", Adverbial);
    // al = a + il: both halves take the phrase's role.
    let a = set("Ho detto la verità al ragazzo.").await;
    has(&a, "al", IndirectObject);
    has(&a, "ragazzo", IndirectObject);
    let a = set("Il film piace a Marco.").await;
    has(&a, "Marco", IndirectObject);
    agreed(&a, &[("film", Some(Subject)), ("piace", Some(Verb))]);
    // Without a direct object, and fronted.
    has(&set("Telefonai a Mary.").await, "Mary", IndirectObject);
    let a = set("A Maria piace il film.").await;
    has(&a, "Maria", IndirectObject);
    has(&a, "film", Subject);
    // andare does not take one: a Roma is where.
    agreed(&set("Vado a Roma.").await, &[("a", Some(Adverbial)), ("Roma", Some(Adverbial))]);
}
