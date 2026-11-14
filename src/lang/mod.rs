//! Languages: what each one provides ([`Language`]) and the ones there are.
//! Adding a language means a module like [`it`] or [`fr`], its files under `data/<code>/`,
//! and a line in [`load`].

pub mod es;
pub mod fr;
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
    /// The morpheme lattice from the tokens (`tokenize(text)`) and the
    /// lexicon's analyses of `lexicon_forms(tokens)`, in the same order. The
    /// text tells what the tokens lost, such as quotes.
    fn lattice(&self, text: &str, tokens: &[String], analyses: &[Analysis]) -> Lattice;
}

/// What a word's case says about it being a name. The lexicon is asked in
/// lower case, so it answers `maria` with the name Maria as well as the noun
/// (marijuana), and `amato` with the surname Amato as well as the participle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Case {
    /// A capital here says nothing: the first word of a sentence, a word
    /// inside a title, or one the language exempts (Italian: after an
    /// article, la Scozia).
    Unmarked,
    /// Lower case in mid-sentence: never a name.
    Lower,
    /// Capitalised in mid-sentence: a name, when the lexicon knows it as one.
    Capital,
}

impl Case {
    /// The case of `tokens[i]`. A word after `.`, `?`, `!` or `:` starts a
    /// sentence, like the first word; `unmarked` exempts it for a reason the
    /// language knows (an opening quote before it, which the tokens lost).
    /// In a title-cased run the capitals mark the whole title, not each word
    /// (Fondo Monetario Internazionale): only its first word is `Capital`.
    pub(crate) fn of(tokens: &[String], i: usize, unmarked: bool) -> Self {
        let starts = |i: usize| i == 0 || matches!(tokens[i - 1].as_str(), "." | "?" | "!" | ":");
        let capitalised = |i: usize| tokens[i].chars().next().is_some_and(char::is_uppercase);
        let in_title = i > 0 && !starts(i - 1) && capitalised(i - 1);
        if unmarked || starts(i) || in_title {
            Case::Unmarked
        } else if capitalised(i) {
            Case::Capital
        } else {
            Case::Lower
        }
    }

    /// Whether a lexicon reading of the word stands: `name` says the reading
    /// is a name, `known_name` that the lexicon has a name reading for the
    /// word at all. A capitalised word with none keeps its other readings (a
    /// title-cased word, a sentence after a dash).
    pub(crate) fn keeps(self, name: bool, known_name: bool) -> bool {
        match self {
            Case::Unmarked => true,
            Case::Lower => !name,
            Case::Capital => name || !known_name,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    #[error("{0} is not supported yet; supported: it, es, fr")]
    Unsupported(&'static str),
    #[error(transparent)]
    Grammar(#[from] GrammarError),
    #[error(transparent)]
    Data(#[from] it::DataError),
    #[error(transparent)]
    DataEs(#[from] es::DataError),
    #[error(transparent)]
    DataFr(#[from] fr::DataError),
}

pub fn load(lang: Lang) -> Result<Box<dyn Language>, LoadError> {
    match lang {
        Lang::It => Ok(Box::new(it::Italian::load()?)),
        Lang::Es => Ok(Box::new(es::Spanish::load()?)),
        Lang::Fr => Ok(Box::new(fr::French::load()?)),
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

#[cfg(test)]
mod tests {
    use super::*;

    fn tokens(text: &str) -> Vec<String> {
        text.split(' ').map(str::to_owned).collect()
    }

    #[test]
    fn case_of_a_word() {
        let t = tokens("Maria ha visto Rosa e rosa . Rosa dorme : Rosa Bianca");
        let case = |i| Case::of(&t, i, false);
        assert_eq!(case(0), Case::Unmarked);
        assert_eq!(case(3), Case::Capital);
        assert_eq!(case(5), Case::Lower);
        assert_eq!(case(7), Case::Unmarked);
        assert_eq!(case(10), Case::Unmarked);
        // Bianca follows a capital that starts a sentence: not a title.
        assert_eq!(case(11), Case::Capital);
        assert_eq!(Case::of(&t, 3, true), Case::Unmarked);
        let t = tokens("il Fondo Monetario Internazionale");
        assert_eq!(Case::of(&t, 1, false), Case::Capital);
        assert_eq!(Case::of(&t, 3, false), Case::Unmarked);
    }

    #[test]
    fn readings_by_case() {
        // (name reading, the word has one)
        assert!(Case::Unmarked.keeps(false, true) && Case::Unmarked.keeps(true, true));
        assert!(!Case::Lower.keeps(true, true) && Case::Lower.keeps(false, true));
        assert!(Case::Capital.keeps(true, true) && !Case::Capital.keeps(false, true));
        assert!(Case::Capital.keeps(false, false));
    }
}
