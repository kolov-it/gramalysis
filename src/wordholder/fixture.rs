//! A [`Lexicon`] answered from a JSON file of recorded API responses:
//! `{"it": {"fece": [<lemma entries as the Word Holder API returns them>]}}`.
//! Lets tests and offline runs use real answers without the network.

use std::collections::HashMap;
use std::path::Path;

use async_trait::async_trait;

use crate::morph::{Analysis, Lang, Lexicon, LexiconError};
use super::dto::{reading_from_entry, LemmaEntry};

pub struct FixtureLexicon {
    entries: HashMap<String, HashMap<String, serde_json::Value>>,
}

impl FixtureLexicon {
    pub fn load(path: &Path) -> Result<Self, LexiconError> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| LexiconError::Protocol(format!("{}: {e}", path.display())))?;
        let entries = serde_json::from_str(&text)
            .map_err(|e| LexiconError::Protocol(format!("{}: {e}", path.display())))?;
        Ok(Self { entries })
    }
}

#[async_trait]
impl Lexicon for FixtureLexicon {
    async fn analyze(&self, lang: Lang, forms: &[&str]) -> Result<Vec<Analysis>, LexiconError> {
        let table = self.entries.get(lang.code());
        forms
            .iter()
            .map(|form| {
                let readings = match table.and_then(|t| t.get(*form)) {
                    None => Vec::new(),
                    Some(value) => {
                        let entries: Vec<LemmaEntry> = serde_json::from_value(value.clone())
                            .map_err(|e| LexiconError::Protocol(format!("fixture entry {form}: {e}")))?;
                        entries.into_iter().map(reading_from_entry).collect()
                    }
                };
                Ok(Analysis { form: (*form).to_owned(), readings })
            })
            .collect()
    }
}
