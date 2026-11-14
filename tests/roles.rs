#![cfg(feature = "wordholder")]
//! Grammatical roles per token, against recorded API answers: the examples of
//! issue #28 and of `examples/it.txt`, and their Spanish and French
//! counterparts (issue #37).

use std::path::Path;

use syntango::lang::{es::Spanish, fr::French, it::Italian};
use syntango::wordholder::FixtureLexicon;
use syntango::{Role, SentenceAnalysis, TokenRoles, analyze};

use Role::*;

async fn run(fixture: &str, text: &str) -> SentenceAnalysis {
    let lexicon = FixtureLexicon::load(Path::new(fixture)).unwrap();
    analyze(&lexicon, &Italian::load().unwrap(), text)
        .await
        .unwrap()
}

async fn es(fixture: &str, text: &str) -> SentenceAnalysis {
    let lexicon = FixtureLexicon::load(Path::new(fixture)).unwrap();
    analyze(&lexicon, &Spanish::load().unwrap(), text)
        .await
        .unwrap()
}

async fn es_set(text: &str) -> SentenceAnalysis {
    es("tests/fixtures/es-set.json", text).await
}

async fn fr(text: &str) -> SentenceAnalysis {
    let lexicon = FixtureLexicon::load(Path::new("tests/fixtures/fr-set.json")).unwrap();
    analyze(&lexicon, &French::load().unwrap(), text)
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

// ------------------------------------------------------------ Spanish (#37)

#[tokio::test]
async fn es_subject_verb_object() {
    // La niña ama al perro: perro is not a person, and vp.object-personal
    // wants the object's person to differ from the verb's, so al perro is
    // only an adverbial for now; an inanimate object shows the plain shape.
    agreed(
        &es_set("La niña lee un libro.").await,
        &[
            ("La", Some(Subject)),
            ("niña", Some(Subject)),
            ("lee", Some(Verb)),
            ("un", Some(DirectObject)),
            ("libro", Some(DirectObject)),
        ],
    );
}

#[tokio::test]
async fn es_personal_a_is_a_direct_object_or_an_adverbial() {
    let a = es_set("Veo a María.").await;
    agreed(&a, &[("Veo", Some(Verb))]);
    differs(&a, "a", DirectObject, &[Adverbial]);
    differs(&a, "María", DirectObject, &[Adverbial]);
}

#[tokio::test]
async fn es_verb_groups() {
    agreed(
        &es_set("Juan está leyendo.").await,
        &[
            ("Juan", Some(Subject)),
            ("está", Some(Verb)),
            ("leyendo", Some(Verb)),
        ],
    );
    agreed(
        &es_set("Lo puedo leer.").await,
        &[
            ("Lo", Some(DirectObject)),
            ("puedo", Some(Verb)),
            ("leer", Some(Verb)),
        ],
    );
}

#[tokio::test]
async fn es_clitics_take_their_role_from_their_case() {
    agreed(
        &es_set("Lo he hecho.").await,
        &[
            ("Lo", Some(DirectObject)),
            ("he", Some(Verb)),
            ("hecho", Some(Verb)),
        ],
    );
    agreed(
        &es_set("Le dije la verdad.").await,
        &[
            ("Le", Some(IndirectObject)),
            ("dije", Some(Verb)),
            ("la", Some(DirectObject)),
            ("verdad", Some(DirectObject)),
        ],
    );
    // se for le before lo (sedat) is the indirect object.
    agreed(
        &es_set("Se lo doy.").await,
        &[
            ("Se", Some(IndirectObject)),
            ("lo", Some(DirectObject)),
            ("doy", Some(Verb)),
        ],
    );
    // me is acc or dat: both parses are kept.
    let a = es_set("Me lo dio.").await;
    let me = token(&a, "Me");
    assert!(!me.agreed);
    assert!(
        me.roles.contains(&Some(DirectObject)) && me.roles.contains(&Some(IndirectObject)),
        "{:?}",
        me.roles
    );
    agreed(&a, &[("lo", Some(DirectObject)), ("dio", Some(Verb))]);
    // Attached clitics make the word the verb group.
    agreed(
        &es("tests/fixtures/es-recorded.json", "¡Dámelo!").await,
        &[("Dámelo", Some(Verb))],
    );
}

#[tokio::test]
async fn es_subject_and_object_complements() {
    agreed(
        &es_set("Es médico.").await,
        &[("Es", Some(Verb)), ("médico", Some(SubjectComplement))],
    );
    agreed(
        &es_set("La puerta está cerrada.").await,
        &[
            ("puerta", Some(Subject)),
            ("está", Some(Verb)),
            ("cerrada", Some(SubjectComplement)),
        ],
    );
    let a = es_set("Dejó la puerta abierta.").await;
    agreed(&a, &[("Dejó", Some(Verb))]);
    differs(&a, "abierta", ObjectComplement, &[DirectObject]);
}

#[tokio::test]
async fn es_passive_with_an_agent() {
    let a = es_set("Fue amado por todos.").await;
    agreed(&a, &[("Fue", Some(Verb))]);
    // As in Italian, amado is also read as an adjective and por todos as a
    // free phrase; every reading is reported.
    differs(&a, "amado", Verb, &[SubjectComplement]);
    differs(&a, "por", Agent, &[Adverbial]);
    differs(&a, "todos", Agent, &[Adverbial]);
}

#[tokio::test]
async fn es_adverbials() {
    agreed(
        &es_set("Vivo en Madrid.").await,
        &[
            ("Vivo", Some(Verb)),
            ("en", Some(Adverbial)),
            ("Madrid", Some(Adverbial)),
        ],
    );
    agreed(
        &es_set("Juan no está en casa.").await,
        &[
            ("Juan", Some(Subject)),
            ("no", Some(Adverbial)),
            ("está", Some(Verb)),
            ("casa", Some(Adverbial)),
        ],
    );
    // An a phrase is an adverbial, even after a verb of giving or saying:
    // Spanish has no dative-verb list yet (Italian #33). Here a casa is also
    // read as a personal-a object.
    let a = es_set("Ayer fui a casa.").await;
    agreed(&a, &[("Ayer", Some(Adverbial)), ("fui", Some(Verb))]);
    differs(&a, "casa", Adverbial, &[DirectObject]);
}

// ------------------------------------------------------------- French (#37)

#[tokio::test]
async fn fr_subject_verb_object() {
    agreed(
        &fr("La fille aime le chien.").await,
        &[
            ("La", Some(Subject)),
            ("fille", Some(Subject)),
            ("aime", Some(Verb)),
            ("le", Some(DirectObject)),
            ("chien", Some(DirectObject)),
        ],
    );
}

#[tokio::test]
async fn fr_inverted_subject_and_negation() {
    agreed(
        &fr("Parlez-vous français ?").await,
        &[
            ("Parlez", Some(Verb)),
            ("-vous", Some(Subject)),
            ("français", Some(DirectObject)),
            ("?", None),
        ],
    );
    // ne … pas are adverbials inside the verb group.
    agreed(
        &fr("Je ne le vois pas.").await,
        &[
            ("Je", Some(Subject)),
            ("ne", Some(Adverbial)),
            ("le", Some(DirectObject)),
            ("vois", Some(Verb)),
            ("pas", Some(Adverbial)),
        ],
    );
}

#[tokio::test]
async fn fr_clitics_take_their_role_from_their_case() {
    agreed(
        &fr("Je l'ai fait.").await,
        &[
            ("Je", Some(Subject)),
            ("l'", Some(DirectObject)),
            ("ai", Some(Verb)),
            ("fait", Some(Verb)),
        ],
    );
    agreed(
        &fr("Je lui ai dit la vérité.").await,
        &[
            ("lui", Some(IndirectObject)),
            ("ai", Some(Verb)),
            ("dit", Some(Verb)),
            ("la", Some(DirectObject)),
            ("vérité", Some(DirectObject)),
        ],
    );
    agreed(
        &fr("Il m'a donné un livre.").await,
        &[
            ("m'", Some(IndirectObject)),
            ("donné", Some(Verb)),
            ("livre", Some(DirectObject)),
        ],
    );
    agreed(
        &fr("Donne-le-moi.").await,
        &[
            ("Donne", Some(Verb)),
            ("-le", Some(DirectObject)),
            ("-moi", Some(IndirectObject)),
        ],
    );
    // y (loc) is an adverbial; en (gen) and a reflexive belong to the verb
    // group, like Italian ne and si.
    agreed(
        &fr("J'y vais.").await,
        &[("y", Some(Adverbial)), ("vais", Some(Verb))],
    );
    agreed(
        &fr("Il y a un chat.").await,
        &[
            ("Il", Some(Subject)),
            ("y", Some(Adverbial)),
            ("a", Some(Verb)),
            ("chat", Some(DirectObject)),
        ],
    );
    agreed(
        &fr("J'en veux.").await,
        &[("en", Some(Verb)), ("veux", Some(Verb))],
    );
    agreed(
        &fr("Je me lave.").await,
        &[("me", Some(Verb)), ("lave", Some(Verb))],
    );
}

#[tokio::test]
async fn fr_subject_and_object_complements() {
    agreed(
        &fr("Il est médecin.").await,
        &[
            ("Il", Some(Subject)),
            ("est", Some(Verb)),
            ("médecin", Some(SubjectComplement)),
        ],
    );
    let a = fr("Il trouve la maison belle.").await;
    agreed(
        &a,
        &[
            ("trouve", Some(Verb)),
            ("la", Some(DirectObject)),
            ("maison", Some(DirectObject)),
        ],
    );
    differs(&a, "belle", ObjectComplement, &[DirectObject]);
}

#[tokio::test]
async fn fr_passive_with_an_agent() {
    agreed(
        &fr("Il est aimé par tous.").await,
        &[
            ("Il", Some(Subject)),
            ("est", Some(Verb)),
            ("aimé", Some(Verb)),
            ("par", Some(Agent)),
            ("tous", Some(Agent)),
        ],
    );
    // The grammar has no agent with de yet: de tous is an adverbial, and
    // aimé a passive or an adjective.
    let a = fr("Il est aimé de tous.").await;
    agreed(&a, &[("de", Some(Adverbial)), ("tous", Some(Adverbial))]);
    differs(&a, "aimé", Verb, &[SubjectComplement]);
}

#[tokio::test]
async fn fr_adverbials() {
    agreed(
        &fr("J'habite à Paris.").await,
        &[
            ("J'", Some(Subject)),
            ("habite", Some(Verb)),
            ("à", Some(Adverbial)),
            ("Paris", Some(Adverbial)),
        ],
    );
    agreed(
        &fr("Il n'est pas à la maison.").await,
        &[
            ("n'", Some(Adverbial)),
            ("est", Some(Verb)),
            ("pas", Some(Adverbial)),
            ("maison", Some(Adverbial)),
        ],
    );
}

/// A reflexive clitic has no role of its own, so it takes the role of the
/// verb group around it. Clitics and verb groups placed directly in a phrase
/// that is not itself a verb group must therefore carry a mark, or their
/// words would get no role (je me suis rendu heureux: me).
#[test]
fn clitics_and_verb_groups_outside_a_verb_group_are_marked() {
    use syntango::syntax::grammar::Grammar;
    // InfChain is not one: its rules mark each V in it.
    const VERB_GROUPS: [&str; 5] = ["V", "Cop", "SerV", "VAgr", "PassP"];
    const CLITICS: [&str; 2] = ["Clitics", "EnCl"];
    let mut unmarked = Vec::new();
    for lang in ["it", "es", "fr"] {
        let path = format!("data/{lang}/grammar.gram");
        let g = Grammar::parse(&std::fs::read_to_string(&path).unwrap()).unwrap();
        for rule in &g.rules {
            let inside = VERB_GROUPS.contains(&rule.lhs.name.as_str())
                || CLITICS.contains(&rule.lhs.name.as_str());
            if inside {
                continue;
            }
            for c in &rule.rhs {
                let n = c.name.as_str();
                if (VERB_GROUPS.contains(&n) || CLITICS.contains(&n)) && c.role.is_none() {
                    unmarked.push(format!("{lang} {}: {n}", rule.name));
                }
            }
        }
    }
    assert!(unmarked.is_empty(), "no role mark: {unmarked:#?}");
}

#[tokio::test]
async fn fr_reflexives_beside_the_verbs_join_the_verb_group() {
    let lexicon = FixtureLexicon::load(Path::new("tests/fixtures/fr-handwritten.json")).unwrap();
    let a = analyze(
        &lexicon,
        &French::load().unwrap(),
        "Je me suis rendu heureux.",
    )
    .await
    .unwrap();
    agreed(
        &a,
        &[
            ("Je", Some(Subject)),
            ("me", Some(Verb)),
            ("suis", Some(Verb)),
            ("rendu", Some(Verb)),
            ("heureux", Some(ObjectComplement)),
        ],
    );
    agreed(
        &fr("Elle se rend utile.").await,
        &[
            ("se", Some(Verb)),
            ("rend", Some(Verb)),
            ("utile", Some(ObjectComplement)),
        ],
    );
    // voici has no verb group: its pronoun is its object, whatever its case.
    let a = fr("Me voici.").await;
    assert!(token(&a, "Me").roles.iter().all(Option::is_some));
    agreed(&fr("Le voilà.").await, &[("Le", Some(DirectObject))]);
}
