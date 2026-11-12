//! The Word Holder API as a [`Lexicon`](crate::morph::Lexicon) (feature
//! `wordholder`). Its forms come from Wiktionary via Kaikki.org; the API never
//! guesses a form it does not know.
//!
//! - [`client`]: the HTTP client, through RapidAPI or at the API's origin.
//! - [`fixture`]: recorded API answers, for tests and offline runs.
//! - `dto`: the wire format and its conversion to readings.

pub mod client;
mod dto;
pub mod fixture;

pub use client::{WordHolderLexicon, BATCH_ENV, KEY_ENV, URL_ENV};
pub use fixture::FixtureLexicon;
