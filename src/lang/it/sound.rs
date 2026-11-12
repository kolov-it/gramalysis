//! How a word starts, for the article before it.

/// The kind of sound a word starts with, which decides the article before
/// it: `sc` for s + consonant, z, gn, ps, pn, x, y and i + vowel (lo zio,
/// lo studente, lo psicologo, lo iogurt); `v` for a vowel or a silent h
/// (l'amico, l'hotel); `c` for any other consonant (il cane).
pub fn onset(word: &str) -> &'static str {
    let w = word.strip_prefix('h').unwrap_or(word);
    let mut chars = w.chars();
    let (Some(a), b) = (chars.next(), chars.next()) else { return "c" };
    let vowel = |c: char| "aeiouàèéìíòóùú".contains(c);
    let consonant = |c: Option<char>| c.is_some_and(|c| c.is_alphabetic() && !vowel(c));
    let s_impura = a == 's' && consonant(b);
    let digraph = matches!((a, b), ('g', Some('n')) | ('p', Some('s')) | ('p', Some('n')));
    let semivowel_i = a == 'i' && b.is_some_and(vowel);
    if s_impura || digraph || semivowel_i || matches!(a, 'z' | 'x' | 'y') {
        "sc"
    } else if vowel(a) {
        "v"
    } else {
        "c"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn onsets() {
        for (w, o) in [("cane", "c"), ("zio", "sc"), ("studente", "sc"), ("psicologo", "sc"), ("gnomo", "sc"),
                       ("iogurt", "sc"), ("amico", "v"), ("hotel", "v"), ("sale", "c"), ("isola", "v")] {
            assert_eq!(onset(w), o, "{w}");
        }
    }
}
