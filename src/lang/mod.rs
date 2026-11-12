//! Languages: what each one provides ([`Language`]) and the ones there are.
//! Adding a language means a module like [`it`], its files under `data/<code>/`,
//! and a line in [`load`].

pub mod it;

use std::str::FromStr;

use crate::syntax::grammar::{Grammar, GrammarError};
use crate::syntax::lattice::Lattice;
use crate::morph::{Analysis, Lang};

pub trait Language: Send + Sync {
    fn lang(&self) -> Lang;
    fn grammar(&self) -> &Grammar;
    fn tokenize(&self, text: &str) -> Vec<String>;
    /// The forms to ask the lexicon about, for the word tokens.
    fn lexicon_forms(&self, tokens: &[String]) -> Vec<String>;
    /// The morpheme lattice from the tokens and the lexicon's analyses of
    /// `lexicon_forms(tokens)`, in the same order.
    fn lattice(&self, tokens: &[String], analyses: &[Analysis]) -> Lattice;
}

#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    #[error("{0} is not supported yet; supported: it")]
    Unsupported(&'static str),
    #[error(transparent)]
    Grammar(#[from] GrammarError),
    #[error(transparent)]
    Data(#[from] it::DataError),
}

pub fn load(lang: Lang) -> Result<Box<dyn Language>, LoadError> {
    match lang {
        Lang::It => Ok(Box::new(it::Italian::load()?)),
        other => Err(LoadError::Unsupported(other.code())),
    }
}

#[derive(Debug, thiserror::Error)]
#[error("unknown language code `{0}`; known: it, es, fr, de, nl, en")]
pub struct UnknownLang(String);

impl FromStr for Lang {
    type Err = UnknownLang;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "it" => Ok(Lang::It),
            "es" => Ok(Lang::Es),
            "fr" => Ok(Lang::Fr),
            "de" => Ok(Lang::De),
            "nl" => Ok(Lang::Nl),
            "en" => Ok(Lang::En),
            other => Err(UnknownLang(other.to_owned())),
        }
    }
}
