//! gramalysis: morphological and syntactic analysis, with explanations.
//!
//! - [`morph`]: readings and features, and the [`morph::Lexicon`] trait any
//!   lexicon implements.
//! - [`syntax`]: grammar files, the morpheme lattice, the Earley parser.
//! - [`lang`]: the [`lang::Language`] trait and each language ([`lang::it`]).
//! - [`analysis`]: the pipeline, [`analyze`].
//! - [`wordholder`] (feature `wordholder`): the Word Holder API as a lexicon.
//!
//! Everything but `wordholder` is free of network code.

pub mod analysis;
pub mod lang;
pub mod morph;
pub mod syntax;
#[cfg(feature = "wordholder")]
pub mod wordholder;

pub use analysis::{analyze, Error, SentenceAnalysis, TokenChoice};
