//! Where each token is in the original text. The tokenizers return strings,
//! not offsets, and normalise a little on the way (’ → ', perchè → perché, a
//! dash or ¿ dropped, `-en` in va-t'en given a hyphen it did not have); the
//! tokens are found again in the text, in order, under the same folding.

use std::ops::Range;

#[derive(Debug, thiserror::Error)]
#[error("token {index} `{token}` is not in the text after byte {from}: {text}")]
pub struct AlignError {
    pub index: usize,
    pub token: String,
    pub from: usize,
    pub text: String,
}

/// The byte range of every token in `text`, in token order.
pub fn token_spans(text: &str, tokens: &[String]) -> Result<Vec<Range<usize>>, AlignError> {
    let chars: Vec<(usize, char)> = text.char_indices().map(|(i, c)| (i, fold(c))).collect();
    let byte_at = |i: usize| chars.get(i).map_or(text.len(), |c| c.0);
    let mut cursor = 0;
    let mut spans = Vec::with_capacity(tokens.len());
    for (index, token) in tokens.iter().enumerate() {
        let folded: Vec<char> = token.chars().map(fold).collect();
        // A French pronoun after a hyphen keeps one even where the text has
        // none (va-t'en → -t', -en).
        let mut variants = vec![folded.as_slice()];
        if folded.len() > 1 && folded[0] == '-' {
            variants.push(&folded[1..]);
        }
        let found = variants
            .iter()
            .filter_map(|v| find(&chars, cursor, v).map(|at| (at, v.len())))
            .min_by_key(|(at, _)| *at);
        let Some((at, len)) = found else {
            return Err(AlignError {
                index,
                token: token.clone(),
                from: byte_at(cursor),
                text: text.to_owned(),
            });
        };
        spans.push(byte_at(at)..byte_at(at + len));
        cursor = at + len;
    }
    Ok(spans)
}

/// The normalisations the tokenizers make inside a token.
fn fold(c: char) -> char {
    match c {
        '’' | 'ʼ' => '\'',
        // The tokenizer writes PERCHÈ as PERCHé.
        'è' | 'È' | 'É' => 'é',
        c => c,
    }
}

fn find(chars: &[(usize, char)], from: usize, needle: &[char]) -> Option<usize> {
    if needle.is_empty() || chars.len() < needle.len() {
        return None;
    }
    (from..=chars.len() - needle.len()).find(|&i| {
        chars[i..i + needle.len()]
            .iter()
            .map(|c| c.1)
            .eq(needle.iter().copied())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lang::{es, fr, it};

    fn pieces(text: &str, tokens: &[String]) -> Vec<String> {
        token_spans(text, tokens)
            .unwrap()
            .into_iter()
            .map(|r| text[r].to_owned())
            .collect()
    }

    #[test]
    fn italian_apostrophes_dashes_and_grave_accents() {
        let text = "L’ho visto — dell’anno scorso, perchè sì, PERCHÈ.";
        let tokens = it::tokenize(text);
        assert_eq!(
            tokens,
            [
                "L'", "ho", "visto", "dell'", "anno", "scorso", ",", "perché", "sì", ",", "PERCHé"
            ]
        );
        assert_eq!(
            pieces(text, &tokens),
            [
                "L’", "ho", "visto", "dell’", "anno", "scorso", ",", "perchè", "sì", ",", "PERCHÈ"
            ]
        );
    }

    #[test]
    fn italian_enclitic_is_one_span() {
        let text = "«Dammelo!», disse.";
        let tokens = it::tokenize(text);
        assert_eq!(pieces(text, &tokens), ["Dammelo", "!", ",", "disse"]);
    }

    #[test]
    fn repeated_forms_get_their_own_spans() {
        let text = "Fu lui, fu lei, fu.";
        let spans = token_spans(text, &it::tokenize(text)).unwrap();
        let fu: Vec<Range<usize>> = spans
            .into_iter()
            .filter(|r| text[r.clone()].eq_ignore_ascii_case("fu"))
            .collect();
        assert_eq!(fu, [0..2, 8..10, 16..18]);
    }

    #[test]
    fn spanish_inverted_marks_and_quotes() {
        let text = "¿Me lo das? ''Dámelo'', dijo.";
        let tokens = es::tokenize(text);
        assert_eq!(
            pieces(text, &tokens),
            ["Me", "lo", "das", "?", "Dámelo", ",", "dijo"]
        );
    }

    #[test]
    fn french_hyphens_euphonic_t_and_elision() {
        let text = "« Va-t’en », dit-il. A-t-il fini ?";
        let tokens = fr::tokenize(text);
        assert_eq!(
            tokens,
            [
                "Va", "-t'", "-en", ",", "dit", "-il", ".", "A", "-il", "fini", "?"
            ]
        );
        assert_eq!(
            pieces(text, &tokens),
            [
                "Va", "-t’", "en", ",", "dit", "-il", ".", "A", "-il", "fini", "?"
            ]
        );
    }

    #[test]
    fn a_token_not_in_the_text_is_an_error() {
        assert!(token_spans("ciao", &["addio".to_owned()]).is_err());
    }
}
