//! What a verb page asks of a reading: which verb it is, and whether a
//! participle was used as the verb or like an adjective. Shared by `eval` and
//! [`crate::examples`].

use crate::lang::es::strip_accent;
use crate::morph::Lang;
use crate::syntax::lattice::Morpheme;

/// The grammar rules for a participle used like an adjective: a past
/// participle (le luci accese) and an Italian present participle (il treno
/// fuggente).
pub const ADJECTIVE_USES: [&str; 2] = ["ap.participle", "ap.present-participle"];

/// A pronominal infinitive's base verb: mettersi → mettere, smetterla →
/// smettere, andarsene → andare, porsi → porre; Spanish irse → ir,
/// arreglárselas → arreglar. Dictionaries and labellers name such verbs
/// either way; for a verb page they are the same verb.
pub fn base_verb(lemma: &str, lang: Lang) -> String {
    if lang == Lang::Es {
        const CLITICS: [&str; 9] = [
            "selas", "selos", "sela", "selo", "las", "los", "se", "la", "lo",
        ];
        for c in CLITICS {
            if let Some(stem) = lemma.strip_suffix(c)
                && stem.len() >= 2
                && stem.ends_with('r')
            {
                return strip_accent(stem);
            }
        }
        return strip_accent(lemma);
    }
    const CLITICS: [&str; 14] = [
        "sene", "sela", "selo", "cela", "celo", "gliela", "glielo", "si", "ci", "vi", "la", "lo",
        "le", "ne",
    ];
    for c in CLITICS {
        if let Some(stem) = lemma.strip_suffix(c)
            && stem.len() >= 3
            && stem.ends_with('r')
        {
            // -rre verbs: porre, condurre, trarre.
            let rre = ["por", "dur", "trar"].iter().any(|s| stem.ends_with(s));
            return format!("{stem}{}", if rre { "re" } else { "e" });
        }
    }
    lemma.to_owned()
}

/// How a parse used a verb morpheme, for a verb page. A participle the
/// grammar only used like an adjective (le luci sono accese) is not the
/// verb; used both ways within one reading, it is both.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VerbUse {
    pub as_verb: bool,
    pub as_adjective: bool,
}

/// `m` is the first morpheme of a reading, `uses` the rules that consumed it.
pub fn verb_use(m: &Morpheme, uses: &[String]) -> VerbUse {
    let participle = m.cat == "Verb"
        && matches!(
            m.feats.get("form").map(String::as_str),
            Some("pastpart" | "prespart")
        );
    let adjectival = uses.iter().any(|u| ADJECTIVE_USES.contains(&u.as_str()));
    let verbal = uses.iter().any(|u| !ADJECTIVE_USES.contains(&u.as_str()));
    VerbUse {
        as_verb: !participle || verbal,
        as_adjective: participle && adjectival,
    }
}

#[cfg(test)]
mod tests {
    use super::{Lang, base_verb};

    #[test]
    fn pronominal_infinitives_map_to_their_base_verb() {
        for (a, b) in [
            ("mettersi", "mettere"),
            ("smetterla", "smettere"),
            ("andarsene", "andare"),
            ("porsi", "porre"),
            ("prenderci", "prendere"),
            ("fare", "fare"),
            ("dire", "dire"),
        ] {
            assert_eq!(base_verb(a, Lang::It), b, "{a}");
        }
        for (a, b) in [
            ("irse", "ir"),
            ("arreglárselas", "arreglar"),
            ("reírse", "reir"),
            ("reír", "reir"),
            ("dar", "dar"),
        ] {
            assert_eq!(base_verb(a, Lang::Es), b, "{a}");
        }
    }
}
