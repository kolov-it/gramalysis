//! The HTTP client: RapidAPI gateway or the API's origin.

use async_trait::async_trait;

use super::dto::{analyses_from_batch, BatchItem, BatchRequest, BatchResponse, MAX_READINGS};
use crate::morph::{Analysis, Lang, Lexicon, LexiconError};

/// Configuration, all required:
/// - `WORDHOLDER_API_URL`: `https://word-holder.p.rapidapi.com` (RapidAPI),
///   `https://public.wordholder.com` (our origin) or a local wordy_public.
/// - `WORDHOLDER_API_KEY`: the RapidAPI key when the URL is a RapidAPI host,
///   otherwise a bearer token for the origin (the admin token, or a local
///   dev token).
/// - `WORDHOLDER_API_BATCH`: forms per request (RapidAPI free plan: 10).
pub const URL_ENV: &str = "WORDHOLDER_API_URL";
pub const KEY_ENV: &str = "WORDHOLDER_API_KEY";
pub const BATCH_ENV: &str = "WORDHOLDER_API_BATCH";
enum Auth {
    /// RapidAPI gateway: key plus the gateway's host name.
    RapidApi { key: String, host: String },
    Bearer(String),
}

pub struct WordHolderLexicon {
    http: reqwest::Client,
    base_url: String,
    auth: Auth,
    batch_size: usize,
}

impl WordHolderLexicon {
    /// RapidAPI auth when `base_url`'s host ends in `.rapidapi.com`, bearer
    /// auth otherwise.
    pub fn new(base_url: &str, key: String, batch_size: usize) -> Result<Self, LexiconError> {
        if batch_size == 0 {
            return Err(LexiconError::Protocol("batch size must be at least 1".to_owned()));
        }
        let base_url = base_url.trim_end_matches('/').to_owned();
        let host = base_url
            .split_once("://")
            .map(|(_, rest)| rest.split(['/', ':']).next().unwrap_or(rest))
            .filter(|h| !h.is_empty())
            .ok_or_else(|| LexiconError::Protocol(format!("not a URL: {base_url}")))?
            .to_owned();
        let auth = if host.ends_with(".rapidapi.com") { Auth::RapidApi { key, host } } else { Auth::Bearer(key) };
        Ok(Self { http: reqwest::Client::new(), base_url, auth, batch_size })
    }

    pub fn from_env() -> Result<Self, LexiconError> {
        let var = |name: &str| std::env::var(name).map_err(|_| LexiconError::Protocol(format!("{name} is not set")));
        let batch = var(BATCH_ENV)?
            .parse()
            .map_err(|_| LexiconError::Protocol(format!("{BATCH_ENV} is not a number")))?;
        Self::new(&var(URL_ENV)?, var(KEY_ENV)?, batch)
    }

    pub fn batch_size(&self) -> usize {
        self.batch_size
    }

    fn post(&self, path: &str) -> reqwest::RequestBuilder {
        let request = self.http.post(format!("{}{path}", self.base_url));
        match &self.auth {
            Auth::RapidApi { key, host } => request.header("X-RapidAPI-Key", key).header("X-RapidAPI-Host", host),
            Auth::Bearer(token) => request.bearer_auth(token),
        }
    }

    /// Sends one chunk; the response body as JSON.
    async fn send(&self, lang: Lang, chunk: &[&str]) -> Result<serde_json::Value, LexiconError> {
        let request = BatchRequest {
            language: lang.code(),
            items: chunk.iter().enumerate().map(|(i, f)| BatchItem { id: i.to_string(), text: f }).collect(),
            limit: MAX_READINGS,
        };
        let response = self.post("/v1/lemmatize:batch").json(&request).send().await.map_err(transport)?;
        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.map_err(transport)?;
            return Err(LexiconError::Http { status: status.as_u16(), body });
        }
        response.json().await.map_err(transport)
    }

    /// The API's lemma entries per form, unconverted: for recording fixtures.
    pub async fn raw(&self, lang: Lang, forms: &[&str]) -> Result<Vec<(String, serde_json::Value)>, LexiconError> {
        let mut out = Vec::with_capacity(forms.len());
        for chunk in forms.chunks(self.batch_size) {
            let body = self.send(lang, chunk).await?;
            let results = body["results"].as_array().ok_or_else(|| LexiconError::Protocol("no results array".to_owned()))?;
            for (i, form) in chunk.iter().enumerate() {
                let result = results
                    .iter()
                    .find(|r| r["id"] == serde_json::Value::String(i.to_string()))
                    .ok_or_else(|| LexiconError::Protocol(format!("no result for {form}")))?;
                out.push(((*form).to_owned(), result["lemmas"].clone()));
            }
        }
        Ok(out)
    }
}

#[async_trait]
impl Lexicon for WordHolderLexicon {
    async fn analyze(&self, lang: Lang, forms: &[&str]) -> Result<Vec<Analysis>, LexiconError> {
        let mut out = Vec::with_capacity(forms.len());
        for chunk in forms.chunks(self.batch_size) {
            let body: BatchResponse = serde_json::from_value(self.send(lang, chunk).await?)
                .map_err(|e| LexiconError::Protocol(format!("unexpected response: {e}")))?;
            out.extend(analyses_from_batch(chunk, body)?);
        }
        Ok(out)
    }
}

fn transport(e: reqwest::Error) -> LexiconError {
    LexiconError::Transport(Box::new(e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auth_follows_the_host() {
        let r = WordHolderLexicon::new("https://word-holder.p.rapidapi.com/", "k".to_owned(), 10).unwrap();
        assert!(matches!(&r.auth, Auth::RapidApi { host, .. } if host == "word-holder.p.rapidapi.com"));
        assert_eq!(r.base_url, "https://word-holder.p.rapidapi.com");
        let o = WordHolderLexicon::new("http://localhost:8081", "t".to_owned(), 100).unwrap();
        assert!(matches!(o.auth, Auth::Bearer(_)));
        assert!(WordHolderLexicon::new("public.wordholder.com", "t".to_owned(), 1).is_err());
    }

    /// Calls the real API: `cargo test -- --ignored` with WORDHOLDER_API_* set.
    #[tokio::test]
    #[ignore]
    async fn live_italian_samples() {
        let lexicon = WordHolderLexicon::from_env().unwrap();
        let forms = ["fece", "ama", "ricordati", "dammelo", "cammello", "pomello", "del", "l'", "Tom"];
        for a in lexicon.analyze(Lang::It, &forms).await.unwrap() {
            println!("{}", a.form);
            for r in &a.readings {
                println!("  {} {:?} {:?} clitic={} {:?}", r.lemma, r.pos, r.features, r.clitic, r.raw_tags);
            }
        }
    }
}
