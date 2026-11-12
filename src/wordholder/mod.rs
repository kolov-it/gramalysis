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

use crate::morph::Lang;
use crate::patch::{PatchError, Patches};

const PATCHES_IT: &str = include_str!("../../data/it/lexicon-patches/wordholder.txt");
const PATCHES_FR: &str = include_str!("../../data/fr/lexicon-patches/wordholder.txt");

/// The corrections for this lexicon's known defects
/// (`data/<lang>/lexicon-patches/wordholder.txt`).
pub fn patches() -> Result<Patches, PatchError> {
    let mut patches = Patches::parse(Lang::It, "data/it/lexicon-patches/wordholder.txt", PATCHES_IT)?;
    patches.add_file(Lang::Fr, "data/fr/lexicon-patches/wordholder.txt", PATCHES_FR)?;
    Ok(patches)
}
