# syntango

(Formerly *gramalysis*.)

Morphological and syntactic analysis of sentences, with explanations.

Give it a sentence and it tells you, for every word, which dictionary entry it
comes from and what form it is, and for the sentence, which structures it can
have. Every result carries the rule that produced it, so a learner can ask *why*:

```text
$ syntango it "La ragazza ama il cane."
La ragazza ama il cane.
S[]  ← clause.subject
  NP[gender=f,number=sg,person=3]:subject  ← np.article
    Det La (il definite=yes,gender=f,next=c,number=sg)
    Nom[gender=f,number=sg]  ← nom.noun
      Noun ragazza (ragazza gender=f,number=sg)
  VP[finite=yes,form=fin,mood=ind,number=sg,person=3]  ← vp.object
    V[finite=yes,form=fin,mood=ind,number=sg,person=3]:verb  ← v.plain
      Verb ama (amare finite=yes,form=fin,mood=ind,number=sg,person=3,tense=pres)
    NP[gender=m,number=sg,person=3]:direct-object  ← np.article
      Det il (il definite=yes,gender=m,next=c,number=sg)
      Nom[gender=m,number=sg]  ← nom.noun
        Noun cane (cane gender=m,number=sg)
```

Each node shows its category, its features, its grammatical role if it has
one (`:subject`, `:verb`, `:direct-object`) and the rule that built it.
`--explain` adds why each rule applies ("The verb agrees with its subject in
person and number."); `--readings` lists how each word was read and whether all
parses agree, and for a participle how it is used (`[used as: adjective]` in
*le luci sono accese*) (in *Disse che non riusciva a trovare un lavoro*, `lavoro` could be
the noun *work* or the verb *I work*; only the noun fits).
`--roles` lists each word's grammatical role across the parses (below).

Italian first, then [Spanish](#spanish) and [French](#french); the design is multilingual from the start.

**Status:** prototype, Italian. The grammar (`data/it/grammar.gram`, 154 rules)
covers agreement and the dropped subject; articles (with the *il / lo / l'*
choice by the next word's first sound), demonstratives, quantifiers and
possessives; clitic pronouns before and after the verb; compound tenses, the
passive and *stare* + gerund; participles used like adjectives, participle
phrases and participle clauses (*Arrivato a casa, …*; *dopo mangiato*);
infinitive complements, *che* and *cui* clauses, indirect questions,
questions, subordinate clauses, commas, coordination and time expressions;
sentences without a verb (*La forza del destino!*, a noun phrase after `;` or
`:`).

On the [Tatoeba homograph set](#measuring-it-the-tatoeba-homograph-set)
(5,000 sentences built around ambiguous verb forms) it decides the ambiguous
word in **71%** of the sentences, and **99.2%** of those decisions agree with
the labels (3,347 of 3,373).

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

## Grammatical roles

Each word gets the role of the phrase it belongs to: `subject`, `verb` (the
verb group: *ha mangiato*, *sta leggendo*, *è amato*), `direct-object`,
`indirect-object`, `subject-complement` (*è **un graffio***, *le luci sono
**accese***), `object-complement` (*rendere le cose **peggiori***), `agent`
(*è amato **da tutti***) or `adverbial` (*a casa*, *ieri*, *non*); a
conjunction or anything outside a role has none.

```text
$ syntango --fixture tests/fixtures/it-set.json --roles it "Gli ho detto la verità."
…
roles (✓ = every parse agrees, - = no role):
  ✓ Gli: indirect-object
  ✓ ho: verb
  ✓ detto: verb
  ✓ la: direct-object
  ✓ verità: direct-object
```

The grammar declares them, on the children of its rules:
`S -> NP[case=nom, …]:subject VP[…]`. A word takes the role of the innermost
marked phrase that holds it, so in *Disse che non riusciva a trovare un
lavoro* `lavoro` is the object of *trovare*. A clitic pronoun marked `:case`
takes its role from its case (`case-roles acc=direct-object,
dat=indirect-object, loc=adverbial`): *l'*, *lo* are direct objects, *gli*
indirect. A word with attached clitics (*dammelo*) is the verb group. A
*di* phrase that the verb takes (*parla **di me***) or a fixed *di* phrase
(*esce **di notte***) is an adverbial. An *a* phrase after a verb that
takes a dative (*ho dato il libro **a Maria***) is an indirect object or an
adverbial, both returned. A sentence without a verb
(*La forza del destino!*) gives its words no role, and neither does a noun
phrase after `;` or `:`, which renames or lists rather than filling a role.

`SentenceAnalysis::roles` has, per token, the distinct roles the kept parses
give it (the first parse's first; `null` where a parse gives none) and
whether they all agree; parses are never ranked, so a word read two ways
keeps both roles. No parse, no roles. Italian is marked; Spanish and French
are not yet, so their words have no role.

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

The first job syntango was built for: find example sentences for verb forms
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
- A phrase right after a participle can belong to the participle or to the
  noun, and both readings are returned: *un suono soffocato di pianti* is a
  sound suffocated by crying (verb use) or a muffled sound of crying
  (adjective use).
- A **noun phrase is a sentence** too (an exclamation, a title, a list after
  `;` or `:`), next to the clause readings: *La forza del destino!* is a noun
  phrase only, *La veste di seta per mia moglie!* is both a dress of silk and
  "he dresses her in silk" (*vestire di*). A noun phrase sentence needs an
  article or another determiner, except for the nouns in
  `data/it/bare-utterance-nouns.tsv` (thanks, wishes, greetings, reactions,
  insults and endearments: *Grazie per il vostro invito!*, *Pezzo d'asino!*,
  *Buon viaggio!*); *Vivo in un appartamento* stays a clause. After `;` or `:`
  a bare noun is enough (*…: argomento in armonia con la mia natura*).
- A **di phrase after a verb** needs a verb that takes one
  (`data/it/di-verbs.tsv`: *parlare di*, *vestire di*, *morire di*; from
  Wiktionary, the labelled sentences, and a model-generated list checked by a
  second model), or a fixed
  time or manner phrase (*di notte*, *di corsa*, *di nuovo*, *uscire di casa*).
  Otherwise it belongs to the noun before it: *la porta di casa mia* is not
  "(he) carries her of my house". *di* + infinitive is not affected.
- An **a phrase after a verb that takes a dative** is an indirect object
  (`data/it/dative-verbs.tsv`: *dare*, *dire*, *mostrare*, *telefonare*,
  *piacere*; from Wiktionary, the labelled sentences, and a model-generated
  list checked by a second model): *Ho dato il libro a Maria*, *Ho detto la
  verità al ragazzo*, *Il film piace a Marco*, and fronted, *A Maria piace il
  film*. It keeps its adverbial reading as well, so both are returned; after
  any other verb it is only an adverbial (*Vado a Roma*).
- A **partitive subject after the verb** is plural (*sono arrivati dei
  ragazzi*) or follows *c'è* (*c'è del pane*); *del destino* is not "some
  destiny".

## Spanish

`src/lang/es/` and `data/es/` follow the Italian structure: tokenizer (the
opening `¿ ¡` are dropped, accents are never stripped, so *tú/tu*, *él/el*,
*sí/si*, *sé/se*, *qué/que* stay different words), function words in
`data/es/closed.txt` (*al* = *a* + *el*, *del* = *de* + *el*), grammar in
`data/es/grammar.gram`, lexicon patches in
`data/es/lexicon-patches/wordholder.txt`.

```sh
cargo run -- --fixture tests/fixtures/es-recorded.json --file examples/es.txt es
cargo run -- --fixture tests/fixtures/es-set.json --readings es "¡Dámelo!"
cargo run --release -- eval es data/es/homograph-set.jsonl \
  --labels data/es/homograph-labels.tsv --fixture tests/fixtures/es-set.json --out /tmp/eval.jsonl
```

- **Attached clitics**: the lexicon does not analyse every verb + clitics
  compound, so the host is looked up on its own: *dámelo* = *da* + *me* +
  *lo* (the accent the clitics added comes off), *vámonos* = *vamos* + *nos*,
  *sentaos* = *sentad* + *os*, *mantente* = *mantén* + *te*.
- **ser, estar and haber** have rules of their own (every verb carries
  `class=ser|estar|haber|lex`): ser says what the subject is (an adjective, a
  noun, a phrase with *de*, *para*, *en*…) or forms the passive, so *fue a
  Madrid* is *ir* and *fue difícil* is *ser*; *ha* only forms compound tenses
  (there is = *hay*).
- **Bare infinitives** only after the verbs that take them (*poder, deber,
  querer, saber, soler, ver, oír, hacer, dejar*, *gustar*…).

On the Tatoeba set (5,000 sentences, 2,034 ambiguous forms of 832 page verbs,
at most 3 sentences a form, alternating spaCy's verb and non-verb readings)
it decides the ambiguous word in **79.9%** of the sentences, and **99.5%** of
those decisions agree with the labels (3,975 of 3,995); 11.1% do not parse and
0.8% have a word the lexicon does not know. `data/es/verified-examples.tsv`
holds the 1,955 decided sentences of 5–20 words where the decision is the page
verb and the label agrees (1,207 forms of 569 verbs), in the format of wordy's
`verified-<lang>.tsv`.

**Labelling conventions** (the header of `data/es/homograph-labels.tsv` has
them in full):

- **Past participles**: *haber* + participle (which never agrees) is a
  compound tense, the verb; *ser* + agreeing participle is the passive, the
  verb (*fue construida*); *estar*, *quedar*, *parecer*, *resultar*,
  *seguir*… + participle is a state, an adjective use (*la puerta está
  cerrada*); after a noun, the verb only with the verb's own complements
  (*un libro escrito en inglés*); *lo* + participle is an adjective use.
  Adjectives whose meaning drifted (*divertido*, *complicado*, *conocido*) are
  in `data/es/drifted-participles.txt`: after *ser* they are the adjective
  (*es complicado*), a passive only with an agent (*fue exagerado por los
  medios*); after *haber* they are the verb.
- **Commands**: *usted* is `subjunctive.present.3s`, *ustedes* `…3p`, a
  negative command the subjunctive; *tú*/*vosotros* the imperative.
- **ser / ir** (*fui, fue, fueron, fuera*…): *ir* for motion or a destination,
  *ser* for what something is.
- **Accents**: a word is read as written; a missing accent that is a typo for
  another word is labelled with the intended word and noted `accent typo`
  (never an example).

## French

`cargo run -- fr "…"` parses French with `data/fr/grammar.gram` (452 rules) and
the function words in `data/fr/closed.txt`. What is French-specific:

- **Tokens:** elision is split off (*l'*, *d'*, *j'*, *qu'*, *jusqu'*,
  *lorsqu'*, *puisqu'*), *aujourd'hui* and *quelqu'un* stay whole; a pronoun
  joined to the verb by a hyphen keeps the hyphen and is attached to it
  (*parlez* *-vous*, *donne* *-le* *-moi*, *a* *-il*: the euphonic *-t-* is
  dropped), as are *-ci* and *-là* (*ce jour* *-là*). A final question mark
  stays, so inversion is a question (*Parlez-vous français ?*) and the same
  words without it an order (*Asseyez-vous.*).
- **Function words:** *du*, *des*, *au*, *aux* are both the contraction
  (*de* + *le*) and the article; *ne* … *pas / plus / jamais / rien / que*
  around the verb or the auxiliary; clitic order *me te se nous vous* > *le
  la les* > *lui leur* > *y* > *en* before the verb, *le* > *moi lui* > *y* >
  *en* after an imperative.
- **Sound:** *le / l'*, *ce / cet*, *mon amie* follow the next word's first
  sound; an aspirated h or a consonantal y (*le héros*, *la hache*, *le yoga*) counts as a consonant
  (`data/fr/aspirated-h.txt`).
- **Subject:** French says it; a clause without one is an order, or has its
  subject after the verb (inversion, *dit Tom*, *où coule le lait*).
- **Participles** follow the Italian design, with French auxiliaries:
  *avoir* + participle (which agrees only with a direct object before it:
  *je les ai vues*, *les pommes que j'ai mangées*), *être* + participle of an
  *être* verb, every pronominal verb with *être* (*elle s'est levée*), and
  a reflexive participle agreeing with the subject (*elle s'est levée*), or
  staying masculine singular when a complement follows (*elle s'est lavé les
  mains*; without transitivity data a bare *ils se sont parlé* is not
  accepted), and
  *être* + participle of an *avoir* verb a passive when marked (*par* + agent,
  *a été*, *fut*, *sera*) and otherwise a state, an adjective use (*la porte
  est fermée*). Polite *vous* takes a plural verb and a singular predicate
  (*vous êtes fatigué*): the predicate's number is kept apart from the verb's.

**The auxiliaries are not in the lexicon yet**: the Word Holder API gives
`grammar.auxiliaries` for Italian only. Until it does, every rule that selects
on the auxiliary accepts both, so *être* + participle reads both as a compound
tense and as a state, and such sentences stay ambiguous (most of the 7%
below).

On the French Tatoeba homograph set (4,384 sentences, 1,750 ambiguous forms of
699 verbs that have a page on wordholder.com and no example sentence for that
form) it decides the word in **80.9%** of the sentences, and **99.9%** of the
decisions agree with the labels (3,544 of 3,546); 11.9% do not parse, 6.8% stay
ambiguous.

```sh
cargo run --release -- eval fr data/fr/homograph-set.jsonl \
  --labels data/fr/homograph-labels.tsv --fixture tests/fixtures/fr-set.json --out /tmp/eval-fr.jsonl
cargo run -- --fixture tests/fixtures/fr-recorded.json --file examples/fr.txt fr
```

The files in `data/fr/` mirror `data/it/` (`tools/fr_homographs.py import`,
`select`), plus `verified-examples.tsv`: the 2,195 sentences (1,195 forms of
532 verbs) where syntango decides the word is the page verb and the label
agrees, 5–20 words long, in the format wordy's verb-form-examples builder
reads (`tools/fr_homographs.py verified`). Inflected participles (*vues*,
*avertis*) are left out of it: a verb table's participle cell holds only the
masculine singular.

**French labelling conventions** (the header of `homograph-labels.tsv`):
the Italian participle rules with the French auxiliaries above, plus a naming
passive (*il était appelé le grain de la pauvreté*) as the verb; an *-ant*
form is the verb (`non_finite.gerund`) with *en* or with the verb's
complements, otherwise an adjective; an imperative has no subject (*Suis-moi*),
a form with a subject or inversion is indicative or subjunctive; a pronominal
verb's lemma is the verb without *se* (*elle s'est levée*: *lever*).

The API's French answers have known defects, corrected as patches
(`data/fr/lexicon-patches/wordholder.txt`) or, where they are systematic, in
`lang/fr`: plural nouns that are also verb forms come back without the noun
(*programmes*, *critiques*); a dictionary form gets a defaulted masculine
singular (*facile*, *heureux*), which `lang/fr` leaves unknown; forms come
back as an infinitive of themselves (*grandisses*); obsolete or non-French
verbs beside the common one (*jetter*, *batter*).

## Example sentences for a verb

What syntango is for: sentences from a corpus that use a verb in a given tense,
for a verb page, with exactly where the verb is so the page can highlight it.

```sh
cargo run --release -- tenses it                 # the tenses and their identifiers (--json)
cargo run --release -- examples it essere --tense indicative.past_historic \
  --tatoeba data/it/tatoeba-pool.jsonl.gz --fixture tests/fixtures/it-set.json
cargo run --release -- examples it mangiare --tense indicative.present_perfect \
  --leipzig ita_news_2024_1M-sentences.txt --limit 50
cargo run --release -- examples es hacer --file my-sentences.txt --include-ambiguous
```

`--tense` takes a tense identifier from `syntango tenses <lang>`
(`indicative.past_historic`, `indicative.present_perfect`), one of its cells
(`indicative.present_perfect.3s`) or a non-finite form (`non_finite.gerund`);
an unknown one is an error that lists the valid ones. Without `--tense` every
cell counts. The simple tenses are the cells of the verb's table
(`data/<lang>/verbs.json`, or `--verbs FILE`); the compound tenses are named
after the auxiliary's tense they are built from:

| Identifier | Italian | Spanish | French | Auxiliary in |
|---|---|---|---|---|
| `indicative.present_perfect` | passato prossimo | pretérito perfecto compuesto | passé composé | `indicative.present` |
| `indicative.pluperfect` | trapassato prossimo | pretérito pluscuamperfecto | plus-que-parfait | `indicative.imperfect` |
| `indicative.past_anterior` | trapassato remoto | pretérito anterior | passé antérieur | `indicative.past_historic` (es `indicative.preterite`) |
| `indicative.future_perfect` | futuro anteriore | futuro compuesto | futur antérieur | `indicative.future` |
| `subjunctive.perfect` | congiuntivo passato | pretérito perfecto de subjuntivo | subjonctif passé | `subjunctive.present` |
| `subjunctive.pluperfect` | congiuntivo trapassato | pluscuamperfecto de subjuntivo | subjonctif plus-que-parfait | `subjunctive.imperfect` |
| `subjunctive.future_perfect` | | futuro compuesto de subjuntivo | | `subjunctive.future` |
| `conditional.perfect` | condizionale passato | condicional compuesto | conditionnel passé | `conditional.present` |
| `non_finite.perfect_infinitive` | infinito passato | infinitivo compuesto | infinitif passé | `non_finite.infinitive` |
| `non_finite.perfect_gerund` | gerundio passato | gerundio compuesto | participe passé composé | `non_finite.gerund` |

A cheap match picks the sentences worth parsing: a form of the verb (clitics
attached allowed: *farlo*, *dámelo*), or its past participle in any gender and
number with an auxiliary somewhere in the sentence. The parse decides. A
compound tense or a passive is named by the grammar while it parses: every
rule that joins an auxiliary and a past participle passes the auxiliary's
features up (`aux_mood`, `aux_tense`, `aux_person`, `aux_number`,
`aux_form`), and a passive rule marks `voice=passive`; Spanish `SerV` and
`PassP` pass them through. The extractor reads these complete chart items
(`Forest::items_where`) and never enumerates parses: an ambiguity such as
*abbiamo* (indicative or subjunctive) is two items over the same words. The
auxiliary's tense names the compound tense, and a participle stacked on
another (*stato*, *sido*, *été*) makes a passive compound. One JSON line per example on stdout; counts and throughput
on stderr:

```json
{"source":"my-sentences.txt","id":1,"text":"Non ho mai mangiato la pizza.","infinitive":"mangiare","lemma":"mangiare","cells":["indicative.present_perfect.1s"],"status":"decided","voice":"active","parses":1,"parts":[{"role":"auxiliary","token":1,"text":"ho","char_start":4,"char_end":6},{"role":"participle","token":3,"text":"mangiato","char_start":11,"char_end":19}],"segments":[{"text":"Non ","highlight":false},{"text":"ho","highlight":true,"role":"auxiliary"},{"text":" mai ","highlight":false},{"text":"mangiato","highlight":true,"role":"participle"},{"text":" la pizza.","highlight":false}]}
{"source":"tatoeba","id":4381,"author":"Pharamp","text":"Per farlo, bisogna correre dei rischi.","infinitive":"fare","lemma":"fare","cells":["non_finite.infinitive"],"status":"decided","voice":"active","parses":3,"parts":[{"role":"verb","token":1,"text":"farlo","char_start":4,"char_end":9,"verb":"far","clitics":["lo"]}],"segments":[{"text":"Per ","highlight":false},{"text":"farlo","highlight":true,"role":"verb"},{"text":", bisogna correre dei rischi.","highlight":false}]}
{"source":"tatoeba","id":998266,"author":"Guybrush88","text":"Cosa direste se foste al mio posto?","infinitive":"essere","lemma":"essere","cells":["indicative.past_historic.2p"],"status":"ambiguous","voice":"active","parses":4,"parts":[{"role":"verb","token":3,"text":"foste","char_start":16,"char_end":21}],"segments":[{"text":"Cosa direste se ","highlight":false},{"text":"foste","highlight":true,"role":"verb"},{"text":" al mio posto?","highlight":false}],"other_readings":["Verb essere subjunctive.imperfect.2p"]}
```

- **Where the verb is**: `parts` are its words, in order: one `verb` for a
  simple tense; the `auxiliary` (or two: *è stato*, *ha sido*, *a été*) and
  the `participle` for a compound tense or a passive. A word between them (an
  adverb, a clitic, an inverted pronoun: *ho già mangiato*, *as-tu fait*) is
  not a part. `segments` concatenate to `text`, the highlighted ones being the
  parts, so a renderer needs no offsets; each part also has
  `char_start`..`char_end` in Unicode scalar values (Rust `chars()`, Python
  string indices; JavaScript's UTF-16 indices are the same only for text
  without characters outside the Basic Multilingual Plane). A part with
  attached clitics (*farlo*, *haberlo*) has `verb`, the verb part as the parse
  split it, and `clitics`.
- **`voice`**: `passive` for the participle with *essere*/*venire*, *ser*,
  *être* (*fu scritto*, *è stato venduto*, *fue construido*, *a été levée*).
  A passive's cell is its auxiliary's: *fu scritto* is
  `indicative.past_historic.3s`, *è stato venduto* `indicative.present_perfect.3s`.
- **`status`**: `decided` when every parse reads the word as this verb in one
  cell and voice with the same auxiliaries; `ambiguous` when some parse reads
  it otherwise (`other_readings`: another word, another cell, an adjective
  use, an auxiliary). *Abbiamo già mangiato* is ambiguous: *abbiamo* is
  indicative or subjunctive. Ambiguity is reported, never resolved; only
  decided examples are printed unless `--include-ambiguous`.
- **Auxiliaries are not examples of their verb**: *ho* in *ho mangiato* is the
  auxiliary of *mangiare*, not an example of *avere*, and likewise the
  auxiliary of a progressive (`aux_progressive=yes` in the grammar): *sto* in
  *sto mangiando*, *estoy* and *he estado* in *estoy / he estado trabajando*;
  the gerund is an example of its own verb (`non_finite.gerund`). Where some
  parse reads the word as the verb itself, its example is ambiguous.
- **The cell** is named as the table names it, and a simple-tense word must
  be written as the table writes that cell: an inflected participle (*fatta*,
  *mangiati*) is not the `past_participle` cell, which holds the masculine
  singular only; in a compound tense or a passive any gender and number
  counts. A French *-ant* form read as a present participle is the table's
  `non_finite.gerund`.
- `source`, `id` and, for Tatoeba, `author` (CC BY attribution) say where the
  sentence comes from. Tatoeba input is JSON lines with `id`, `author`, `text`;
  Leipzig input is the upstream `id<TAB>sentence` file; `--file` takes one
  sentence a line (the id is the line number). A `.gz` file is read as gzip.

**Not yet supported:** the perfect imperative (*abbi fatto*, *aie fini*), and
*stare* / *estar* + gerund as a tense of its own.

As a library (`examples::Extractor`; `sentences` is any iterator of
`Result<Sentence, BoxError>`, `extractor.examples(&sentence)` does one;
`examples::tenses::tenses(lang)` is the tense list):

```rust
let tables: VerbTables = serde_json::from_str(&std::fs::read_to_string("data/it/verbs.json")?)?;
let query = Query { infinitive: "essere".to_owned(), tense: Some("indicative.past_historic".to_owned()) };
let extractor = Extractor::new(&lexicon, &italian, &tables, &query)?;
let options = Options { limit: Some(20), include_ambiguous: false };
let stats = extractor
    .run(sentences, &options, |e| {
        let marked: String = e.segments.iter().map(|s| if s.highlight { format!("[{}]", s.text) } else { s.text.clone() }).collect();
        println!("{marked}");
        Ok(())
    })
    .await?;
```

## Using it as a library

```
src/
  morph.rs          readings, features, the Lexicon trait
  patch.rs          patches for a lexicon's known defects (PatchedLexicon)
  syntax/           grammar files, morpheme lattice, Earley parser (shared
                    parse forest: all parses, polynomial work)
  lang/             the Language trait; it/ = Italian (tokenizer, function
                    words, clitics, first sounds, participles); es/ = Spanish
  analysis.rs       the pipeline: analyze(lexicon, language, sentence)
  verbs.rs          what a verb page asks of a reading: base verb, participle use
  examples/         example sentences for a verb: Extractor, the tense list
                    (tenses.rs), verb tables and cells (table.rs), tokens
                    back to text offsets (align.rs)
  wordholder/       Word Holder's lemmatize contract   [feature wordholder]
    client.rs         HTTP client (RapidAPI or the API's origin)
                                                     [feature wordholder-http]
    dto.rs            the API's wire format → readings (public)
    fixture.rs        recorded answers, for tests and offline runs
  bin/syntango/     the command line: parse, record, eval, tenses, examples  [feature cli]
data/it/            grammar.gram, closed.txt (function words),
                    drifted-participles.txt, lexicon-patches/, the Tatoeba set
data/es/            the same for Spanish
tools/              it_homographs.py: builds the Tatoeba set (--lang es for
                    Spanish); verified_examples.py: decided sentences that
                    agree with their labels, for wordy
```

| Feature | Contains | Adds dependencies |
|---|---|---|
| *(none)* | `Lexicon` trait, languages, grammars, parser, `analyze`, `PatchedLexicon` and the patch-file format | — |
| `wordholder` | Word Holder's lemmatize contract: the wire types and their conversion to readings (`analyses_from_batch`), the Word Holder patch files and `wordholder::patches()`, `FixtureLexicon` | `serde_json` |
| `wordholder-http` | `WordHolderLexicon`, the HTTP client (RapidAPI or origin); implies `wordholder` | `reqwest` |
| `cli` (default) | the `syntango` binary; implies `wordholder-http` | `tokio`, `clap` |

Library users take `default-features = false` plus the feature they need:

- **Your own lexicon:** no feature. Implement `morph::Lexicon`; `PatchedLexicon`
  works with a patch file of your own.
- **Word Holder data in process, or offline tests:** `wordholder`. Produce a
  `/v1/lemmatize:batch`-shaped answer however you like (`BatchResponse::new`,
  or deserialize the JSON), turn it into readings with `analyses_from_batch`,
  and wrap your lexicon in `PatchedLexicon` with `wordholder::patches()`; or
  use `FixtureLexicon` on recorded answers. The patches are written against
  the readings this conversion produces, so use them together.
- **Word Holder over HTTP:** `wordholder-http`.

```toml
syntango = { git = "https://github.com/kolov-it/syntango", default-features = false, features = ["wordholder"] }
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
  say why. syntango returns every valid reading and the rule behind it.
- **Rule-based grammars** (Grammatical Framework, DELPH-IN) are the closest
  relatives, but are research systems that are heavy to adopt. syntango is a plain
  Rust library aimed at language learners and the tools built for them.
- **Grammar checkers** (LanguageTool) look for errors. syntango builds the
  structure.

## Licence

[Apache License 2.0](LICENSE). Use it freely, commercially too, with
attribution: anything that distributes syntango or a work derived from it must
include the [NOTICE](NOTICE) file's attribution, where third-party notices
normally appear.

The data files under `data/` keep their sources' licences, listed in
[data/it/NOTICE.md](data/it/NOTICE.md) and [data/es/NOTICE.md](data/es/NOTICE.md): Tatoeba sentences under CC BY 2.0 FR,
Wiktionary-derived tables under CC BY-SA 4.0. The recorded API answers in
`tests/fixtures/` hold grammatical tags only.
