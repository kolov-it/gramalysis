#!/usr/bin/env python3
"""French verb forms that have no example sentence on a Word Holder verb page
because the form is ambiguous, and Tatoeba sentences containing them.

Two steps:

  import   copies the sources into data/fr/ (run once; the only step that reads
           outside the repository):
             python3 tools/fr_homographs.py import \\
               --verb-pages DIR          vocabuddy content/verb-batch/cache/fr
               --conjugations FILE       verb-conjugations.json (Word Holder dictionary v19)
               --page-examples FILE      verb-form-examples.json (same version)
               --homographs FILE         wordy config/homographs/homographs-fr.tsv
               --tatoeba FILE            fr-prepared.jsonl.gz (Tatoeba + spaCy tags)

  select   builds data/fr/homograph-set.jsonl from data/fr/ alone:
             python3 tools/fr_homographs.py select [--size 5000] [--per-form 3]

The same steps as tools/it_homographs.py, on the French files, with at most
--per-form sentences per form. Pronominal verbs' tables write their forms with
the pronoun (m'enfuis, nous enfuyons); like every form of more than one word,
those are left out. Only the standard library; deterministic for the same
inputs.
"""

import argparse
import gzip
import json
import os
import re
import sys
from collections import Counter, defaultdict

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
DATA = os.path.join(ROOT, "data", "fr")
VERBS = os.path.join(DATA, "verbs.json")
HOMOGRAPHS = os.path.join(DATA, "homographs.tsv")
PAGE_FORMS = os.path.join(DATA, "page-example-forms.tsv")
OTHER_VERBS = os.path.join(DATA, "form-other-verbs.tsv")
POOL = os.path.join(DATA, "tatoeba-pool.jsonl.gz")
CANDIDATES = os.path.join(DATA, "ambiguous-forms.tsv")
SET = os.path.join(DATA, "homograph-set.jsonl")

WORD = re.compile(r"[^\W\d_]+(?:'[^\W\d_]+)?", re.UNICODE)
POOL_PER_GROUP = 8   # pool sentences per form and spaCy reading (verb / other)
MIN_WORDS, MAX_WORDS = 4, 20


def cells(table):
    """(cell name, form) for every one-word form of a conjugation table."""
    for mood, tenses in table["conjugation"].items():
        if mood == "non_finite":
            for name, form in tenses.items():
                yield f"non_finite.{name}", form
            continue
        for tense, persons in tenses.items():
            for person, form in persons.items():
                yield f"{mood}.{tense}.{person}", form


def one_word(form):
    return form and " " not in form and "'" not in form


def words(text):
    return [m.group(0) for m in WORD.finditer(text)]


def cmd_import(a):
    os.makedirs(DATA, exist_ok=True)
    page_verbs = sorted(f[:-5] for f in os.listdir(a.verb_pages) if f.endswith(".json"))
    tables = json.load(open(a.conjugations))["verbs"]
    by_inf = {t["infinitive"]: t for t in tables}
    missing = [v for v in page_verbs if v not in by_inf]
    if missing:
        sys.exit(f"verb pages without a conjugation table: {missing[:10]}")

    # Every verb a form belongs to, over the whole dictionary.
    form_verbs = defaultdict(set)
    for t in tables:
        for _, form in cells(t):
            if one_word(form):
                form_verbs[form.lower()].add(t["infinitive"])

    with open(a.homographs) as src, open(HOMOGRAPHS, "w") as dst:
        dst.write(src.read())

    page_tables = [by_inf[v] for v in page_verbs]
    with open(VERBS, "w") as f:
        json.dump({"source": "Wiktionary via Kaikki.org, Word Holder dictionary v19 (CC BY-SA 4.0)",
                   "verbs": page_tables}, f, ensure_ascii=False, separators=(",", ":"))
        f.write("\n")

    shown = json.load(open(a.page_examples))["verbs"]
    with open(PAGE_FORMS, "w") as f:
        f.write("# verb\tform — forms that already have an example sentence on the verb page\n")
        for v in sorted(shown, key=lambda v: v["infinitive"]):
            if v["infinitive"] in by_inf and v["infinitive"] in set(page_verbs):
                for fm in v["forms"]:
                    f.write(f"{v['infinitive']}\t{fm['form']}\n")

    page_forms = {fm.lower() for t in page_tables for _, fm in cells(t) if one_word(fm)}
    with open(OTHER_VERBS, "w") as f:
        f.write("# form\tverbs — every verb in the dictionary with this form (page verbs' forms only)\n")
        for form in sorted(page_forms):
            if len(form_verbs[form]) > 1:
                f.write(f"{form}\t{';'.join(sorted(form_verbs[form]))}\n")

    # Forms that lack an example on their verb page.
    without_example = defaultdict(set)
    shown_by_verb = defaultdict(set)
    for v in shown:
        shown_by_verb[v["infinitive"]] = {fm["form"].lower() for fm in v["forms"]}
    for t in page_tables:
        for _, fm in cells(t):
            if one_word(fm) and fm.lower() not in shown_by_verb[t["infinitive"]]:
                without_example[fm.lower()].add(t["infinitive"])

    homograph_forms = {l.split("\t", 1)[0] for l in open(HOMOGRAPHS) if l.strip() and not l.startswith("#")}

    def read_tatoeba():
        with gzip.open(a.tatoeba, "rt") as f:
            for line in f:
                if not line.startswith("#"):
                    yield json.loads(line)

    # spaCy's part of speech for each such form over all of Tatoeba.
    spacy_pos = defaultdict(Counter)
    for s in read_tatoeba():
        for tok, pos, _ in s["tokens"]:
            if tok.lower() in without_example:
                spacy_pos[tok.lower()]["verb" if pos in ("VERB", "AUX") else "other"] += 1

    def reasons(fm):
        r = []
        if fm in homograph_forms:
            r.append("homograph-list")
        if len(form_verbs[fm]) > 1:
            r.append("several-verbs")
        if spacy_pos[fm]["other"] and spacy_pos[fm]["verb"]:
            r.append("spacy-mixed-pos")
        elif spacy_pos[fm]["other"]:
            r.append("spacy-non-verb")
        return r

    candidates = {fm: reasons(fm) for fm in without_example}
    candidates = {fm: r for fm, r in candidates.items() if r}
    with open(CANDIDATES, "w") as f:
        f.write("# form\tpage verbs\tambiguous because\tspaCy verb/other tags over all of Tatoeba\n")
        for fm in sorted(candidates):
            sp = spacy_pos[fm]
            f.write(f"{fm}\t{';'.join(sorted(without_example[fm]))}\t{','.join(candidates[fm])}\t{sp['verb']}/{sp['other']}\n")

    # Pool: per candidate form, up to POOL_PER_GROUP sentences spaCy reads as
    # the verb and as much again that it reads as something else.
    kept = Counter()
    pool = []
    for s in read_tatoeba():
        ws = words(s["text"])
        if not (MIN_WORDS <= len(ws) <= MAX_WORDS):
            continue
        tags, take = [], False
        for i, (tok, pos, lemma) in enumerate(s["tokens"]):
            fm = tok.lower()
            if fm in candidates:
                group = (fm, "verb" if pos in ("VERB", "AUX") else "other")
                if kept[group] < POOL_PER_GROUP:
                    kept[group] += 1
                    take = True
                tags.append([i, pos, lemma])
        if take:
            pool.append({"id": s["id"], "author": s["author"], "text": s["text"],
                         "tokens": [t[0] for t in s["tokens"]], "spacy": tags})
    pool.sort(key=lambda s: s["id"])
    with gzip.open(POOL, "wt") as f:
        for s in pool:
            f.write(json.dumps(s, ensure_ascii=False, separators=(",", ":")) + "\n")
    print(f"page verbs {len(page_verbs)}, ambiguous forms without an example {len(candidates)}, pool sentences {len(pool)}")


def load_homographs():
    out = {}
    for line in open(HOMOGRAPHS):
        if line.startswith("#") or not line.strip():
            continue
        parts = line.rstrip("\n").split("\t")
        if len(parts) < 4:
            continue
        readings = []
        for r in parts[3].split("; "):
            m = re.match(r"(.+?)/(\w+)", r.strip())
            if m:
                readings.append({"lemma": m.group(1), "pos": m.group(2)})
        out[parts[0]] = readings
    return out


def cmd_select(a):
    tables = json.load(open(VERBS))["verbs"]
    other_verbs = {}
    for line in open(OTHER_VERBS):
        if not line.startswith("#"):
            fm, vs = line.rstrip("\n").split("\t")
            other_verbs[fm] = vs.split(";")
    homographs = load_homographs()
    candidates = {}
    for line in open(CANDIDATES):
        if not line.startswith("#"):
            fm, verbs, why, _ = line.rstrip("\n").split("\t")
            candidates[fm] = (set(verbs.split(";")), why.split(","))
    cells_of = defaultdict(lambda: defaultdict(list))
    for t in tables:
        for cell, fm in cells(t):
            if one_word(fm) and fm.lower() in candidates and t["infinitive"] in candidates[fm.lower()][0]:
                cells_of[fm.lower()][t["infinitive"]].append(cell)

    # Per form, its pool sentences alternating spaCy's verb and other readings.
    groups = defaultdict(lambda: {"verb": [], "other": []})
    with gzip.open(POOL, "rt") as f:
        for line in f:
            s = json.loads(line)
            for i, pos, lemma in s["spacy"]:
                fm = s["tokens"][i].lower()
                if fm in candidates:
                    groups[fm]["verb" if pos in ("VERB", "AUX") else "other"].append((s, i, pos, lemma))
    by_form = {}
    for fm, g in groups.items():
        seq = []
        for k in range(max(len(g["verb"]), len(g["other"]))):
            for name in ("verb", "other"):
                if k < len(g[name]):
                    seq.append(g[name][k])
        by_form[fm] = seq

    items, round_ = [], 0
    forms = sorted(by_form)
    while len(items) < a.size and round_ < a.per_form and any(len(by_form[f]) > round_ for f in forms):
        for fm in forms:
            if len(by_form[fm]) > round_ and len(items) < a.size:
                s, i, pos, lemma = by_form[fm][round_]
                verb_cells = cells_of[fm]
                readings = [{"lemma": v, "pos": "verb", "cells": cs} for v, cs in sorted(verb_cells.items())]
                for v in other_verbs.get(fm, []):
                    if v not in verb_cells:
                        readings.append({"lemma": v, "pos": "verb", "cells": None})
                for r in homographs.get(fm, []):
                    if not any(x["lemma"] == r["lemma"] and x["pos"] == r["pos"] for x in readings):
                        readings.append({"lemma": r["lemma"], "pos": r["pos"], "cells": None})
                items.append({
                    "tatoeba_id": s["id"], "author": s["author"], "text": s["text"],
                    "form": fm, "token": i,
                    # which occurrence of the form in the sentence (0 = first)
                    "occurrence": sum(1 for t in s["tokens"][:i] if t.lower() == fm),
                    "page_verbs": sorted(verb_cells),
                    "readings": readings, "ambiguous_because": candidates[fm][1],
                    "spacy": {"pos": pos, "lemma": lemma},
                    "label": None,
                })
        round_ += 1
    items.sort(key=lambda x: (x["form"], x["tatoeba_id"]))
    with open(SET, "w") as f:
        for it in items:
            f.write(json.dumps(it, ensure_ascii=False, separators=(",", ":")) + "\n")
    reasons = Counter(r for it in items for r in it["ambiguous_because"])
    spacy_verb = sum(it["spacy"]["pos"] in ("VERB", "AUX") for it in items)
    print(f"ambiguous forms {len(candidates)}, with sentences {len(by_form)}, items {len(items)}, spaCy reads {spacy_verb} as the verb")
    print("ambiguous because:", dict(reasons))


def main():
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = p.add_subparsers(dest="cmd", required=True)
    i = sub.add_parser("import")
    for name in ("verb-pages", "conjugations", "page-examples", "homographs", "tatoeba"):
        i.add_argument(f"--{name}", required=True)
    s = sub.add_parser("select")
    s.add_argument("--size", type=int, default=5000)
    s.add_argument("--per-form", type=int, default=3)
    a = p.parse_args()
    {"import": cmd_import, "select": cmd_select}[a.cmd](a)


if __name__ == "__main__":
    main()
