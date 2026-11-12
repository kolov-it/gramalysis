//! The function-word list (`data/it/closed.txt`): articles, prepositions,
//! pronouns, conjunctions, each with exact features.

use std::collections::HashMap;

use super::DataError;
use crate::syntax::feats::FeatMap;
use crate::syntax::lattice::Morpheme;

/// Form → its readings; a reading is one or more morphemes (`del` = di + il).
pub type FunctionWords = HashMap<String, Vec<Vec<Morpheme>>>;

/// The reading with the token's own spelling on it. A contraction's whole
/// token stands on its first morpheme.
pub fn with_surface(reading: &[Morpheme], token: &str) -> Vec<Morpheme> {
    if reading.len() == 1 {
        let mut m = reading[0].clone();
        m.surface = token.to_owned();
        return vec![m];
    }
    reading
        .iter()
        .enumerate()
        .map(|(i, m)| Morpheme { surface: if i == 0 { token.to_owned() } else { String::new() }, ..m.clone() })
        .collect()
}

pub fn parse(text: &str) -> Result<FunctionWords, DataError> {
    let mut out = FunctionWords::new();
    for (i, raw) in text.lines().enumerate() {
        let err = |message: String| DataError::Closed { line: i + 1, message };
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (form, body) = line.split_once(" = ").ok_or_else(|| err("expected `form = reading`".to_owned()))?;
        let mut morphs = Vec::new();
        for piece in body.split(" + ") {
            let piece = piece.trim();
            let (lemma, rest) = piece.split_once(':').ok_or_else(|| err(format!("expected `lemma:Cat` in `{piece}`")))?;
            let (cat, feats_text) = match rest.split_once('{') {
                Some((cat, f)) => (cat, f.strip_suffix('}').ok_or_else(|| err(format!("missing `}}` in `{piece}`")))?),
                None => (rest, ""),
            };
            let mut feats = FeatMap::new();
            for pair in feats_text.split(',').filter(|p| !p.is_empty()) {
                let (k, v) = pair.split_once('=').ok_or_else(|| err(format!("expected `k=v`, got `{pair}`")))?;
                feats.insert(k.to_owned(), v.to_owned());
            }
            morphs.push(Morpheme { surface: lemma.to_owned(), lemma: lemma.to_owned(), cat: cat.to_owned(), feats });
        }
        out.entry(form.trim().to_owned()).or_default().push(morphs);
    }
    Ok(out)
}
