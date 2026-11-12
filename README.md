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
    Det La (il definite=yes,gender=f,next=c,number=sg)
    Nom[gender=f,number=sg]  ← nom.noun
      Noun ragazza (ragazza gender=f,number=sg)
  VP[finite=yes,form=fin,mood=ind,number=sg,person=3]  ← vp.object
    V[finite=yes,form=fin,mood=ind,number=sg,person=3]  ← v.plain
      Verb ama (amare finite=yes,form=fin,mood=ind,number=sg,person=3,tense=pres)
    NP[gender=m,number=sg,person=3]  ← np.article
      Det il (il definite=yes,gender=m,next=c,number=sg)
      Nom[gender=m,number=sg]  ← nom.noun
        Noun cane (cane gender=m,number=sg)
```

Each node shows its category, its features and the rule that built it.
`--explain` adds why each rule applies ("The verb agrees with its subject in
person and number."); `--readings` lists how each word was read and whether all
parses agree, and for a participle how it is used (`[used as: adjective]` in
*le luci sono accese*) (in *Disse che non riusciva a trovare un lavoro*, `lavoro` could be
the noun *work* or the verb *I work*; only the noun fits).

Italian first; the design is multilingual from the start.

**Status:** prototype, Italian. The grammar (`data/it/grammar.gram`, 115 rules)
covers agreement and the dropped subject; articles (with the *il / lo / l'*
choice by the next word's first sound), demonstratives, quantifiers and
possessives; clitic pronouns before and after the verb; compound tenses, the
passive and *stare* + gerund; participles used like adjectives and participle
phrases; infinitive complements, *che* and *cui* clauses, indirect questions,
questions, subordinate clauses, commas, coordination and time expressions.

On the [Tatoeba homograph set](#measuring-it-the-tatoeba-homograph-set)
(5,000 sentences built around ambiguous verb forms) it decides the ambiguous
word in **66%** of the sentences, and **99.7%** of those decisions agree with
the labels (2,366 of 2,374).

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
cargo run -- --fixture tests/fixtures/it-set.json it "Io corsi a casa."   # any word of the Tatoeba set
cargo test
```

`tests/fixtures/it-set.json` holds the answers for every word of the
[Tatoeba homograph set](#measuring-it-the-tatoeba-homograph-set), about 6,400
forms.

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
(only grammar is kept; dictionary meanings are dropped). `record` skips words
already in the file and saves after every request, so a run stopped by a rate
limit continues where it stopped:

```sh
cargo run -- record tests/fixtures/it-recorded.json it "Una frase nuova."
cargo run -- record tests/fixtures/it-set.json it --set data/it/homograph-set.jsonl
```

## Lexicon patches

Every lexicon has defects: a reading it lacks (*scorsero* without *scorrere*), a
lemma it names wrongly (*aver* for *avere*). Corrections are data, not parser
code: `data/<lang>/lexicon-patches/<lexicon>.txt`, one line each with its reason,
applied on top of the lexicon by `patch::PatchedLexicon`.

```text
scorsero  +  scorrere  verb  mood=indicative,tense=preterite,person=third,number=plural  # only scorgere is returned
aver      =  avere     verb  form=infinitive                                             # returned as lemma "aver"
star      -  star      verb                                                              # the noun (una star) stays
```

`+` adds a reading, `=` replaces all readings, `-` removes one. The CLI and
`eval` apply the patches; `--no-patches` shows the lexicon as it is. Delete a
line once the lexicon answers correctly.

## Measuring it: the Tatoeba homograph set

The first job gramalysis was built for: find example sentences for verb forms
that are spelled like another word (*stessi*: *stare* or *stesso*; *porta*:
*portare* or the noun), which plain text matching cannot tell apart.

| File (`data/it/`) | What |
|---|---|
| `homograph-set.jsonl` | 5,000 Tatoeba sentences, each with one ambiguous word, its possible readings and spaCy's guess |
| `homograph-labels.tsv` | the correct reading of the word (verb, form and cell, or what else it is), with who labelled and who checked it |
| `ambiguous-forms.tsv`, `homographs.tsv`, `form-other-verbs.tsv`, `page-example-forms.tsv`, `verbs.json` | why each form is ambiguous; the 863 verb pages' tables; the forms that already have an example |
| `tatoeba-pool.jsonl.gz` | the 16,583 candidate sentences the set was drawn from |

`tools/it_homographs.py import` rebuilds these from their sources;
`tools/it_homographs.py select` draws the set from the files here alone.

```sh
cargo run --release -- eval it data/it/homograph-set.jsonl \
  --labels data/it/homograph-labels.tsv --fixture tests/fixtures/it-set.json --out /tmp/eval.jsonl
```

reports how many sentences hit an unknown word, do not parse, stay ambiguous or
are decided, and how many decisions match the labels (spaCy's guess stands in
for unlabelled sentences, but it is a poor reference). The question scored is
the one a verb page needs: *which verb, or not a verb at all*.

**Labelling conventions** (owner decisions):

- A **past participle** is one form of its verb, whether used as a verb or like
  an adjective; the grammar decides its use. After *essere*, the verb's
  **auxiliary** decides: *sono salite* (*salire* takes *essere*) is a compound
  tense; *le luci sono accese* (*accendere* takes *avere*) is a state, an
  adjective use, unless a passive marker is there (*venire*, *è stato*,
  *da* + agent, *essere* in the passato remoto or future). Reflexives take
  *essere* with any verb (*mi sono alzato*). The auxiliary comes from the
  lexicon (`grammar.auxiliaries`).
- **Adjectives whose meaning drifted** from the verb stay adjectives:
  `data/it/drifted-participles.txt` (*esatto* is "exact", not *esigere*'s
  participle).
- A **present participle** is an adjective (*pesante*, *corrente*) unless it
  carries the verb's complements (*il fiume passante per Parigi*).

## Using it as a library

```
src/
  morph.rs          readings, features, the Lexicon trait
  patch.rs          patches for a lexicon's known defects (PatchedLexicon)
  syntax/           grammar files, morpheme lattice, Earley parser (shared
                    parse forest: all parses, polynomial work)
  lang/             the Language trait; it/ = Italian (tokenizer, function
                    words, clitics, first sounds, participles)
  analysis.rs       the pipeline: analyze(lexicon, language, sentence)
  wordholder/       the Word Holder API as a lexicon   [feature wordholder]
    client.rs         HTTP client (RapidAPI or the API's origin)
    dto.rs            the API's wire format → readings
    fixture.rs        recorded answers, for tests and offline runs
  bin/gramalysis/   the command line: parse, record, eval  [feature cli]
data/it/            grammar.gram, closed.txt (function words),
                    drifted-participles.txt, lexicon-patches/, the Tatoeba set
tools/              it_homographs.py: builds the Tatoeba set
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
- **Participles**: one reading, the verb's participle; the grammar decides whether
  it is a compound tense, a passive, a participle phrase or an adjective use.
- **Grammar** (`data/it/grammar.gram`): a context-free grammar whose categories carry
  features (`NP[person=?p, number=?n]`); every rule has a name and a one-line
  explanation.
- **Parser**: Earley over the lattice with a shared parse forest: every parse is
  counted, the first ones are kept as trees, and the readings any parse uses are
  reported per word.

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

[Apache License 2.0](LICENSE). Use it freely, commercially too, with
attribution: anything that distributes gramalysis or a work derived from it must
include the [NOTICE](NOTICE) file's attribution, where third-party notices
normally appear.

The data files under `data/` keep their sources' licences, listed in
[data/it/NOTICE.md](data/it/NOTICE.md): Tatoeba sentences under CC BY 2.0 FR,
Wiktionary-derived tables under CC BY-SA 4.0. The recorded API answers in
`tests/fixtures/` hold grammatical tags only.
