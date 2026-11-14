//! Grammatical roles: what a phrase does in its clause (subject, verb, direct
//! object…). The grammar declares them on a rule's children
//! (`NP[case=nom]:subject`); the parser carries them into the trees, and
//! [`crate::SentenceAnalysis::roles`] gives one per token.

use std::fmt;

use serde::{Deserialize, Serialize};

/// The role of a phrase in its clause. Serialized in kebab-case
/// (`direct-object`). More roles may be added, so match with a wildcard.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum Role {
    /// La ragazza ama il cane; è arrivato Marco.
    Subject,
    /// The verb group: ama; ha mangiato; sta leggendo; è amato.
    Verb,
    /// ama il cane; l'ho fatto (a clitic with `case=acc`).
    DirectObject,
    /// gli ho detto (a clitic with `case=dat`).
    IndirectObject,
    /// è un graffio; le luci sono accese.
    SubjectComplement,
    /// rendere le cose peggiori.
    ObjectComplement,
    /// è amato da tutti.
    Agent,
    /// Prepositional phrases, adverbs, time expressions: a casa, ieri, non.
    Adverbial,
}

impl Role {
    pub const ALL: [Role; 8] = [
        Role::Subject,
        Role::Verb,
        Role::DirectObject,
        Role::IndirectObject,
        Role::SubjectComplement,
        Role::ObjectComplement,
        Role::Agent,
        Role::Adverbial,
    ];

    /// The kebab-case name, as in grammar files and JSON.
    pub fn name(self) -> &'static str {
        match self {
            Role::Subject => "subject",
            Role::Verb => "verb",
            Role::DirectObject => "direct-object",
            Role::IndirectObject => "indirect-object",
            Role::SubjectComplement => "subject-complement",
            Role::ObjectComplement => "object-complement",
            Role::Agent => "agent",
            Role::Adverbial => "adverbial",
        }
    }

    pub fn from_name(name: &str) -> Option<Role> {
        Role::ALL.into_iter().find(|r| r.name() == name)
    }
}

impl fmt::Display for Role {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_round_trip() {
        for r in Role::ALL {
            assert_eq!(Role::from_name(r.name()), Some(r));
        }
        assert_eq!(Role::from_name("object"), None);
    }

    #[cfg(feature = "wordholder")]
    #[test]
    fn serializes_as_the_grammar_name() {
        for r in Role::ALL {
            let json = serde_json::to_string(&r).unwrap();
            assert_eq!(json, format!("\"{}\"", r.name()));
            assert_eq!(serde_json::from_str::<Role>(&json).unwrap(), r);
        }
    }
}
