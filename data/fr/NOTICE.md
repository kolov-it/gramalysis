# Data in `data/fr/` and its licences

The code of syntango is Apache-2.0 (see ../../NOTICE). The data files below are not code
and keep their sources' licences.

| File | Source | Licence |
|---|---|---|
| `closed.txt`, `grammar.gram`, `aspirated-h.txt`, `drifted-participles.txt`, `lexicon-patches/wordholder.txt` | written for syntango | Apache-2.0 |
| `homograph-labels.tsv` | labels written for syntango (model labellers, conventions in its header), keyed by Tatoeba id | Apache-2.0 |
| `tatoeba-pool.jsonl.gz`, `homograph-set.jsonl` (the `text` field of each sentence) | [Tatoeba](https://tatoeba.org), French sentences, export of 2026-10-09. Each sentence keeps its Tatoeba id (`id` / `tatoeba_id`, page `https://tatoeba.org/sentences/show/<id>`) and its author (`author`). No translation is copied. | [CC BY 2.0 FR](https://creativecommons.org/licenses/by/2.0/fr/) |
| the `spacy` / `tokens` fields of those files | tokens, part-of-speech tags and lemmas from spaCy `fr_core_news_md` 3.8.0 run over the Tatoeba sentences | as the sentences |
| `verbs.json` | French conjugation tables from [Wiktionary](https://en.wiktionary.org/) via [Kaikki.org](https://kaikki.org/), as published in the Word Holder dictionary, version v19 | [CC BY-SA 4.0](https://creativecommons.org/licenses/by-sa/4.0/) |
| `homographs.tsv`, `form-other-verbs.tsv`, `ambiguous-forms.tsv` | derived from the same Wiktionary data (and, for `ambiguous-forms.tsv`, spaCy's tags over Tatoeba) | [CC BY-SA 4.0](https://creativecommons.org/licenses/by-sa/4.0/) |
| `page-example-forms.tsv` | which verb forms already show an example sentence on wordholder.com verb pages | Apache-2.0 |
| `verified-examples.tsv` | verb, form, Tatoeba id and table cell of the sentences syntango and the labels agree on; no sentence text | Apache-2.0 |

`tools/fr_homographs.py import` regenerates the source copies from their
origins; `tools/fr_homographs.py select` builds `homograph-set.jsonl` from the
files here alone; `tools/fr_homographs.py verified` writes
`verified-examples.tsv` from an `eval` run.
