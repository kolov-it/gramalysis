//! syntango: morphological and syntactic analysis, with explanations.
//!
//! - [`morph`]: readings and features, and the [`morph::Lexicon`] trait any
//!   lexicon implements.
//! - [`syntax`]: grammar files, the morpheme lattice, the Earley parser.
//! - [`lang`]: the [`lang::Language`] trait and each language ([`lang::it`], [`lang::es`], [`lang::fr`]).
//! - [`patch`]: corrections to a lexicon's known defects, applied on top of it.
//! - [`analysis`]: the pipeline, [`analyze`].
//! - [`wordholder`] (feature `wordholder`): Word Holder's lemmatize contract
//!   and patches; its HTTP client needs feature `wordholder-http`.
//!
//! Only feature `wordholder-http` brings network code.

#![cfg_attr(docsrs, feature(doc_cfg))]

pub mod analysis;
pub mod lang;
pub mod morph;
pub mod patch;
pub mod syntax;
#[cfg(feature = "wordholder")]
#[cfg_attr(docsrs, doc(cfg(feature = "wordholder")))]
pub mod wordholder;

pub use analysis::{analyze, Error, SentenceAnalysis, TokenChoice, TokenRoles};
pub use syntax::role::Role;
