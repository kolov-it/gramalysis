//! Splitting French text into tokens: words, elided forms (`l'`, `qu'`,
//! `jusqu'`), pronouns joined to the verb by a hyphen (`parlez-vous`,
//! `donne-le-moi`, `a-t-il`) and the punctuation the grammar uses.
//!
//! A pronoun after a hyphen keeps the hyphen (`-vous`, `-le`, `-moi`): it is
//! attached to the verb before it, and the grammar treats it so (inversion,
//! affirmative imperative). The euphonic `-t-` (`a-t-il`) is dropped.

/// Words that elide before a vowel and are split off with their apostrophe.
/// `aujourd'hui`, `quelqu'un` and `presqu'île` stay whole words.
const ELIDING: [&str; 14] =
    ["l", "d", "j", "m", "t", "s", "n", "c", "ç", "qu", "jusqu", "lorsqu", "puisqu", "quoiqu"];

/// Pronouns that can follow a verb after a hyphen.
const HYPHEN_PRONOUNS: [&str; 19] = [
    "je", "tu", "il", "elle", "on", "nous", "vous", "ils", "elles", "le", "la", "les", "lui", "leur", "moi", "toi", "y",
    "en", "ce",
];

/// Nouns written with a hyphen before a pronoun: they stay one word. The
/// imperative they share a spelling with (rendez-vous = give yourselves up) is
/// rare beside the noun, and a token has one spelling in the lattice.
const HYPHENATED_NOUNS: [&str; 4] = ["rendez-vous", "chez-moi", "chez-toi", "chez-soi"];

/// -ci and -là after a demonstrative (celui-ci, ce jour-là).
pub const DEMONSTRATIVE: [&str; 2] = ["ci", "là"];

pub fn tokenize(text: &str) -> Vec<String> {
    let text = text.replace(['’', 'ʼ'], "'").replace(['\u{a0}', '\u{202f}'], " ");
    let mut tokens = Vec::new();
    for raw in text.split_whitespace() {
        let word = raw.trim_start_matches(['"', '«', '(', '“', '—', '–', '-']);
        let core = word.trim_end_matches(['.', ',', ';', ':', '!', '?', '"', '»', ')', '”', '…']);
        let trailing = &word[core.len()..];
        for (i, part) in hyphen_parts(core).into_iter().enumerate() {
            let hyphen = if i > 0 { "-" } else { "" };
            for piece in elision_parts(part) {
                tokens.push(format!("{hyphen}{piece}"));
            }
        }
        tokens.extend(trailing.chars().filter(|c| matches!(c, ',' | ';' | ':' | '.' | '?' | '!')).map(|c| c.to_string()));
    }
    // Sentence-final punctuation ends the text; inside it, it separates
    // sentences (Tu viens ? On t'attend.). A final question mark stays: it
    // tells a question (Parlez-vous français ?) from an order (Asseyez-vous.).
    let mut question = false;
    while let Some(t) = tokens.last().filter(|t| matches!(t.as_str(), "." | "?" | "!")) {
        question |= t == "?";
        tokens.pop();
    }
    if question {
        tokens.push("?".to_owned());
    }
    tokens
}

/// `parlez-vous` → [parlez, vous]; `a-t-il` → [a, il]; `donne-le-moi` →
/// [donne, le, moi]; `va-t'en` → [va, t'en]; `celui-ci` → [celui, ci]. A
/// hyphenated word whose tail is not a chain of pronouns (or -ci, -là) stays
/// whole (`peut-être`, `grand-mère`, `là-bas`).
fn hyphen_parts(word: &str) -> Vec<&str> {
    let parts: Vec<&str> = word.split('-').collect();
    if parts.len() < 2 || parts[0].is_empty() || HYPHENATED_NOUNS.contains(&word.to_lowercase().as_str()) {
        return vec![word];
    }
    let pronoun = |p: &str| {
        let lower = p.to_lowercase();
        HYPHEN_PRONOUNS.contains(&lower.as_str()) || DEMONSTRATIVE.contains(&lower.as_str()) || lower == "t" || lower == "t'en" || lower == "m'en" || lower == "t'y" || lower == "m'y"
    };
    if !parts[1..].iter().all(|p| pronoun(p)) {
        return vec![word];
    }
    // The euphonic t is only a sound between a vowel and il/elle/on.
    let last = parts.len() - 1;
    parts
        .iter()
        .enumerate()
        .filter(|(i, p)| !(*i > 0 && *i < last && p.eq_ignore_ascii_case("t")))
        .map(|(_, p)| *p)
        .collect()
}

/// `l'homme` → [l', homme]; `jusqu'à` → [jusqu', à]; `aujourd'hui` stays.
fn elision_parts(word: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = word;
    while let Some(i) = rest.find('\'') {
        let head = &rest[..i];
        if i + 1 >= rest.len() || !ELIDING.contains(&head.to_lowercase().as_str()) {
            break;
        }
        out.push(rest[..=i].to_owned());
        rest = &rest[i + 1..];
    }
    if !rest.is_empty() {
        out.push(rest.to_owned());
    }
    out
}

pub fn is_punct(token: &str) -> bool {
    matches!(token, "," | ";" | ":" | "." | "?" | "!")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn elision_and_punctuation() {
        assert_eq!(tokenize("L'homme qu'il a vu, jusqu’à hier."), ["L'", "homme", "qu'", "il", "a", "vu", ",", "jusqu'", "à", "hier"]);
        assert_eq!(tokenize("Aujourd'hui, quelqu'un est venu."), ["Aujourd'hui", ",", "quelqu'un", "est", "venu"]);
    }

    #[test]
    fn hyphenated_pronouns() {
        assert_eq!(tokenize("Parlez-vous français ?"), ["Parlez", "-vous", "français", "?"]);
        assert_eq!(tokenize("A-t-il fini ?"), ["A", "-il", "fini", "?"]);
        assert_eq!(tokenize("Donne-le-moi !"), ["Donne", "-le", "-moi"]);
        assert_eq!(tokenize("Va-t'en !"), ["Va", "-t'", "-en"]);
        assert_eq!(tokenize("Qu'est-ce que c'est ?"), ["Qu'", "est", "-ce", "que", "c'", "est", "?"]);
        assert_eq!(tokenize("C'est peut-être ma grand-mère."), ["C'", "est", "peut-être", "ma", "grand-mère"]);
        assert_eq!(tokenize("J'ai un rendez-vous."), ["J'", "ai", "un", "rendez-vous"]);
        assert_eq!(tokenize("Ce jour-là, celle-ci était là-bas."), ["Ce", "jour", "-là", ",", "celle", "-ci", "était", "là-bas"]);
    }

    #[test]
    fn quotes() {
        assert_eq!(tokenize("« Fais-moi confiance », dit-il."), ["Fais", "-moi", "confiance", ",", "dit", "-il"]);
    }
}
