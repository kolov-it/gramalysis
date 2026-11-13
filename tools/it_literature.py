#!/usr/bin/env python3
"""Verb-form example candidates from public-domain Italian literature.

  import   copies the books' text (Project Gutenberg header and licence
           removed; the works are public domain) into data/it/literature/:
             python3 tools/it_literature.py import --gutenberg-dir DIR
           DIR holds <id>.txt as downloaded from gutenberg.org (ids in WORKS).

  select   splits the books into sentences and writes the candidate set,
           in the format `gramalysis eval` reads, from data/it/ alone:
             python3 tools/it_literature.py select [--per-form 4]

  examples writes verb-examples.tsv: every candidate whose label
           (literature-labels.tsv) makes it a form of a verb with a page,
           with the parser's verdict from a `gramalysis eval --out` file:
             python3 tools/it_literature.py examples --eval EVAL.jsonl

A candidate is a sentence holding a one-word form of a verb that has a page
on wordholder.com (data/it/verbs.json). `missing_on_page` marks forms the verb
page shows no example for today (data/it/page-example-forms.tsv).
"""

import argparse
import collections
import json
import os
import re

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
DATA = os.path.join(ROOT, "data", "it")
LIT = os.path.join(DATA, "literature")
TEXTS = os.path.join(LIT, "texts")
SENTENCES = os.path.join(LIT, "sentences.jsonl")
SET = os.path.join(LIT, "literature-set.jsonl")
LABELS = os.path.join(LIT, "literature-labels.tsv")
EXAMPLES = os.path.join(LIT, "verb-examples.tsv")

# id, author, title, year, Project Gutenberg ebook number
WORKS = [
    ("collodi-pinocchio", "Carlo Collodi", "Le avventure di Pinocchio", 1883, 52484),
    ("deamicis-ricordi-infanzia", "Edmondo De Amicis", "Ricordi d'infanzia e di scuola", 1901, 61885),
    ("manzoni-promessi-sposi", "Alessandro Manzoni", "I promessi sposi", 1840, 45334),
    ("pirandello-cavallo-luna", "Luigi Pirandello", "Un cavallo nella luna", 1918, 56775),
]
MIN_WORDS, MAX_WORDS = 5, 25
WORD = re.compile(r"[^\W\d_]+(?:'[^\W\d_]+)?")


def gutenberg_body(text):
    start = text.index("*** START")
    end = text.index("*** END")
    return text[text.index("\n", start) + 1:end].strip() + "\n"


def cmd_import(a):
    os.makedirs(TEXTS, exist_ok=True)
    with open(os.path.join(LIT, "sources.tsv"), "w") as f:
        f.write("# id\tauthor\ttitle\tyear\tsource\tlicence\n")
        for wid, author, title, year, pg in WORKS:
            raw = open(os.path.join(a.gutenberg_dir, f"{wid}.txt"), encoding="utf-8").read()
            with open(os.path.join(TEXTS, f"{wid}.txt"), "w", encoding="utf-8") as out:
                out.write(gutenberg_body(raw))
            f.write(f"{wid}\t{author}\t{title}\t{year}\thttps://www.gutenberg.org/ebooks/{pg}\tpublic domain\n")
    print(f"imported {len(WORKS)} works")


def split_sentences(text):
    """(paragraph number, sentence) for sentences of MIN..MAX words."""
    for n, para in enumerate(re.split(r"\n\s*\n", text)):
        p = re.sub(r"\s+", " ", para).strip()
        if len(p) < 20 or p.isupper():
            continue
        p = p.replace("—", " — ").replace("«", '"').replace("»", '"').replace("_", "")
        for s in re.split(r'(?<=[.!?])\s+(?=["—]?\s*[A-ZÈÀÌÒÙ])', p):
            s = re.sub(r"\s+", " ", s).strip(' —"')
            words = WORD.findall(s)
            if MIN_WORDS <= len(words) <= MAX_WORDS and not re.search(r"[\d\[\]*]", s):
                yield n, s


def cells(table):
    for mood, tenses in table["conjugation"].items():
        if mood == "non_finite":
            for name, form in tenses.items():
                yield f"non_finite.{name}", form
            continue
        for tense, persons in tenses.items():
            for person, form in persons.items():
                yield f"{mood}.{tense}.{person}", form


def cmd_select(a):
    works = {}
    for line in open(os.path.join(LIT, "sources.tsv")):
        if not line.startswith("#"):
            wid, author, title, year, source, _ = line.rstrip("\n").split("\t")
            works[wid] = {"author": author, "title": title, "year": int(year), "source": source}

    sentences = []
    for wid in works:
        text = open(os.path.join(TEXTS, f"{wid}.txt"), encoding="utf-8").read()
        for para, s in split_sentences(text):
            sentences.append({"id": len(sentences) + 1, "work": wid, "paragraph": para, "text": s})
    with open(SENTENCES, "w") as f:
        for s in sentences:
            f.write(json.dumps(s, ensure_ascii=False) + "\n")

    shown = collections.defaultdict(set)
    for line in open(os.path.join(DATA, "page-example-forms.tsv")):
        if not line.startswith("#"):
            verb, form = line.rstrip("\n").split("\t")
            shown[verb].add(form.lower())
    # form -> {verb: missing on that verb's page}
    targets = collections.defaultdict(dict)
    for t in json.load(open(os.path.join(DATA, "verbs.json")))["verbs"]:
        for _, form in cells(t):
            if form and " " not in form and "'" not in form:
                f = form.lower()
                targets[f][t["infinitive"]] = f not in shown[t["infinitive"]]

    # Forms missing on a page first; at most --per-form sentences per form,
    # spread across the works in round-robin order.
    by_form = collections.defaultdict(list)
    for s in sentences:
        seen = collections.Counter()
        for w in (w.lower() for w in WORD.findall(s["text"])):
            if w in targets:
                by_form[w].append((s, seen[w]))
            seen[w] += 1
    items = []
    for form in sorted(by_form, key=lambda f: (not any(targets[f].values()), f)):
        per_work = collections.defaultdict(list)
        for s, occ in by_form[form]:
            per_work[s["work"]].append((s, occ))
        picked = []
        while len(picked) < a.per_form and any(per_work.values()):
            for wid in sorted(per_work):
                if per_work[wid] and len(picked) < a.per_form:
                    picked.append(per_work[wid].pop(0))
        for s, occ in picked:
            items.append({
                "tatoeba_id": s["id"], "work": s["work"], "text": s["text"], "form": form,
                "occurrence": occ, "page_verbs": sorted(targets[form]),
                "missing_on_page": sorted(v for v, missing in targets[form].items() if missing),
                "spacy": {"pos": "", "lemma": ""},
            })
    with open(SET, "w") as f:
        for it in items:
            f.write(json.dumps(it, ensure_ascii=False) + "\n")
    per_work = collections.Counter(s["work"] for s in sentences)
    print("sentences", dict(per_work), "candidates", len(items), "forms", len({i["form"] for i in items}),
          "forms missing on a page", len({i["form"] for i in items if i["missing_on_page"]}))


def cmd_examples(a):
    works = {}
    for line in open(os.path.join(LIT, "sources.tsv")):
        if not line.startswith("#"):
            wid, author, title, year, source, _ = line.rstrip("\n").split("\t")
            works[wid] = (author, title, year)
    labels = {}
    for line in open(LABELS):
        if line.startswith("#") or not line.strip():
            continue
        sid, form, occ, lemma, pos, cell, *_ = line.rstrip("\n").split("\t")
        labels[(int(sid), form, int(occ))] = (lemma, pos, cell)
    items = [json.loads(l) for l in open(SET)]
    verdicts = [json.loads(l) for l in open(a.eval)]
    if len(verdicts) != len(items):
        raise SystemExit(f"{a.eval} has {len(verdicts)} rows, {SET} has {len(items)}")
    rows = []
    for it, v in zip(items, verdicts):
        if (v["text"], v["form"]) != (it["text"], it["form"]):
            raise SystemExit(f"{a.eval} is not an eval of {SET} (row {it['tatoeba_id']})")
        label = labels.get((it["tatoeba_id"], it["form"], it["occurrence"]))
        if not label or label[1] != "verb" or not label[2]:
            continue
        # A pronominal use is labelled with its base verb (andarsene: andare).
        pages = [p for p in it["page_verbs"] if p == label[0] or re.sub(r"(si|sene|sela|seli|cene|ci|ne)$", "e", p) == label[0]]
        if not pages:
            continue
        lemma, cell = pages[0], label[2]
        if v["status"] == "decided":
            parser = "agrees" if v["readings"][0] == {"verb": True, "lemma": lemma} else "disagrees"
        else:
            parser = v["status"]
        author, title, year = works[it["work"]]
        rows.append((lemma, it["form"], cell, "yes" if lemma in it["missing_on_page"] else "no", parser,
                     it["text"], it["tatoeba_id"], author, title, year))
    rows.sort(key=lambda r: (r[0], r[1], r[4] != "agrees", r[6]))
    with open(EXAMPLES, "w") as f:
        f.write("# Verb-form example sentences from the works in sources.tsv (public domain).\n"
                "# Built by tools/it_literature.py examples; the reading of the form is the\n"
                "# label in literature-labels.tsv. missing_on_page: the verb page shows no\n"
                "# example for this form today. parser: gramalysis on the sentence (agrees,\n"
                "# disagrees, ambiguous).\n"
                "# verb\tform\tcell\tmissing_on_page\tparser\tsentence\tsentence_id\tauthor\ttitle\tyear\n")
        for r in rows:
            f.write("\t".join(map(str, r)) + "\n")
    print("examples", len(rows), "verbs", len({r[0] for r in rows}),
          "forms missing on a page", len({(r[0], r[1]) for r in rows if r[3] == "yes"}),
          "parser", dict(collections.Counter(r[4] for r in rows)))


def main():
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = p.add_subparsers(dest="cmd", required=True)
    i = sub.add_parser("import")
    i.add_argument("--gutenberg-dir", required=True)
    s = sub.add_parser("select")
    s.add_argument("--per-form", type=int, default=4)
    e = sub.add_parser("examples")
    e.add_argument("--eval", required=True, help="a `gramalysis eval --out` file of literature-set.jsonl")
    a = p.parse_args()
    {"import": cmd_import, "select": cmd_select, "examples": cmd_examples}[a.cmd](a)


if __name__ == "__main__":
    main()
