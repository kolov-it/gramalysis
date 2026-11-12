//! How a word starts, for the article or determiner before it.

use std::collections::HashSet;

/// The kind of sound a word starts with, which decides the form before it:
/// `v` for a vowel or a mute h (l'homme, cet arbre, mon amie, bel homme),
/// `c` for a consonant or an aspirated h (le héros, ce hibou, la hache).
/// `aspirated` holds the words with an aspirated h.
pub fn onset(word: &str, aspirated: &HashSet<String>) -> &'static str {
    let mut chars = word.chars();
    let Some(first) = chars.next() else { return "c" };
    let vowel = |c: char| "aeiouyàâäéèêëîïôöùûüœæ".contains(c);
    if first == 'h' {
        return if is_aspirated(word, aspirated) { "c" } else { "v" };
    }
    if vowel(first) { "v" } else { "c" }
}

/// The word, or the word less a plural or feminine ending, is on the list.
fn is_aspirated(word: &str, aspirated: &HashSet<String>) -> bool {
    if aspirated.contains(word) {
        return true;
    }
    ["s", "x", "e", "es"].iter().any(|end| word.strip_suffix(end).is_some_and(|stem| aspirated.contains(stem)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn onsets() {
        let aspirated: HashSet<String> = ["héros", "haut", "hache"].iter().map(|s| s.to_string()).collect();
        for (w, o) in [("chat", "c"), ("ami", "v"), ("homme", "v"), ("héros", "c"), ("hautes", "c"), ("haches", "c"),
                       ("hôtel", "v"), ("école", "v"), ("yeux", "v")] {
            assert_eq!(onset(w, &aspirated), o, "{w}");
        }
    }
}
