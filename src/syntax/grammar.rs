//! Grammar files: a context-free grammar whose categories carry feature
//! constraints. Syntax:
//!
//! ```text
//! start S
//! # comment
//! rule clause.subject: S -> NP[case=nom, person=?p] VP[finite=yes, person=?p]
//!   "The verb agrees with its subject in person."
//! ```
//!
//! A value is a constant (`nom`), a choice of constants (`acc|dat`) or a
//! variable (`?p`) that must take the same value everywhere in the rule.
//! `!=` negates: `prep!=di` refuses those constants, `person!=?o` the
//! variable's value.
//!
//! A child can carry the grammatical role of the phrase it stands for, after
//! its features: `NP[case=nom]:subject`, `Adv[lemma=non]:adverbial`. The
//! names are [`Role`]'s. `:case` gives the role by the child's `case`
//! feature, as a `case-roles` line maps it (a clitic pronoun):
//!
//! ```text
//! case-roles acc=direct-object, dat=indirect-object
//! rule clitics.other: Clitics -> Pron[clitic=yes]:case
//!   "A clitic pronoun."
//! ```
//!
//! Roles never take part in parsing; they only label the trees.

use std::collections::{BTreeMap, HashSet};

use crate::syntax::role::Role;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Value {
    Any(Vec<String>),
    Var(String),
    /// `k!=?v`: must differ from `?v` once both are known.
    NotVar(String),
    /// `k!=a|b`: must be none of these constants.
    NotAny(Vec<String>),
}

/// The role a rule gives one of its children.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoleMark {
    Role(Role),
    /// The role the `case-roles` line gives the child's `case` (none if it
    /// gives that case none).
    ByCase,
}

#[derive(Debug, Clone)]
pub struct Cat {
    pub name: String,
    pub constraints: Vec<(String, Value)>,
    /// Only on a rule's right side.
    pub role: Option<RoleMark>,
}

#[derive(Debug, Clone)]
pub struct Rule {
    pub name: String,
    pub lhs: Cat,
    pub rhs: Vec<Cat>,
    pub explanation: String,
}

#[derive(Debug)]
pub struct Grammar {
    pub start: String,
    pub rules: Vec<Rule>,
    /// The role a child marked `:case` takes for each value of its `case`.
    pub case_roles: BTreeMap<String, Role>,
    nonterminals: HashSet<String>,
}

#[derive(Debug, thiserror::Error)]
#[error("grammar line {line}: {message}")]
pub struct GrammarError {
    pub line: usize,
    pub message: String,
}

impl Grammar {
    pub fn is_nonterminal(&self, name: &str) -> bool {
        self.nonterminals.contains(name)
    }

    pub fn parse(text: &str) -> Result<Grammar, GrammarError> {
        let mut start = None;
        let mut rules: Vec<Rule> = Vec::new();
        let mut case_roles = BTreeMap::new();
        let mut by_case_line = None;
        for (i, raw) in text.lines().enumerate() {
            let line_no = i + 1;
            let err = |message: String| GrammarError { line: line_no, message };
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some(rest) = line.strip_prefix("start ") {
                start = Some(rest.trim().to_owned());
            } else if let Some(rest) = line.strip_prefix("case-roles ") {
                for pair in rest.split(',').map(str::trim).filter(|p| !p.is_empty()) {
                    let (case, role) = pair
                        .split_once('=')
                        .ok_or_else(|| err(format!("expected `case=role`, got `{pair}`")))?;
                    let role = Role::from_name(role.trim())
                        .ok_or_else(|| err(format!("unknown role `{}`", role.trim())))?;
                    case_roles.insert(case.trim().to_owned(), role);
                }
            } else if let Some(rest) = line.strip_prefix("rule ") {
                let (name, body) = rest
                    .split_once(':')
                    .ok_or_else(|| err("expected `rule name: LHS -> RHS`".to_owned()))?;
                let (lhs, rhs) = body
                    .split_once("->")
                    .ok_or_else(|| err("expected `->`".to_owned()))?;
                let lhs = parse_cats(lhs).map_err(&err)?;
                let rhs = parse_cats(rhs).map_err(&err)?;
                let [lhs] = <[Cat; 1]>::try_from(lhs)
                    .map_err(|_| err("the left side must be one category".to_owned()))?;
                if rhs.is_empty() {
                    return Err(err("empty right side".to_owned()));
                }
                if lhs.role.is_some() {
                    return Err(err("a role goes on a child, not on the left side".to_owned()));
                }
                if by_case_line.is_none() && rhs.iter().any(|c| c.role == Some(RoleMark::ByCase)) {
                    by_case_line = Some(line_no);
                }
                rules.push(Rule { name: name.trim().to_owned(), lhs, rhs, explanation: String::new() });
            } else if line.starts_with('"') && line.ends_with('"') && line.len() >= 2 {
                let rule = rules
                    .last_mut()
                    .ok_or_else(|| err("explanation before any rule".to_owned()))?;
                if !rule.explanation.is_empty() {
                    rule.explanation.push(' ');
                }
                rule.explanation.push_str(&line[1..line.len() - 1]);
            } else {
                return Err(err(format!("cannot read `{line}`")));
            }
        }
        let start = start.ok_or(GrammarError { line: 0, message: "no `start` line".to_owned() })?;
        if let (Some(line), true) = (by_case_line, case_roles.is_empty()) {
            return Err(GrammarError { line, message: "`:case` needs a `case-roles` line".to_owned() });
        }
        let nonterminals = rules.iter().map(|r| r.lhs.name.clone()).collect();
        Ok(Grammar { start, rules, case_roles, nonterminals })
    }
}

fn parse_cats(text: &str) -> Result<Vec<Cat>, String> {
    let mut cats = Vec::new();
    let mut rest = text.trim();
    while !rest.is_empty() {
        let name_end = rest
            .find(|c: char| c == '[' || c == ':' || c.is_whitespace())
            .unwrap_or(rest.len());
        let name = &rest[..name_end];
        if name.is_empty() || !name.chars().all(|c| c.is_alphanumeric() || c == '_') {
            return Err(format!("bad category name in `{rest}`"));
        }
        rest = &rest[name_end..];
        let mut constraints = Vec::new();
        if let Some(after) = rest.strip_prefix('[') {
            let close = after.find(']').ok_or_else(|| format!("missing `]` after {name}"))?;
            for pair in after[..close].split(',').map(str::trim).filter(|p| !p.is_empty()) {
                let (k, v, negated) = match pair.split_once("!=") {
                    Some((k, v)) => (k, v, true),
                    None => {
                        let (k, v) = pair
                            .split_once('=')
                            .ok_or_else(|| format!("expected `feature=value`, got `{pair}`"))?;
                        (k, v, false)
                    }
                };
                let v = v.trim();
                let value = match (v.strip_prefix('?'), negated) {
                    (Some(var), false) => Value::Var(var.to_owned()),
                    (Some(var), true) => Value::NotVar(var.to_owned()),
                    (None, false) => Value::Any(v.split('|').map(|s| s.trim().to_owned()).collect()),
                    (None, true) => Value::NotAny(v.split('|').map(|s| s.trim().to_owned()).collect()),
                };
                constraints.push((k.trim().to_owned(), value));
            }
            rest = &after[close + 1..];
        }
        let mut role = None;
        if let Some(after) = rest.strip_prefix(':') {
            let end = after.find(char::is_whitespace).unwrap_or(after.len());
            let mark = &after[..end];
            role = Some(match mark {
                "case" => RoleMark::ByCase,
                _ => RoleMark::Role(Role::from_name(mark).ok_or_else(|| format!("unknown role `{mark}` on {name}"))?),
            });
            rest = &after[end..];
        }
        cats.push(Cat { name: name.to_owned(), constraints, role });
        rest = rest.trim_start();
    }
    Ok(cats)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_rules_constraints_and_explanations() {
        let g = Grammar::parse(
            "start S\n# c\nrule a.b: S -> NP[case=nom, person=?p] VP[mood=ind|sub, person!=?q, prep!=di|a]\n  \"one\"\n  \"two\"\n",
        )
        .unwrap();
        let r = &g.rules[0];
        assert_eq!((r.name.as_str(), r.lhs.name.as_str(), r.rhs.len()), ("a.b", "S", 2));
        assert_eq!(r.rhs[0].constraints[1], ("person".to_owned(), Value::Var("p".to_owned())));
        assert_eq!(
            r.rhs[1].constraints[0].1,
            Value::Any(vec!["ind".to_owned(), "sub".to_owned()])
        );
        assert_eq!(r.rhs[1].constraints[1].1, Value::NotVar("q".to_owned()));
        assert_eq!(
            r.rhs[1].constraints[2].1,
            Value::NotAny(vec!["di".to_owned(), "a".to_owned()])
        );
        assert_eq!(r.explanation, "one two");
        assert!(g.is_nonterminal("S") && !g.is_nonterminal("NP"));
    }

    #[test]
    fn reads_roles_on_children() {
        let g = Grammar::parse(
            "start S\ncase-roles acc=direct-object, dat=indirect-object\nrule a: S -> NP[case=nom]:subject VP Pron:case Adv:adverbial\n",
        )
        .unwrap();
        let roles: Vec<_> = g.rules[0].rhs.iter().map(|c| c.role).collect();
        assert_eq!(
            roles,
            [Some(RoleMark::Role(Role::Subject)), None, Some(RoleMark::ByCase), Some(RoleMark::Role(Role::Adverbial))]
        );
        assert_eq!(g.rules[0].rhs[0].constraints.len(), 1);
        assert_eq!(g.case_roles.get("dat"), Some(&Role::IndirectObject));
    }

    #[test]
    fn rejects_bad_roles() {
        assert_eq!(Grammar::parse("start S\nrule a: S -> NP:object\n").unwrap_err().line, 2);
        assert_eq!(Grammar::parse("start S\nrule a: S:subject -> NP\n").unwrap_err().line, 2);
        assert_eq!(Grammar::parse("start S\nrule a: S -> Pron:case\n").unwrap_err().line, 2);
        assert_eq!(Grammar::parse("start S\ncase-roles acc=object\n").unwrap_err().line, 2);
    }

    #[test]
    fn rejects_garbage_with_a_line_number() {
        let e = Grammar::parse("start S\nrule x S -> A\n").unwrap_err();
        assert_eq!(e.line, 2);
    }
}
