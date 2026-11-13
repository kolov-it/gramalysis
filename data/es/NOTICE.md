# Data in `data/es/` and its licences

The code of syntango is Apache-2.0 (see ../../NOTICE). The data files below are not code
and keep their sources' licences.

| File | Source | Licence |
|---|---|---|
| `closed.txt`, `grammar.gram`, `drifted-participles.txt`, `lexicon-patches/` | written for syntango | Apache-2.0 |
| `tatoeba-pool.jsonl.gz`, `homograph-set.jsonl` (the `text` field of each sentence) | [Tatoeba](https://tatoeba.org), Spanish sentences, export of 2026-10-09. Each sentence keeps its Tatoeba id (`id` / `tatoeba_id`, page `https://tatoeba.org/sentences/show/<id>`) and its author (`author`). No English translations are copied. | [CC BY 2.0 FR](https://creativecommons.org/licenses/by/2.0/fr/) |
| the `spacy` / `tokens` fields of those files | tokens, part-of-speech tags and lemmas from spaCy `es_core_news_md` 3.8.0 run over the Tatoeba sentences | as the sentences |
| `homograph-labels.tsv` | the correct reading of each sentence's ambiguous word, labelled for syntango (who labelled is in the file) | Apache-2.0 |
| `verified-examples.tsv` | the sentences of `homograph-set.jsonl` where syntango and the label agree on the page verb (ids and cells only, no sentence text) | Apache-2.0 |
| `verbs.json` | Spanish conjugation tables from [Wiktionary](https://en.wiktionary.org/) via [Kaikki.org](https://kaikki.org/), as published in the Word Holder dictionary, version v19 | [CC BY-SA 4.0](https://creativecommons.org/licenses/by-sa/4.0/) |
| `homographs.tsv`, `form-other-verbs.tsv`, `ambiguous-forms.tsv` | derived from the same Wiktionary data (and, for `homographs.tsv` and `ambiguous-forms.tsv`, spaCy's tags over Tatoeba and Leipzig corpus word counts) | [CC BY-SA 4.0](https://creativecommons.org/licenses/by-sa/4.0/) |
| `page-example-forms.tsv` | which verb forms already show an example sentence on wordholder.com verb pages | Apache-2.0 |

`python3 tools/it_homographs.py --lang es import` regenerates these files from their
sources; `python3 tools/it_homographs.py --lang es select` builds `homograph-set.jsonl`
from the files here alone; `tools/verified_examples.py --lang es` writes
`verified-examples.tsv` from an `eval --out` file.
