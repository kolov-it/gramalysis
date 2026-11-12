//! Earley parsing over a morpheme lattice, with feature constraints checked
//! as items advance. Returns every complete parse, not one best guess.

use std::collections::{BTreeMap, HashSet};

use serde::Serialize;

use crate::syntax::feats::FeatMap;
use crate::syntax::grammar::{Cat, Grammar, Value};
use crate::syntax::lattice::{Lattice, Morpheme};

type Env = BTreeMap<String, String>;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum Child {
    Item(usize),
    Edge(usize),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct Item {
    rule: usize,
    dot: usize,
    start: usize,
    end: usize,
    env: Env,
    children: Vec<Child>,
}

#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Tree {
    Node {
        cat: String,
        feats: FeatMap,
        rule: String,
        explanation: String,
        children: Vec<Tree>,
    },
    Leaf {
        token: usize,
        morph: Morpheme,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("the sentence is too ambiguous: more than {0} chart items")]
    TooManyItems(usize),
}

const MAX_ITEMS: usize = 500_000;

pub fn parse(grammar: &Grammar, lattice: &Lattice) -> Result<Vec<Tree>, ParseError> {
    let mut items: Vec<Item> = Vec::new();
    let mut seen: HashSet<Item> = HashSet::new();
    let mut chart: Vec<Vec<usize>> = vec![Vec::new(); lattice.nodes];

    let mut add = |item: Item, items: &mut Vec<Item>, chart: &mut Vec<Vec<usize>>| -> Result<(), ParseError> {
        if seen.contains(&item) {
            return Ok(());
        }
        if items.len() >= MAX_ITEMS {
            return Err(ParseError::TooManyItems(MAX_ITEMS));
        }
        seen.insert(item.clone());
        chart[item.end].push(items.len());
        items.push(item);
        Ok(())
    };

    for (r, rule) in grammar.rules.iter().enumerate() {
        if rule.lhs.name == grammar.start {
            add(Item { rule: r, dot: 0, start: 0, end: 0, env: Env::new(), children: vec![] }, &mut items, &mut chart)?;
        }
    }

    for node in 0..lattice.nodes {
        let mut i = 0;
        while i < chart[node].len() {
            let id = chart[node][i];
            i += 1;
            let item = items[id].clone();
            let rule = &grammar.rules[item.rule];
            match rule.rhs.get(item.dot) {
                None => {
                    // Complete: advance every item at `start` waiting for this category.
                    let feats = lhs_feats(&rule.lhs, &item.env);
                    let waiting: Vec<usize> = chart[item.start].clone();
                    for w in waiting {
                        let wi = &items[w];
                        let wrule = &grammar.rules[wi.rule];
                        let Some(next) = wrule.rhs.get(wi.dot) else { continue };
                        if next.name != rule.lhs.name {
                            continue;
                        }
                        if let Some(env) = unify(next, &feats, &wi.env) {
                            let mut children = wi.children.clone();
                            children.push(Child::Item(id));
                            let advanced = Item { rule: wi.rule, dot: wi.dot + 1, start: wi.start, end: node, env, children };
                            add(advanced, &mut items, &mut chart)?;
                        }
                    }
                }
                Some(next) if grammar.is_nonterminal(&next.name) => {
                    for (r, candidate) in grammar.rules.iter().enumerate() {
                        if candidate.lhs.name == next.name {
                            add(Item { rule: r, dot: 0, start: node, end: node, env: Env::new(), children: vec![] }, &mut items, &mut chart)?;
                        }
                    }
                }
                Some(next) => {
                    for (e, edge) in lattice.edges.iter().enumerate() {
                        if edge.from != node || edge.morph.cat != next.name {
                            continue;
                        }
                        let mut feats = edge.morph.feats.clone();
                        feats.insert("lemma".to_owned(), edge.morph.lemma.clone());
                        if let Some(env) = unify(next, &feats, &item.env) {
                            let mut children = item.children.clone();
                            children.push(Child::Edge(e));
                            let advanced = Item { rule: item.rule, dot: item.dot + 1, start: item.start, end: edge.to, env, children };
                            add(advanced, &mut items, &mut chart)?;
                        }
                    }
                }
            }
        }
    }

    let last = lattice.last_node();
    Ok(chart[last]
        .iter()
        .map(|&id| &items[id])
        .filter(|it| {
            let rule = &grammar.rules[it.rule];
            it.start == 0 && rule.lhs.name == grammar.start && it.dot == rule.rhs.len()
        })
        .map(|it| build(grammar, lattice, &items, it))
        .collect())
}

/// Checks `cat`'s constraints against `feats`, extending `env`. A feature the
/// morpheme or phrase does not have is unknown and passes.
fn unify(cat: &Cat, feats: &FeatMap, env: &Env) -> Option<Env> {
    let mut env = env.clone();
    for (key, value) in &cat.constraints {
        let Some(actual) = feats.get(key) else { continue };
        match value {
            Value::Any(allowed) => {
                if !allowed.iter().any(|a| a == actual) {
                    return None;
                }
            }
            Value::Var(var) => match env.get(var) {
                Some(bound) if bound != actual => return None,
                Some(_) => {}
                None => {
                    env.insert(var.clone(), actual.clone());
                }
            },
            Value::NotVar(var) => {
                if env.get(var) == Some(actual) {
                    return None;
                }
            }
        }
    }
    Some(env)
}

fn lhs_feats(lhs: &Cat, env: &Env) -> FeatMap {
    let mut feats = FeatMap::new();
    for (key, value) in &lhs.constraints {
        let resolved = match value {
            Value::Any(allowed) if allowed.len() == 1 => Some(allowed[0].clone()),
            Value::Any(_) | Value::NotVar(_) => None,
            Value::Var(var) => env.get(var).cloned(),
        };
        if let Some(v) = resolved {
            feats.insert(key.clone(), v);
        }
    }
    feats
}

fn build(grammar: &Grammar, lattice: &Lattice, items: &[Item], item: &Item) -> Tree {
    let rule = &grammar.rules[item.rule];
    let children = item
        .children
        .iter()
        .map(|c| match c {
            Child::Item(id) => build(grammar, lattice, items, &items[*id]),
            Child::Edge(e) => Tree::Leaf { token: lattice.edges[*e].token, morph: lattice.edges[*e].morph.clone() },
        })
        .collect();
    Tree::Node {
        cat: rule.lhs.name.clone(),
        feats: lhs_feats(&rule.lhs, &item.env),
        rule: rule.name.clone(),
        explanation: rule.explanation.clone(),
        children,
    }
}

impl Tree {
    /// The morphemes the parse chose, in sentence order.
    pub fn leaves(&self) -> Vec<(usize, &Morpheme)> {
        match self {
            Tree::Leaf { token, morph } => vec![(*token, morph)],
            Tree::Node { children, .. } => children.iter().flat_map(|c| c.leaves()).collect(),
        }
    }

    pub fn rules(&self) -> Vec<(&str, &str)> {
        match self {
            Tree::Leaf { .. } => vec![],
            Tree::Node { rule, explanation, children, .. } => {
                let mut out = vec![(rule.as_str(), explanation.as_str())];
                out.extend(children.iter().flat_map(|c| c.rules()));
                out
            }
        }
    }

    pub fn render(&self) -> String {
        let mut out = String::new();
        self.render_into(&mut out, 0);
        out
    }

    fn render_into(&self, out: &mut String, depth: usize) {
        let pad = "  ".repeat(depth);
        match self {
            Tree::Leaf { morph, .. } => {
                out.push_str(&format!(
                    "{pad}{} {} ({} {})\n",
                    morph.cat,
                    morph.surface,
                    morph.lemma,
                    crate::syntax::feats::display(&morph.feats)
                ));
            }
            Tree::Node { cat, feats, rule, children, .. } => {
                out.push_str(&format!("{pad}{cat}[{}]  ← {rule}\n", crate::syntax::feats::display(feats)));
                for c in children {
                    c.render_into(out, depth + 1);
                }
            }
        }
    }
}
