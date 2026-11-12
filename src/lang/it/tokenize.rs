//! Splitting Italian text into tokens: words, elided forms (`l'`, `dell'`)
//! and the punctuation the grammar uses.

pub fn tokenize(text: &str) -> Vec<String> {
    let text = text.replace('’', "'");
    let mut tokens = Vec::new();
    for raw in text.split_whitespace() {
        let word = raw.trim_start_matches(['"', '«', '(', '“']);
        let core = word.trim_end_matches(['.', ',', ';', ':', '!', '?', '"', '»', ')', '”']);
        let trailing = &word[core.len()..];
        let mut rest = core;
        while let Some(i) = rest.find('\'') {
            if i + 1 >= rest.len() {
                break;
            }
            tokens.push(rest[..=i].to_owned());
            rest = &rest[i + 1..];
        }
        if !rest.is_empty() {
            tokens.push(rest.to_owned());
        }
        tokens.extend(trailing.chars().filter(|c| matches!(c, ',' | ';' | ':' | '.' | '?' | '!')).map(|c| c.to_string()));
    }
    // Sentence-final punctuation ends the text; inside it, it separates
    // sentences (Come stai? Hai fatto buon viaggio?).
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
    fn tokenizes_elision_and_punctuation() {
        assert_eq!(tokenize("L'ho visto, dell’anno scorso."), ["L'", "ho", "visto", ",", "dell'", "anno", "scorso"]);
    }
}
