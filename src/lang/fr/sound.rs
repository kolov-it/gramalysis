//! How a word starts, for the article or determiner before it.

use std::collections::HashSet;

/// The kind of sound a word starts with, which decides the form before it:
/// `v` for a vowel or a mute h (l'homme, cet arbre, mon amie, bel homme),
/// `c` for a consonant, an aspirated h (le héros, ce hibou, la hache) or a
/// consonantal y (le yoga, le yacht). `aspirated` holds the words that start
/// with a consonant sound although their first letter is h or y.
pub fn onset(word: &str, aspirated: &HashSet<String>) -> &'static str {
    let mut chars = word.chars();
    let Some(first) = chars.next() else { return "c" };
    let vowel = |c: char| "aeiouyàâäéèêëîïôöùûüœæ".contains(c);
    if matches!(first, 'h' | 'y') && is_aspirated(word, aspirated) {
        return "c";
    }
    if first == 'h' || vowel(first) { "v" } else { "c" }
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
        let aspirated: HashSet<String> = ["héros", "haut", "hache", "yoga"].iter().map(|s| s.to_string()).collect();
        for (w, o) in [("chat", "c"), ("ami", "v"), ("homme", "v"), ("héros", "c"), ("hautes", "c"), ("haches", "c"),
                       ("hôtel", "v"), ("école", "v"), ("yeux", "v"), ("yoga", "c")] {
            assert_eq!(onset(w, &aspirated), o, "{w}");
        }
    }
}
