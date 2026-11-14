//! Word Holder's lemmatize contract as a [`Lexicon`](crate::morph::Lexicon)
//! (feature `wordholder`). Its forms come from Wiktionary via Kaikki.org; the
//! API never guesses a form it does not know.
//!
//! - The contract: the wire types of `/v1/lemmatize:batch` ([`BatchRequest`],
//!   [`BatchResponse`] and the types inside them) and their conversion to
//!   readings ([`analyses_from_batch`], [`reading_from_entry`]). Whoever
//!   answers lemmatize requests in process builds a [`BatchResponse`] and
//!   converts it, without the network.
//! - [`patches`]: the corrections for this data's known defects. They are
//!   written against the readings the conversion produces, so apply them to
//!   those readings, through [`PatchedLexicon`](crate::patch::PatchedLexicon).
//! - [`fixture`]: recorded API answers, for tests and offline runs.
//! - `client` (feature `wordholder-http`): the HTTP client, through RapidAPI
//!   or at the API's origin.

#[cfg(feature = "wordholder-http")]
#[cfg_attr(docsrs, doc(cfg(feature = "wordholder-http")))]
pub mod client;
mod dto;
pub mod fixture;

#[cfg(feature = "wordholder-http")]
#[cfg_attr(docsrs, doc(cfg(feature = "wordholder-http")))]
pub use client::{WordHolderLexicon, BATCH_ENV, KEY_ENV, URL_ENV};
pub use dto::{
    analyses_from_batch, reading_from_entry, BatchItem, BatchRequest, BatchResponse, BatchResult, Grammar,
    LemmaEntry, MAX_READINGS,
};
pub use fixture::FixtureLexicon;

use crate::morph::Lang;
use crate::patch::{PatchError, Patches};

const PATCHES_IT: &str = include_str!("../../data/it/lexicon-patches/wordholder.txt");
const PATCHES_ES: &str = include_str!("../../data/es/lexicon-patches/wordholder.txt");
const PATCHES_FR: &str = include_str!("../../data/fr/lexicon-patches/wordholder.txt");

/// The corrections for this lexicon's known defects
/// (`data/<lang>/lexicon-patches/wordholder.txt`).
pub fn patches() -> Result<Patches, PatchError> {
    let mut patches = Patches::parse(Lang::It, "data/it/lexicon-patches/wordholder.txt", PATCHES_IT)?;
    patches.add_file(Lang::Es, "data/es/lexicon-patches/wordholder.txt", PATCHES_ES)?;
    patches.add_file(Lang::Fr, "data/fr/lexicon-patches/wordholder.txt", PATCHES_FR)?;
    Ok(patches)
}
