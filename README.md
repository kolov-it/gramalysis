# gramalysis

Morphological and syntactic analysis of sentences, with explanations.

Give it a sentence and it tells you, for every word, which dictionary entry it
comes from and what form it is, and for the sentence, which structures it can
have: subject, predicate, objects, complements. Every result carries the rule
that produced it, so a learner can ask *why*:

> *Tom **fece** del volontariato.*
> **fece** is *fare*, passato remoto, 3rd person singular: the subject *Tom* is
> 3rd person singular, and the verb agrees with its subject.

Italian first; the design is multilingual from the start.

**Status:** design. Nothing to run yet. Start with [doc/design.md](doc/design.md).

## How it differs from existing tools

- **Statistical parsers** (spaCy, Stanza, UDPipe) return one best guess and cannot
  say why. gramalysis returns every valid reading and the rule behind it.
- **Rule-based grammars** (Grammatical Framework, DELPH-IN) are the closest
  relatives, but are research systems that are heavy to adopt. gramalysis is a plain
  Rust library aimed at language learners and the tools built for them.
- **Grammar checkers** (LanguageTool) look for errors. gramalysis builds the
  structure.

Output uses the [Universal Dependencies](https://universaldependencies.org/)
part-of-speech and feature vocabulary, so it can be measured against the UD
treebanks.

## Licence

Code: dual-licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at
your option.

Lexicon data is not part of this repository. Lexicons built from Wiktionary are
CC BY-SA and are distributed separately with their own notice.
