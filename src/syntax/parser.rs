//! Earley parsing over a morpheme lattice, with feature constraints checked
//! as items advance. Equal sub-parses are stored once (a shared parse
//! forest), so the work stays polynomial however many parses a sentence has;
//! every parse can still be counted, listed, or asked which readings it uses.

use std::collections::{BTreeMap, HashMap, HashSet};

use serde::Serialize;

use crate::syntax::feats::FeatMap;
use crate::syntax::grammar::{Cat, Grammar, RoleMark, Rule, Value};
use crate::syntax::lattice::{Lattice, Morpheme};
use crate::syntax::role::Role;

type Env = BTreeMap<String, String>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Child {
    Item(usize),
    Edge(usize),
}

/// A dotted rule over a span with its variable bindings. Two derivations with
/// the same key are the same item; each keeps a back-pointer.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct Key {
    rule: usize,
    dot: usize,
    start: usize,
    end: usize,
    env: Env,
}

/// How an item with dot > 0 was reached: the item before its last step
/// (dot - 1), and what that step consumed.
type BackPointer = (usize, Child);

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Tree {
    Node {
        cat: String,
        feats: FeatMap,
        rule: String,
        explanation: String,
        /// The role the parent's rule gives this phrase.
        #[serde(skip_serializing_if = "Option::is_none")]
        role: Option<Role>,
        children: Vec<Tree>,
    },
    Leaf {
        token: usize,
        morph: Morpheme,
        /// The role the parent's rule gives this word.
        #[serde(skip_serializing_if = "Option::is_none")]
        role: Option<Role>,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("the sentence is too ambiguous: more than {0} chart items")]
    TooManyItems(usize),
}

const MAX_ITEMS: usize = 2_000_000;

/// A complete chart item some parse uses ([`Forest::items_where`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChartItem {
    pub id: usize,
    /// Index into the grammar's rules.
    pub rule: usize,
    /// Lattice nodes it spans.
    pub start: usize,
    pub end: usize,
    /// Its category's features, as the rule set them.
    pub feats: FeatMap,
    /// Per right-hand-side position, every child any of its derivations has
    /// there: alternatives side by side, never multiplied out.
    pub children: Vec<Vec<ItemChild>>,
    /// Where parses use it: (rule, right-hand-side position) of each item
    /// that has it as a child. Empty for a whole-sentence item.
    pub used_as: Vec<(usize, usize)>,
}

/// A child of a chart item: a lattice edge or another chart item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ItemChild {
    Edge(usize),
    Item(usize),
}

/// Every parse of a sentence, shared.
pub struct Forest<'g> {
    grammar: &'g Grammar,
    keys: Vec<Key>,
    back: Vec<Vec<BackPointer>>,
    roots: Vec<usize>,
}

pub fn parse<'g>(grammar: &'g Grammar, lattice: &Lattice) -> Result<Forest<'g>, ParseError> {
    let mut keys: Vec<Key> = Vec::new();
    let mut back: Vec<Vec<BackPointer>> = Vec::new();
    let mut index: HashMap<Key, usize> = HashMap::new();
    let mut chart: Vec<Vec<usize>> = vec![Vec::new(); lattice.nodes];
    // Items at a node waiting for a category, for completion.
    let mut waiting: Vec<HashMap<String, Vec<usize>>> = vec![HashMap::new(); lattice.nodes];

    let mut add = |key: Key,
                   bp: Option<BackPointer>,
                   keys: &mut Vec<Key>,
                   back: &mut Vec<Vec<BackPointer>>,
                   chart: &mut Vec<Vec<usize>>,
                   waiting: &mut Vec<HashMap<String, Vec<usize>>>|
     -> Result<(), ParseError> {
        if let Some(&id) = index.get(&key) {
            if let Some(bp) = bp {
                back[id].push(bp);
            }
            return Ok(());
        }
        if keys.len() >= MAX_ITEMS {
            return Err(ParseError::TooManyItems(MAX_ITEMS));
        }
        let id = keys.len();
        let rule = &grammar.rules[key.rule];
        if let Some(next) = rule.rhs.get(key.dot) {
            waiting[key.end].entry(next.name.clone()).or_default().push(id);
        }
        chart[key.end].push(id);
        index.insert(key.clone(), id);
        keys.push(key);
        back.push(bp.into_iter().collect());
        Ok(())
    };

    for (r, rule) in grammar.rules.iter().enumerate() {
        if rule.lhs.name == grammar.start {
            let key = Key { rule: r, dot: 0, start: 0, end: 0, env: Env::new() };
            add(key, None, &mut keys, &mut back, &mut chart, &mut waiting)?;
        }
    }

    let mut predicted: Vec<HashSet<String>> = vec![HashSet::new(); lattice.nodes];
    for node in 0..lattice.nodes {
        let mut i = 0;
        while i < chart[node].len() {
            let id = chart[node][i];
            i += 1;
            let key = keys[id].clone();
            let rule = &grammar.rules[key.rule];
            match rule.rhs.get(key.dot) {
                None => {
                    let feats = lhs_feats(&rule.lhs, &key.env);
                    let candidates = waiting[key.start].get(&rule.lhs.name).cloned().unwrap_or_default();
                    for w in candidates {
                        let wk = keys[w].clone();
                        let next = &grammar.rules[wk.rule].rhs[wk.dot];
                        if let Some(env) = unify(next, &feats, &wk.env) {
                            let advanced = Key { rule: wk.rule, dot: wk.dot + 1, start: wk.start, end: node, env };
                            add(advanced, Some((w, Child::Item(id))), &mut keys, &mut back, &mut chart, &mut waiting)?;
                        }
                    }
                }
                Some(next) if grammar.is_nonterminal(&next.name) => {
                    if predicted[node].insert(next.name.clone()) {
                        for (r, candidate) in grammar.rules.iter().enumerate() {
                            if candidate.lhs.name == next.name {
                                let key = Key { rule: r, dot: 0, start: node, end: node, env: Env::new() };
                                add(key, None, &mut keys, &mut back, &mut chart, &mut waiting)?;
                            }
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
                        if let Some(env) = unify(next, &feats, &key.env) {
                            let advanced = Key { rule: key.rule, dot: key.dot + 1, start: key.start, end: edge.to, env };
                            add(advanced, Some((id, Child::Edge(e))), &mut keys, &mut back, &mut chart, &mut waiting)?;
                        }
                    }
                }
            }
        }
    }

    let last = lattice.last_node();
    let roots = chart[last]
        .iter()
        .copied()
        .filter(|&id| {
            let k = &keys[id];
            let rule = &grammar.rules[k.rule];
            k.start == 0 && rule.lhs.name == grammar.start && k.dot == rule.rhs.len()
        })
        .collect();
    Ok(Forest { grammar, keys, back, roots })
}

impl Forest<'_> {
    pub fn is_empty(&self) -> bool {
        self.roots.is_empty()
    }

    /// How many complete parses there are (saturating).
    pub fn count(&self) -> u64 {
        let mut memo = HashMap::new();
        self.roots.iter().fold(0u64, |acc, &r| acc.saturating_add(self.count_item(r, &mut memo)))
    }

    fn count_item(&self, id: usize, memo: &mut HashMap<usize, u64>) -> u64 {
        if self.keys[id].dot == 0 {
            return 1;
        }
        if let Some(&n) = memo.get(&id) {
            return n;
        }
        let mut total = 0u64;
        for &(prev, child) in &self.back[id] {
            let c = match child {
                Child::Item(c) => self.count_item(c, memo),
                Child::Edge(_) => 1,
            };
            total = total.saturating_add(self.count_item(prev, memo).saturating_mul(c));
        }
        memo.insert(id, total);
        total
    }

    /// Lattice edges (morpheme readings) used by at least one parse.
    pub fn used_edges(&self) -> HashSet<usize> {
        let mut seen = HashSet::new();
        let mut edges = HashSet::new();
        let mut stack: Vec<usize> = self.roots.clone();
        while let Some(id) = stack.pop() {
            if !seen.insert(id) {
                continue;
            }
            for &(prev, child) in &self.back[id] {
                stack.push(prev);
                match child {
                    Child::Item(c) => stack.push(c),
                    Child::Edge(e) => {
                        edges.insert(e);
                    }
                }
            }
        }
        edges
    }

    /// For each lattice edge any parse uses, the rules that consumed it
    /// directly (`ap.participle` for a participle used like an adjective,
    /// `v.perfect-essere` for one in a compound tense, …).
    pub fn edge_rules(&self) -> HashMap<usize, HashSet<String>> {
        let mut seen = HashSet::new();
        let mut out: HashMap<usize, HashSet<String>> = HashMap::new();
        let mut stack: Vec<usize> = self.roots.clone();
        while let Some(id) = stack.pop() {
            if !seen.insert(id) {
                continue;
            }
            let rule = &self.grammar.rules[self.keys[id].rule].name;
            for &(prev, child) in &self.back[id] {
                stack.push(prev);
                match child {
                    Child::Item(c) => stack.push(c),
                    Child::Edge(e) => {
                        out.entry(e).or_default().insert(rule.clone());
                    }
                }
            }
        }
        out
    }

    /// The complete items some parse uses whose rule `keep`s, each with its
    /// children per position. A scan of the chart: the work is bounded by
    /// the chart's size, however many parses share the items.
    pub fn items_where(&self, keep: impl Fn(&Rule) -> bool) -> Vec<ChartItem> {
        let mut seen = HashSet::new();
        let mut out = Vec::new();
        let mut used_as: HashMap<usize, Vec<(usize, usize)>> = HashMap::new();
        let mut stack: Vec<usize> = self.roots.clone();
        while let Some(id) = stack.pop() {
            if !seen.insert(id) {
                continue;
            }
            let key = &self.keys[id];
            let rule = &self.grammar.rules[key.rule];
            if key.dot == rule.rhs.len() && keep(rule) {
                out.push(ChartItem {
                    id,
                    rule: key.rule,
                    start: key.start,
                    end: key.end,
                    feats: lhs_feats(&rule.lhs, &key.env),
                    children: self.children(id),
                    used_as: Vec::new(),
                });
            }
            for &(prev, child) in &self.back[id] {
                stack.push(prev);
                if let Child::Item(c) = child {
                    let at = (key.rule, key.dot - 1);
                    let uses = used_as.entry(c).or_default();
                    if !uses.contains(&at) {
                        uses.push(at);
                    }
                    stack.push(c);
                }
            }
        }
        for item in &mut out {
            item.used_as = used_as.remove(&item.id).unwrap_or_default();
        }
        out.sort_by_key(|i| i.id);
        out
    }

    /// Per position, the distinct children of an item's derivations.
    fn children(&self, id: usize) -> Vec<Vec<ItemChild>> {
        let mut out: Vec<Vec<ItemChild>> = vec![Vec::new(); self.keys[id].dot];
        let mut seen = HashSet::new();
        let mut stack = vec![id];
        while let Some(i) = stack.pop() {
            if !seen.insert(i) || self.keys[i].dot == 0 {
                continue;
            }
            let at = self.keys[i].dot - 1;
            for &(prev, child) in &self.back[i] {
                let part = match child {
                    Child::Edge(e) => ItemChild::Edge(e),
                    Child::Item(c) => ItemChild::Item(c),
                };
                if !out[at].contains(&part) {
                    out[at].push(part);
                }
                stack.push(prev);
            }
        }
        out
    }

    /// Up to `limit` parses as trees.
    pub fn trees(&self, lattice: &Lattice, limit: usize) -> Vec<Tree> {
        self.parses(lattice, limit).into_iter().map(|(t, _)| t).collect()
    }

    /// Up to `limit` parses, each as a tree and as the chart items it is
    /// built from ([`Derivation`]), in the order of [`Forest::trees`].
    pub fn parses(&self, lattice: &Lattice, limit: usize) -> Vec<(Tree, Derivation)> {
        let mut out = Vec::new();
        for &r in &self.roots {
            for t in self.item_trees(r, lattice, limit - out.len().min(limit)) {
                if out.len() >= limit {
                    return out;
                }
                out.push(t);
            }
        }
        out
    }

    fn item_trees(&self, id: usize, lattice: &Lattice, limit: usize) -> Vec<(Tree, Derivation)> {
        let key = &self.keys[id];
        let rule = &self.grammar.rules[key.rule];
        self.children_lists(id, lattice, limit)
            .into_iter()
            .map(|(children, picked, mut derivation)| {
                derivation.items.insert(id, DerivedItem { rule: key.rule, children: picked });
                let tree = Tree::Node {
                    cat: rule.lhs.name.clone(),
                    feats: lhs_feats(&rule.lhs, &key.env),
                    rule: rule.name.clone(),
                    explanation: rule.explanation.clone(),
                    role: None,
                    children,
                };
                (tree, derivation)
            })
            .collect()
    }

    /// The children of an item, for up to `limit` derivations: as trees, as
    /// the chart children they are, and the items below them.
    fn children_lists(&self, id: usize, lattice: &Lattice, limit: usize) -> Vec<(Vec<Tree>, Vec<ItemChild>, Derivation)> {
        if self.keys[id].dot == 0 || limit == 0 {
            return vec![(Vec::new(), Vec::new(), Derivation::default())];
        }
        let key = &self.keys[id];
        let mark = self.grammar.rules[key.rule].rhs[key.dot - 1].role;
        let mut out = Vec::new();
        for &(prev, child) in &self.back[id] {
            let lasts: Vec<(Tree, ItemChild, Derivation)> = match child {
                Child::Edge(e) => {
                    let leaf = Tree::Leaf { token: lattice.edges[e].token, morph: lattice.edges[e].morph.clone(), role: None };
                    vec![(leaf, ItemChild::Edge(e), Derivation::default())]
                }
                Child::Item(c) => {
                    self.item_trees(c, lattice, limit).into_iter().map(|(t, d)| (t, ItemChild::Item(c), d)).collect()
                }
            };
            for (mut last, picked, below) in lasts {
                last.set_role(mark, &self.grammar.case_roles);
                for (mut prefix, mut prefix_picked, mut derivation) in self.children_lists(prev, lattice, limit) {
                    if out.len() >= limit {
                        return out;
                    }
                    prefix.push(last.clone());
                    prefix_picked.push(picked);
                    derivation.items.extend(below.items.iter().map(|(k, v)| (*k, v.clone())));
                    out.push((prefix, prefix_picked, derivation));
                }
            }
        }
        out
    }
}

/// The chart items one parse is built from ([`Forest::parses`]): each
/// complete item, by id ([`ChartItem::id`]), with the one child it has at each
/// right-hand-side position in this parse.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Derivation {
    pub items: BTreeMap<usize, DerivedItem>,
}

/// A complete chart item as one parse uses it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DerivedItem {
    /// Index into the grammar's rules.
    pub rule: usize,
    /// Per right-hand-side position, the child this parse has there.
    pub children: Vec<ItemChild>,
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
            Value::NotAny(refused) => {
                if refused.iter().any(|r| r == actual) {
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
            Value::Any(_) | Value::NotVar(_) | Value::NotAny(_) => None,
            Value::Var(var) => env.get(var).cloned(),
        };
        if let Some(v) = resolved {
            feats.insert(key.clone(), v);
        }
    }
    feats
}


impl Tree {
    fn set_role(&mut self, mark: Option<RoleMark>, case_roles: &BTreeMap<String, Role>) {
        let (slot, feats) = match self {
            Tree::Node { role, feats, .. } => (role, &*feats),
            Tree::Leaf { role, morph, .. } => (role, &morph.feats),
        };
        *slot = match mark {
            None => None,
            Some(RoleMark::Role(r)) => Some(r),
            Some(RoleMark::ByCase) => feats.get("case").and_then(|c| case_roles.get(c)).copied(),
        };
    }

    pub fn role(&self) -> Option<Role> {
        match self {
            Tree::Node { role, .. } | Tree::Leaf { role, .. } => *role,
        }
    }

    /// Each token's role in this parse: the innermost phrase with a role that
    /// holds all of the token's morphemes (`dammelo` is the verb group, though
    /// its `me` and `lo` have roles of their own), or the role every one of its
    /// morphemes has. `None` outside any role.
    pub fn token_roles(&self, tokens: usize) -> Vec<Option<Role>> {
        let mut total = vec![0usize; tokens];
        for (t, _) in self.leaves() {
            total[t] += 1;
        }
        let mut out = vec![None; tokens];
        self.assign_roles(&total, &mut out);
        // A token split across sibling phrases of the same role (`dalla` =
        // da + la in `Prep:agent NP:agent`) has that role, though no one
        // phrase holds it.
        let mut morph_roles: Vec<Vec<Option<Role>>> = vec![Vec::new(); tokens];
        self.morpheme_roles(None, &mut morph_roles);
        for (t, roles) in morph_roles.iter().enumerate() {
            if let [Some(first), rest @ ..] = roles.as_slice()
                && rest.iter().all(|r| r == &Some(*first))
            {
                out[t] = Some(*first);
            }
        }
        out
    }

    /// For each token, the role of the innermost phrase with a role holding
    /// each of its morphemes.
    fn morpheme_roles(&self, inherited: Option<Role>, out: &mut [Vec<Option<Role>>]) {
        let role = self.role().or(inherited);
        match self {
            Tree::Leaf { token, .. } => out[*token].push(role),
            Tree::Node { children, .. } => {
                for c in children {
                    c.morpheme_roles(role, out);
                }
            }
        }
    }

    /// Post-order, so a phrase deeper in the tree claims a token first.
    /// Returns how many of each token's morphemes this subtree holds.
    fn assign_roles(&self, total: &[usize], out: &mut [Option<Role>]) -> Vec<usize> {
        let mut held = vec![0usize; total.len()];
        match self {
            Tree::Leaf { token, .. } => held[*token] += 1,
            Tree::Node { children, .. } => {
                for c in children {
                    for (h, n) in held.iter_mut().zip(c.assign_roles(total, out)) {
                        *h += n;
                    }
                }
            }
        }
        if let Some(role) = self.role() {
            for t in 0..total.len() {
                if held[t] > 0 && held[t] == total[t] && out[t].is_none() {
                    out[t] = Some(role);
                }
            }
        }
        held
    }

    /// The morphemes the parse chose, in sentence order.
    pub fn leaves(&self) -> Vec<(usize, &Morpheme)> {
        match self {
            Tree::Leaf { token, morph, .. } => vec![(*token, morph)],
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
            Tree::Leaf { morph, role, .. } => {
                out.push_str(&format!(
                    "{pad}{} {} ({} {}){}\n",
                    morph.cat,
                    morph.surface,
                    morph.lemma,
                    crate::syntax::feats::display(&morph.feats),
                    role_suffix(*role)
                ));
            }
            Tree::Node { cat, feats, rule, role, children, .. } => {
                out.push_str(&format!(
                    "{pad}{cat}[{}]{}  ← {rule}\n",
                    crate::syntax::feats::display(feats),
                    role_suffix(*role)
                ));
                for c in children {
                    c.render_into(out, depth + 1);
                }
            }
        }
    }
}

fn role_suffix(role: Option<Role>) -> String {
    role.map(|r| format!(":{r}")).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn morph(cat: &str, feats: &[(&str, &str)]) -> Morpheme {
        Morpheme {
            surface: cat.to_lowercase(),
            lemma: cat.to_lowercase(),
            cat: cat.to_owned(),
            feats: feats.iter().map(|(k, v)| ((*k).to_owned(), (*v).to_owned())).collect(),
        }
    }

    /// A chart scan lists each complete item once with its children side by
    /// side. Like a French object-agreement question (S -> WhP QMark NP
    /// Verb(avoir) Verb(pastpart)), a long ambiguous phrase before the
    /// auxiliary: 16 words with two readings each are 65,536 parses and
    /// still one item.
    #[test]
    fn chart_items_do_not_multiply_with_the_readings() {
        let grammar = Grammar::parse(
            "start S\n\
             rule s: S[aux_tense=?t] -> NP Verb[lemma=avere, tense=?t] Verb[form=pastpart]\n  \"x\"\n\
             rule np.more: NP -> NP X\n  \"x\"\n\
             rule np.one: NP -> X\n  \"x\"\n",
        )
        .unwrap();
        let mut lattice = Lattice::new();
        for _ in 0..16 {
            lattice.push_token("x", vec![vec![morph("X", &[("a", "1")])], vec![morph("X", &[("a", "2")])]]);
        }
        lattice.push_token("ho", vec![vec![morph("Verb", &[("form", "fin"), ("tense", "pres")])]]);
        lattice.edges.last_mut().unwrap().morph.lemma = "avere".to_owned();
        lattice.push_token("fatto", vec![vec![morph("Verb", &[("form", "pastpart")])]]);
        let forest = parse(&grammar, &lattice).unwrap();
        assert_eq!(forest.count(), 65_536);
        let items = forest.items_where(|r| r.name == "s");
        let [s] = items.as_slice() else { panic!("{items:?}") };
        assert_eq!(s.feats.get("aux_tense").map(String::as_str), Some("pres"));
        assert_eq!(s.children[1], [ItemChild::Edge(32)]);
        assert_eq!(s.children[2], [ItemChild::Edge(33)]);
        assert_eq!(forest.items_where(|r| r.name == "np.more").len(), 15);
    }
}
