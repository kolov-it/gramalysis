//! Splitting Italian text into tokens: words, elided forms (`l'`, `dell'`)
//! and the punctuation the grammar uses.

pub fn tokenize(text: &str) -> Vec<String> {
    split(text).into_iter().map(|(token, _)| token).collect()
}

/// For each token of [`tokenize`], whether an opening quote stands right
/// before it: a quotation starts a sentence of its own.
pub fn after_quote(text: &str) -> Vec<bool> {
    split(text).into_iter().map(|(_, quoted)| quoted).collect()
}

fn split(text: &str) -> Vec<(String, bool)> {
    let text = text.replace('’', "'");
    let mut tokens: Vec<(String, bool)> = Vec::new();
    // Dashes set off asides and dialogue in literary text; the grammar reads
    // the sentence without them.
    let text = text.replace(['—', '–'], " ");
    for raw in text.split_whitespace() {
        let word = raw.trim_start_matches(['"', '«', '(', '“']);
        let mut quoted = raw[..raw.len() - word.len()].contains(['"', '«', '“']);
        let core = word.trim_end_matches(['.', ',', ';', ':', '!', '?', '"', '»', ')', '”']);
        let trailing = &word[core.len()..];
        let mut rest = core;
        while let Some(i) = rest.find('\'') {
            if i + 1 >= rest.len() {
                break;
            }
            tokens.push((rest[..=i].to_owned(), std::mem::take(&mut quoted)));
            rest = &rest[i + 1..];
        }
        if !rest.is_empty() {
            tokens.push((modern_spelling(rest), quoted));
        }
        tokens.extend(trailing.chars().filter(|c| matches!(c, ',' | ';' | ':' | '.' | '?' | '!')).map(|c| (c.to_string(), false)));
    }
    // Sentence-final punctuation ends the text; inside it, it separates
    // sentences (Come stai? Hai fatto buon viaggio?).
    while tokens.last().is_some_and(|(t, _)| matches!(t.as_str(), "." | "?" | "!")) {
        tokens.pop();
    }
    tokens
}

/// Older texts write a final closed é with a grave accent (perchè, sè, nè);
/// modern spelling, and so the dictionary, has é.
fn modern_spelling(word: &str) -> String {
    let lower = word.to_lowercase();
    if lower.ends_with("chè") || lower == "sè" || lower == "nè" {
        let mut w = word.to_owned();
        w.truncate(w.len() - 'è'.len_utf8());
        w.push('é');
        return w;
    }
    word.to_owned()
}

pub fn is_punct(token: &str) -> bool {
    matches!(token, "," | ";" | ":" | "." | "?" | "!")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenizes_elision_and_punctuation() {
        assert_eq!(tokenize("L'ho visto, dell’anno scorso."), ["L'", "ho", "visto", ",", "dell'", "anno", "scorso"]);
    }

    #[test]
    fn marks_words_after_an_opening_quote() {
        assert_eq!(after_quote("FMI significa \"Fondo Monetario\"."), [false, false, true, false]);
        assert_eq!(after_quote("Disse «L'ho visto»"), [false, true, false, false]);
    }

    #[test]
    fn drops_dashes_and_modernizes_grave_accents() {
        assert_eq!(tokenize("Un altro, — un tedesco, — rise; perchè nè sè"), ["Un", "altro", ",", "un", "tedesco", ",", "rise", ";", "perché", "né", "sé"]);
        assert_eq!(tokenize("Il caffè"), ["Il", "caffè"]);
    }
}
