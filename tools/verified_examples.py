#!/usr/bin/env python3
"""Writes data/<lang>/verified-examples.tsv: the homograph-set sentences where
syntango decides the word is its page verb AND the label agrees, in the
format wordy's verb-form-examples builder reads
(verb<TAB>form<TAB>tatoeba_id<TAB>cell<TAB>source).

    cargo run --release -- eval es data/es/homograph-set.jsonl \\
      --labels data/es/homograph-labels.tsv --fixture tests/fixtures/es-set.json --out /tmp/eval.jsonl
    python3 tools/verified_examples.py --lang es /tmp/eval.jsonl [--date 2026-11-12]

Kept: sentences of 5-20 words (the builder's bounds), labelled as a verb with
the same lemma syntango chose, not noted as an accent typo, whose label cell
holds the form in the verb's table. A label with several cells (1s|3s) gives
the first one that holds the form. Only the standard library.
"""

import argparse
import datetime
import json
import os
import re

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
WORD = re.compile(r"[^\W\d_]+(?:'[^\W\d_]+)?", re.UNICODE)
MIN_WORDS, MAX_WORDS = 5, 20


def cells(table):
    for mood, tenses in table["conjugation"].items():
        if mood == "non_finite":
            for name, form in tenses.items():
                yield f"non_finite.{name}", form
            continue
        for tense, persons in tenses.items():
            for person, form in persons.items():
                for one in form.split(" / "):
                    yield f"{mood}.{tense}.{person}", one


def main():
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("--lang", required=True)
    p.add_argument("--date", default=datetime.date.today().isoformat())
    p.add_argument("eval_out", help="the JSON lines `syntango eval --out` wrote")
    a = p.parse_args()
    data = os.path.join(ROOT, "data", a.lang)

    tables = {t["infinitive"]: t for t in json.load(open(os.path.join(data, "verbs.json")))["verbs"]}
    holds = {v: {(c, f.lower()) for c, f in cells(t)} for v, t in tables.items()}
    labels = {}
    for line in open(os.path.join(data, "homograph-labels.tsv")):
        if line.startswith("#") or not line.strip():
            continue
        cols = line.rstrip("\n").split("\t")
        labels[(int(cols[0]), cols[1])] = cols
    items = {}
    for line in open(os.path.join(data, "homograph-set.jsonl")):
        it = json.loads(line)
        items[(it["tatoeba_id"], it["form"])] = it

    rows = set()
    for line in open(a.eval_out):
        r = json.loads(line)
        if r["status"] != "decided" or r.get("correct") is not True or r["reference_source"] != "label":
            continue
        reading = r["readings"][0]
        if not reading["verb"]:
            continue
        key = (r["tatoeba_id"], r["form"])
        _, form, lemma, pos, cell, _, _, note = labels[key]
        if pos != "verb" or "accent typo" in note or lemma not in items[key]["page_verbs"]:
            continue
        if not MIN_WORDS <= len(WORD.findall(items[key]["text"])) <= MAX_WORDS:
            continue
        cell = next((c for c in cell.split("|") if (c, form) in holds.get(lemma, set())), None)
        if cell is None:
            continue
        rows.add((lemma, form, r["tatoeba_id"], cell))

    out = os.path.join(data, "verified-examples.tsv")
    with open(out, "w") as f:
        f.write("# verb\tform\ttatoeba_id\tcell\tsource — Tatoeba sentences syntango decided as this\n")
        f.write("# form of this verb, agreeing with an independent label (tools/verified_examples.py)\n")
        for lemma, form, tid, cell in sorted(rows):
            f.write(f"{lemma}\t{form}\t{tid}\t{cell}\tsyntango+label {a.date}\n")
    print(f"{len(rows)} rows, {len({r[1] for r in rows})} forms, {len({r[0] for r in rows})} verbs -> {out}")


if __name__ == "__main__":
    main()
