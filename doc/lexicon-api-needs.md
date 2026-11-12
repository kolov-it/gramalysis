# What the parser needs from the lexicon API

Checked against the live `POST /v1/lemmatize:batch` on 2026-11-12; the
recorded answers are in `tests/fixtures/it-recorded.json`. The API is ours, so
each gap is a change to make in wordy. Gaps marked *worked around* have a
stopgap in `src/lang/it/mod.rs` (`complete_features`) to delete once the API answers.

## Decided (owner, 2026-11-12)

- A `clitic: true` flag is enough; gramalysis splits the clitics itself.
- Articles, pronouns, prepositions and conjunctions are gramalysis's own data
  (`data/it/closed.txt`), not the lexicon's.
- Ten readings per form are enough.

## Gaps found in the live answers

| # | Gap | Example | Effect | Status |
|---|---|---|---|---|
| 1 | **One entry per lemma, not per reading.** A form with two readings of the same lemma returns only one. | `ama`: present 3sg only, the imperative 2sg is missing. `ricordati`: imperative only, the masculine plural participle is missing. | The parser cannot consider the missing reading; ambiguity it should resolve is hidden. | API change |
| 2 | **No grammar on dictionary forms.** | `ragazza`, `lavoro`, `volontariato`: `grammar: null`. | Noun gender is unknown, so agreement with articles, adjectives and participles is not checked for most nouns. | Number/form worked around; **gender needs the API** |
| 3 | **Clitic forms carry no grammar for the host verb.** | `dammelo`: `rawTags: ["compound-of"]`, no mood/person. | The host is not known to be an imperative; the clitic person rules cannot apply (2 parses instead of 1). | API change |
| 4 | **`clitic` flag missing on some clitic forms.** | `ricordati` (= ricorda + ti): `clitic: false`. | The form is not split; *ti* is lost. | API change |
| 5 | **Passato remoto has no mood.** | `fece`: tags `historic past`, no `mood`. | — | Worked around (person without mood ⇒ indicative) |
| 6 | **Participles tagged inconsistently.** | `arrivata`: `formKind: participle`; `arrivato`, `fatto`: `past-participle` with no gender/number. | — | Worked around (Italian participle ⇒ past; citation participle ⇒ masc. sg.) |

Not needed after all: segments for clitic chains (decided above), pronoun case
and article features (gramalysis's own list).

## Clitics: who does what

The lexicon says a form has clitics; gramalysis splits them (`split_enclitics`)
and gives each piece its readings from `closed.txt`. The grammar decides where
clitics may stand and in which order, and checks person: a reflexive clitic has
the verb's person, a 1st/2nd person clitic that is not reflexive has a
different one. `cammello` and `pomello` come back as nouns without `clitic`,
so they are never split.

## Status after wordy #1593 (merged 2026-11-12, not yet deployed)

Gaps 1–6 above are fixed in the API: one entry per reading, grammar on
dictionary forms (noun gender, infinitive, masculine singular), the host verb's
grammar on clitic forms, the passato remoto as indicative (`tense: preterite`),
consistent past participles. The stopgaps gramalysis had for them are removed.

Still open, in progress in wordy:

| Gap | Example | Meanwhile |
|---|---|---|
| The verb's auxiliary in compound tenses | `accendere` → avere, `salire` → both | the grammar accepts either |
| Forms only the conjugation tables know, and the inflected past participles | `pulite`, `salite`, `scorsero`, `soffrisse`, `svolta` | some are patched in `data/it/lexicon-patches/wordholder.txt` |
| Truncated infinitives named as their own lemma | `aver`, `far`, `star` | patched |
| Spanish clitic forms without host grammar (needs a data rebuild) | `dámelo` | — |
