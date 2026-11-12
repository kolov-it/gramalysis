# gramalysis

Morphological and syntactic analysis of sentences, with explanations.

Give it a sentence and it tells you, for every word, which dictionary entry it
comes from and what form it is, and for the sentence, which structures it can
have. Every result carries the rule that produced it, so a learner can ask *why*:

```text
$ gramalysis it "La ragazza ama il cane."
La ragazza ama il cane.
S[]  ← clause.subject
  NP[gender=f,number=sg,person=3]  ← np.article
    Det La (il definite=yes,gender=f,number=sg)
    Nom[number=sg]  ← nom.noun
      Noun ragazza (ragazza number=sg)
  VP[finite=yes,mood=ind,number=sg,person=3]  ← vp.object
    V[finite=yes,mood=ind,number=sg,person=3]  ← v.plain
      Verb ama (amare finite=yes,mood=ind,number=sg,person=3,tense=pres)
    NP[gender=m,number=sg,person=3]  ← np.article
      Det il (il definite=yes,gender=m,number=sg)
      Nom[number=sg]  ← nom.noun
        Noun cane (cane number=sg)
```

Each node shows its category, its features and the rule that built it.
`--explain` adds why each rule applies ("The verb agrees with its subject in
person and number."); `--readings` lists how each word was read and whether all
parses agree (in *Disse che non riusciva a trovare un lavoro*, `lavoro` could be
the noun *work* or the verb *I work*; only the noun fits).

Italian first; the design is multilingual from the start.

**Status:** early prototype. The Italian grammar covers simple clauses:
subject–verb agreement and the dropped subject, articles and adjectives,
clitic pronouns before and after the verb, compound tenses with *avere* and
*essere*, negation, infinitive complements and *che* clauses.

## Goal: anyone can run every example

Everything in this README runs for anyone, with a free key of their own. The
parser needs a lexicon (word form → dictionary entry and grammar); the one
used here is the [Word Holder API](https://rapidapi.com/leantyped-wordholder/api/word-holder) on RapidAPI, which has a free
plan.

### 1. Without a key: recorded answers

The repository ships the API's answers for the example sentences, so the
examples run offline:

```sh
cargo run -- --fixture tests/fixtures/it-recorded.json --file examples/it.txt it
cargo test
```

### 2. With your own key

1. Subscribe to the [Word Holder API on RapidAPI](https://rapidapi.com/leantyped-wordholder/api/word-holder). The free plan is
   enough (10 words per request).
2. Configure the backend. The key is read only from the environment, so it
   never lands in shell history; the URL and batch size can be environment
   variables or flags (`--api-url`, `--batch`):

   ```sh
   export WORDHOLDER_API_KEY=<your RapidAPI key>
   export WORDHOLDER_API_URL=https://word-holder.p.rapidapi.com
   export WORDHOLDER_API_BATCH=10
   ```

3. Run any sentence: the language code, then the sentence.

   ```sh
   cargo run -- it "La ragazza ama il cane."
   cargo run -- --explain --readings it "Dammelo!"
   cargo run -- --file examples/it.txt it
   cargo run -- --json it "Ricordati di me."   # the full analysis as JSON
   cargo test -- --ignored                     # the live API test
   ```

`WORDHOLDER_API_URL` can also point at the API's own origin or a local server;
any URL that is not a `*.rapidapi.com` host is called with
`Authorization: Bearer $WORDHOLDER_API_KEY`. `cargo run -- --help` lists every
option. Without a configured backend the tool says exactly what is missing.

To add your own sentences to the offline set, record the API's answers for them
(only grammar is kept; dictionary meanings are dropped):

```sh
cargo run -- record tests/fixtures/it-recorded.json it "Una frase nuova."
```

## Using it as a library

```
src/
  morph.rs          readings, features, the Lexicon trait
  syntax/           grammar files, morpheme lattice, Earley parser
  lang/             the Language trait; it/ = Italian (tokenizer, function
                    words, clitics, first sounds)
  analysis.rs       the pipeline: analyze(lexicon, language, sentence)
  wordholder/       the Word Holder API as a lexicon   [feature wordholder]
    client.rs         HTTP client (RapidAPI or the API's origin)
    dto.rs            the API's wire format → readings
    fixture.rs        recorded answers, for tests and offline runs
  main.rs           the command line                    [feature cli]
data/it/            closed.txt (function words), grammar.gram
```

| Feature | Contents |
|---|---|
| none | everything except `wordholder/` and the binary. No network code. |
| `wordholder` | the API client and the recorded-answers lexicon. |
| `cli` (default) | the `gramalysis` binary; implies `wordholder`. |

To bring your own lexicon, depend on the generic part only and implement
`morph::Lexicon`:

```toml
gramalysis = { git = "https://github.com/kolov-it/gramalysis", default-features = false }
```

## How it works

```
sentence → tokenizer → lexicon → morpheme lattice → Earley parser + feature grammar → parses + explanations
```

- **Lexicon** (`morph::Lexicon`): every reading of a word form; it never chooses.
- **Function words** (`data/it/closed.txt`): articles, prepositions and their
  contracted forms (*del* = *di* + *il*), pronouns, conjunctions. Kept here, not
  in the lexicon.
- **Clitics**: when the lexicon says a verb form carries clitics, they are split
  off (*dammelo* = *da* + *me* + *lo*); the grammar decides where clitics may stand
  and checks their person against the verb.
- **Grammar** (`data/it/grammar.gram`): a context-free grammar whose categories carry
  features (`NP[person=?p, number=?n]`); every rule has a name and a one-line
  explanation.
- **Parser**: Earley over the lattice, returning every parse.

Design notes: [doc/design.md](doc/design.md). What the parser needs from the
lexicon and the gaps found in the live API:
[doc/lexicon-api-needs.md](doc/lexicon-api-needs.md).

## How it differs from existing tools

- **Statistical parsers** (spaCy, Stanza, UDPipe) return one best guess and cannot
  say why. gramalysis returns every valid reading and the rule behind it.
- **Rule-based grammars** (Grammatical Framework, DELPH-IN) are the closest
  relatives, but are research systems that are heavy to adopt. gramalysis is a plain
  Rust library aimed at language learners and the tools built for them.
- **Grammar checkers** (LanguageTool) look for errors. gramalysis builds the
  structure.

## Licence

Code: dual-licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at
your option.

The recorded API answers in `tests/fixtures/` hold grammatical tags only; the
Word Holder API's data comes from Wiktionary via Kaikki.org (CC BY-SA 4.0).
