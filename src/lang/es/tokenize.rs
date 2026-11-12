//! Splitting Spanish text into tokens: words and the punctuation the grammar
//! uses. The opening ¿ and ¡ only repeat what the closing ? and ! say, so
//! they are dropped.

pub fn tokenize(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    for raw in text.split_whitespace() {
        let word = raw.trim_start_matches(['"', '«', '(', '“', '¿', '¡', '—', '-']);
        let core = word.trim_end_matches(['.', ',', ';', ':', '!', '?', '"', '»', ')', '”', '…', '—']);
        let trailing = &word[core.len()..];
        // ¿ or ¡ inside a word's leading run (",¿") is dropped the same way.
        let core = core.trim_start_matches(['¿', '¡']);
        if !core.is_empty() {
            tokens.push(core.to_owned());
        }
        tokens.extend(trailing.chars().filter(|c| matches!(c, ',' | ';' | ':' | '.' | '?' | '!')).map(|c| c.to_string()));
    }
    // Sentence-final punctuation ends the text; inside it, it separates
    // sentences (¿Vienes? Te espero).
    while tokens.last().is_some_and(|t| matches!(t.as_str(), "." | "?" | "!")) {
        tokens.pop();
    }
    tokens
}

pub fn is_punct(token: &str) -> bool {
    matches!(token, "," | ";" | ":" | "." | "?" | "!")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drops_inverted_marks_and_keeps_inner_punctuation() {
        assert_eq!(tokenize("¿Dónde está?"), ["Dónde", "está"]);
        assert_eq!(tokenize("Juan, ¡ven aquí!"), ["Juan", ",", "ven", "aquí"]);
        assert_eq!(tokenize("¿Vienes? Te espero."), ["Vienes", "?", "Te", "espero"]);
    }
}
