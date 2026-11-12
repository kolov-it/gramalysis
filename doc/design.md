# gramalysis — design (draft 1, 2026-11-12)

## Goal

For a sentence in a supported language, produce:

1. **Morphology** (Italian: *analisi grammaticale*). Each token's possible readings:
   lemma, part of speech, features (tense, person, number, gender, …).
2. **Syntax** (Italian: *analisi logica*). Every structure the grammar allows for
   the sentence, and from it the reading of each token that the structure
   requires.
3. **Explanations.** Each rule that took part in a parse names itself and says,
   in plain language, what it checked.

The first user is the Word Holder Anki decks. Today a corpus sentence becomes a
card example only when the verb form fits exactly one cell of the conjugation
table, so only 71 of the 300 cards in the Italian passato remoto Core deck have
one. A parse that settles *"fece here is fare, 3rd person singular"* would let
the decks use far more sentences. That number is the first success measure.

## Non-goals (for now)

- Grammar *checking* or correction.
- Statistical disambiguation. Ranking parses comes later, and only if rules
  cannot narrow them enough.
- Speed beyond "a sentence in milliseconds".

## Pipeline

```
sentence ─▶ tokenizer ─▶ lexicon (lemmatizer) ─▶ chart parser + grammar ─▶ parses + explanations
```

### 1. Tokenizer

Splits text into tokens and punctuation. Language-specific rules for elision
and contractions that are visible in spelling: `l'ho` → `l'` `ho`,
`dell'anno` → `dell'` `anno`. Splitting *inside* a word (`dammelo`, `del`) is the
lexicon's job, because it depends on the dictionary.

### 2. Lexicon API (the lemmatizer)

```rust
pub trait Lexicon {
    /// Every reading of a surface form; empty if the form is unknown.
    fn analyze(&self, lang: Lang, form: &str) -> Result<Vec<Reading>, LexiconError>;
}

/// One reading of a token. A contraction or a verb with attached clitics is
/// one reading made of several morphemes: `dammelo` = da + me + lo.
pub struct Reading {
    pub morphemes: Vec<Morpheme>,
}

pub struct Morpheme {
    pub surface: String,     // the part of the token this morpheme covers
    pub lemma: String,       // dictionary entry: "fare"
    pub upos: Upos,          // UD part of speech: VERB, AUX, PRON, DET, ADP, …
    pub feats: Features,     // UD features: Mood=Ind|Tense=Past|Person=3|Number=Sing
}
```

- The lexicon **never chooses**. `ama` returns both *amare* present 3sg and *amare*
  imperative 2sg; the parser decides.
- Features use the **UD vocabulary** so the same grammar machinery works across
  languages and output can be compared with UD treebanks.
- Implementations:
  - an in-memory lexicon loaded from a TSV file (tests, offline use);
  - an HTTP client for a lexicon service. Word Holder's backend (wordy) will
    serve one from its Wiktionary-derived form tables:
    `GET /v1/lexicon/{lang}/analyze?form=fece` → `{"readings": [...]}` (same
    shape as `Reading`, JSON).

### 3. Grammar

A context-free grammar with **features**, one file per language. A plain CFG
cannot express agreement or a dropped subject without exploding into thousands
of rules; features keep it small:

```
# rule-name: LHS -> RHS        "explanation shown to the learner"
clause.subject:  S -> NP[Number=?n, Person=?p] VP[Number=?n, Person=?p]
                 "the verb agrees with its subject in person and number"
clause.prodrop:  S -> VP[VerbForm=Fin]
                 "Italian can leave the subject out; the verb ending carries it"
np.det-noun:     NP[Number=?n, Gender=?g, Person=3] -> DET[Number=?n, Gender=?g] NOUN[Number=?n, Gender=?g]
                 "article and noun agree in gender and number"
```

`?n` is a variable: every occurrence in a rule must unify to the same value.

Italian topics the first grammar must cover, roughly in order: agreement, null
subject, compound tenses (auxiliary + participle with agreement after *essere*),
clitic pronouns before and after the verb, prepositional complements, relative
clauses, free subject position (*è arrivato Marco*).

### 4. Parser

An **Earley chart parser** with feature unification. It accepts any CFG
(no conversion to normal form, so rule names and explanations survive), handles
ambiguity naturally, and can return every parse as a shared forest.

### 5. Output

- Per token: the readings the lexicon gave, and the ones that survive in at least
  one parse.
- Per parse: the tree, the rule name and explanation at each node, and the
  bindings that rule produced.
- A shortcut the decks need: **"every parse agrees that token *i* is reading *r*"**.
- A UD-style export (CoNLL-U) for evaluation.

## Evaluation

- **Test sentences:** start with ~50 real Italian sentences (the deck examples and
  short news sentences), each with the expected reading of its verb.
- **Treebank:** later, measure lemma/feature accuracy on the UD Italian ISDT
  treebank (CC BY-NC-SA, used only for evaluation and never included in the repo).

## Data and licences

- Code: MIT OR Apache-2.0.
- Lexicon data built from Wiktionary is CC BY-SA 4.0. It is never committed here;
  it is downloaded separately with its notice. Test fixtures use a handful of
  hand-written entries.

## Milestones

1. **Crate skeleton:** types (`Reading`, `Morpheme`, `Features`), the `Lexicon`
   trait, the TSV lexicon, the Italian tokenizer. Tests on elision.
2. **Parser:** the grammar file format, Earley with unification, simple clauses
   (agreement, null subject, direct object). Explanations in the output.
3. **Lexicon service** in wordy, and the HTTP client here.
4. **First consumer:** the Anki deck generator picks sentences by parse instead of
   by unique cell. Target: well above 71/300 cards with an example, with no wrong
   readings found in a manual check of 50.
5. **Second language** (Spanish), to prove the language boundary is in the right
   place.

## Open questions

- Grammar file syntax: the custom format above, or Rust code (a DSL/macros)?
  A text file is easier to read for linguists and learners; Rust code gets type
  checking.
- How much of the Italian clitic system goes into the lexicon (splitting
  `dammelo`) versus the grammar (where clitics may sit)?
- Unknown words: give up on the sentence, or allow a wildcard reading for
  proper names (*Tom*)?
