//! The verbs of a parse: each simple verb, compound tense, passive and
//! progressive as one group of tokens with its lemma, cell and voice
//! ([`VerbGroup`], [`crate::SentenceAnalysis::verb_groups`]).
//!
//! The grammar names a compound tense or a passive while it parses: the rules
//! that join an auxiliary and a past participle pass the auxiliary's features
//! up (`aux_mood`, `aux_tense`, `aux_person`, `aux_number`, `aux_form`), a
//! passive rule marks `voice=passive`, a progressive one `aux_progressive=yes`,
//! and an auxiliary between the governing one and the participle that is not
//! a participle itself (Spanish *siendo*) is `aux_layer`. The groups are read
//! from those chart items ([`crate::syntax::parser::Forest::items_where`]); the
//! auxiliary's tense names the compound tense
//! ([`crate::examples::tenses::compound_of`]).

use std::collections::{HashMap, HashSet};

use serde::Serialize;

use crate::examples::table::cell_of;
use crate::examples::tenses::{auxiliaries, compound_of, tense_of};
use crate::morph::Lang;
use crate::syntax::feats::FeatMap;
use crate::syntax::grammar::{Cat, Grammar, Value};
use crate::syntax::lattice::Lattice;
use crate::syntax::parser::{ChartItem, Derivation, ItemChild};
use crate::verbs::verb_use;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Voice {
    Active,
    /// The participle with essere/venire, ser, être: fu chiesto, è stato
    /// venduto, fue construida, a été vendu.
    Passive,
}

/// One verb of a parse: a simple verb (one token), a compound tense or a
/// passive (auxiliaries + past participle), or a progressive (auxiliaries +
/// gerund).
///
/// | Sentence | `auxiliaries` | `participle` | `verb` | `lemma` | `cell` | `voice` | `progressive` |
/// |---|---|---|---|---|---|---|---|
/// | *La ragazza **ama** il cane* | | | 2 | amare | `indicative.present.3s` | active | no |
/// | ***Ho dato** il libro a Maria* | 0 | 1 | 1 | dare | `indicative.present_perfect.1s` | active | no |
/// | *Il libro **è stato letto*** | 2, 3 | 4 | 4 | leggere | `indicative.present_perfect.3s` | passive | no |
/// | *Il libro **fu letto*** | 2 | 3 | 3 | leggere | `indicative.past_historic.3s` | passive | no |
/// | *El programa **está siendo grabado*** | 2, 3 | 4 | 4 | grabar | `indicative.present.3s` | passive | yes |
/// | *I prezzi **stanno crescendo*** | 2 | | 3 | crescere | `indicative.present.3p` | active | yes |
/// | ***Sigue estando lloviendo*** | 0, 1 | | 2 | llover | `indicative.present.3s` | active | yes |
/// | *No **está siendo** difícil* | 1 | | 2 | ser | `indicative.present.3s` | active | yes |
/// | ***Ha estado lloviendo*** | 0, 1 | | 2 | llover | `indicative.present_perfect.3s` | active | yes |
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[non_exhaustive]
pub struct VerbGroup {
    /// Every token of the group, in sentence order (indices into
    /// [`crate::SentenceAnalysis::tokens`]). A word between them is not part
    /// of it: *ho già mangiato* is tokens 0 and 2.
    pub tokens: Vec<usize>,
    /// The auxiliaries, in sentence order: the one that carries the tense
    /// (*ho*, *è*, *ha*, *sto*) and any between it and the main verb (*stato*
    /// in *è stato letto*, *estado* in *ha estado lloviendo*). Empty for a
    /// simple verb.
    pub auxiliaries: Vec<usize>,
    /// The past participle of a compound tense or a passive.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub participle: Option<usize>,
    /// The main verb's token: the simple verb, the participle, or a
    /// progressive's gerund.
    pub verb: usize,
    /// The main verb's lemma (*dare* for *ho dato*, never the auxiliary's).
    pub lemma: String,
    /// The cell, named as `syntango tenses` names tenses
    /// (`indicative.present_perfect.1s`, `non_finite.infinitive`); `None`
    /// when no cell is known (an auxiliary in the imperative). A compound
    /// tense is named after its auxiliary's tense; a passive's cell is the
    /// tense of the passive (*fu letto* `indicative.past_historic.3s`, *è
    /// stato letto* `indicative.present_perfect.3s`); a progressive's is its
    /// auxiliaries' (*stanno crescendo* `indicative.present.3p`, *ha estado
    /// lloviendo* `indicative.present_perfect.3s`), with `progressive` set.
    pub cell: Option<String>,
    pub voice: Voice,
    /// *stare*/*estar* (or a Spanish *seguir*, *ir*, …) + gerund. The cell is
    /// the auxiliaries' tense; the gerund is the `verb`. Also set on a
    /// progressive passive (Spanish *está siendo grabado*): there the
    /// auxiliaries are the progressive (*está siendo*), the participle is the
    /// `verb`, and the cell is the auxiliary's tense (`indicative.present.3s`).
    pub progressive: bool,
}

/// A compound tense, a passive or a progressive in the chart: an auxiliary
/// (+ stacked auxiliaries) + participle(s), from one chart item.
#[derive(Debug, Clone)]
pub(crate) struct Construction {
    /// The rules that consumed its words directly.
    pub rules: Vec<String>,
    /// The auxiliary's lattice edge, the one that carries the tense (ho, è,
    /// fue, a).
    pub aux: usize,
    /// Auxiliaries in participle form before the main participle (stato,
    /// sido, été).
    pub layers: Vec<usize>,
    pub participles: Vec<usize>,
    pub voice: Voice,
    /// The cell the group is in, when the auxiliary's cell names one.
    pub cell: Option<String>,
    pub aux_cell: Option<String>,
    /// A progressive (sto mangiando, estoy trabajando, he estado
    /// trabajando): its auxiliaries are kept from being examples of their own
    /// verbs; the gerund stays an example of its verb. No participles.
    pub progressive: bool,
    /// The lattice nodes the chart item spans.
    pub start: usize,
    pub end: usize,
}

/// A verb item's words in rule order, through the phrases that pass the
/// auxiliary up (Spanish `SerV`, `PassP`).
#[derive(Debug, Clone)]
enum Slot {
    Aux(Vec<usize>, String),
    /// An auxiliary between the governing one and the participle that is not
    /// a participle itself (siendo in está siendo construida): `aux_layer`.
    Layer(Vec<usize>, String),
    Participle(Vec<usize>, String),
    Coordinator,
}

/// The compound tenses, passives and progressives that `items` (verb chart
/// items, [`crate::SentenceAnalysis::verb_items`]) build, one per item.
///
/// With `skip_passed_up` (the example extractor), an item whose only use is
/// to pass its auxiliary up to another (Spanish ha sido in ha sido
/// construido) is left out as part of that one. Without it (one parse's
/// [`VerbGroup`]s), every item gives its construction and [`chains`] joins
/// them, so a wrapper that forwards no construction of its own (`serv.negation`
/// in no está siendo difícil) loses nothing.
pub(crate) fn constructions(
    grammar: &Grammar,
    lang: Lang,
    lattice: &Lattice,
    items: &[ChartItem],
    skip_passed_up: bool,
) -> Vec<Construction> {
    let by_id: HashMap<usize, &ChartItem> = items.iter().map(|i| (i.id, i)).collect();
    let mut out = Vec::new();
    for item in items {
        let only_passed_up = !item.used_as.is_empty()
            && item
                .used_as
                .iter()
                .all(|&(rule, at)| passes_up(&grammar.rules[rule].rhs[at]));
        if skip_passed_up && only_passed_up {
            continue;
        }
        let progressive = item.feats.get("aux_progressive").map(String::as_str) == Some("yes");
        for slots in slots(grammar, &by_id, item) {
            if progressive {
                out.extend(progressive_group(lang, item, &slots));
            } else {
                out.extend(verb_group(lang, lattice, item, &slots));
            }
        }
    }
    out
}

/// The verb groups of one parse: its constructions, then every other verb
/// the parse uses as a verb, one token each. Sorted by the main verb's token.
pub(crate) fn parse_groups(
    grammar: &Grammar,
    lang: Lang,
    lattice: &Lattice,
    verb_items: &[ChartItem],
    derivation: &Derivation,
) -> Vec<VerbGroup> {
    let mut parents: HashMap<usize, Vec<(usize, usize)>> = HashMap::new();
    let mut consumer: HashMap<usize, usize> = HashMap::new();
    for d in derivation.items.values() {
        for (at, child) in d.children.iter().enumerate() {
            match child {
                ItemChild::Item(c) => parents.entry(*c).or_default().push((d.rule, at)),
                ItemChild::Edge(e) => {
                    consumer.insert(*e, d.rule);
                }
            }
        }
    }
    // This parse's verb items, each with the one child it has here.
    let items: Vec<ChartItem> = verb_items
        .iter()
        .filter_map(|item| {
            let d = derivation.items.get(&item.id)?;
            Some(ChartItem {
                children: d.children.iter().map(|c| vec![*c]).collect(),
                used_as: parents.get(&item.id).cloned().unwrap_or_default(),
                ..item.clone()
            })
        })
        .collect();
    let token = |e: usize| lattice.edges[e].token;
    let constructions = constructions(grammar, lang, lattice, &items, false);
    let mut claimed: HashSet<usize> = HashSet::new();
    let mut out = Vec::new();
    for c in chains(lattice, &consumer, &constructions) {
        claimed.extend(c.edges());
        let mut auxiliaries: Vec<usize> = std::iter::once(c.aux)
            .chain(c.layers.iter().copied())
            .map(token)
            .collect();
        auxiliaries.sort();
        for &e in &c.mains {
            let mut tokens = auxiliaries.clone();
            tokens.push(token(e));
            tokens.sort();
            out.push(VerbGroup {
                tokens,
                auxiliaries: auxiliaries.clone(),
                participle: c.participle.then(|| token(e)),
                verb: token(e),
                lemma: lattice.edges[e].morph.lemma.clone(),
                cell: c.cell.clone(),
                voice: c.voice,
                progressive: c.progressive,
            });
        }
    }
    for (&e, &rule) in &consumer {
        let m = &lattice.edges[e].morph;
        if claimed.contains(&e)
            || m.cat != "Verb"
            || !verb_use(m, &[grammar.rules[rule].name.clone()]).as_verb
        {
            continue;
        }
        out.push(VerbGroup {
            tokens: vec![token(e)],
            auxiliaries: Vec::new(),
            participle: None,
            verb: token(e),
            lemma: m.lemma.clone(),
            cell: cell_of(&m.feats, lang),
            voice: Voice::Active,
            progressive: false,
        });
    }
    out.sort_by(|a, b| (a.verb, &a.tokens).cmp(&(b.verb, &b.tokens)));
    out.dedup();
    out
}

/// One verb chain of a parse, as lattice edges: the auxiliary that carries
/// the tense, the auxiliaries after it, and the main verb(s) (two for
/// participles coordinated after one auxiliary).
#[derive(Debug, Clone)]
struct Chain {
    aux: usize,
    layers: Vec<usize>,
    mains: Vec<usize>,
    /// The mains are past participles (a compound tense or a passive), not a
    /// progressive's gerund.
    participle: bool,
    cell: Option<String>,
    voice: Voice,
    progressive: bool,
}

impl Chain {
    fn edges(&self) -> HashSet<usize> {
        std::iter::once(self.aux)
            .chain(self.layers.iter().copied())
            .chain(self.mains.iter().copied())
            .collect()
    }
}

/// One parse's constructions joined into chains, so each verb chain is one
/// chain however the grammar nests it:
///
/// - a construction inside another (the `SerV` of está siendo in está siendo
///   grabado, ha sido in ha sido construido, wrapped by `serv.negation` or
///   not) is part of it; an inner progressive on the same auxiliary makes
///   the outer one progressive;
/// - a construction whose main verb is another's auxiliary is stacked on it
///   (sigue estando + estando lloviendo = sigue estando lloviendo): the outer
///   auxiliary carries the tense, the inner main verb is the main verb.
fn chains(
    lattice: &Lattice,
    consumer: &HashMap<usize, usize>,
    constructions: &[Construction],
) -> Vec<Chain> {
    let mut chains: Vec<Chain> = constructions
        .iter()
        .filter_map(|c| {
            let mains = if c.progressive {
                gerund(lattice, consumer, c).into_iter().collect()
            } else {
                c.participles.clone()
            };
            (!mains.is_empty()).then(|| Chain {
                aux: c.aux,
                layers: c.layers.clone(),
                mains,
                participle: !c.progressive,
                cell: c.cell.clone(),
                voice: c.voice,
                progressive: c.progressive,
            })
        })
        .collect();
    // Inner constructions into the ones that hold them.
    let mut i = 0;
    while i < chains.len() {
        let inner = chains[i].edges();
        let holder = (0..chains.len()).find(|&j| {
            let outer = chains[j].edges();
            j != i && inner.is_subset(&outer) && (inner != outer || j < i)
        });
        match holder {
            Some(j) => {
                let removed = chains.remove(i);
                let j = if j > i { j - 1 } else { j };
                if removed.aux == chains[j].aux {
                    chains[j].progressive |= removed.progressive;
                }
            }
            None => i += 1,
        }
    }
    // Stacked chains into one.
    while let Some((a, b)) = (0..chains.len())
        .flat_map(|a| (0..chains.len()).map(move |b| (a, b)))
        .find(|&(a, b)| a != b && chains[a].mains == [chains[b].aux])
    {
        let inner = chains[b].clone();
        let outer = &mut chains[a];
        outer.layers.push(inner.aux);
        outer.layers.extend(inner.layers);
        outer.mains = inner.mains;
        outer.participle = inner.participle;
        outer.progressive |= inner.progressive;
        if inner.voice == Voice::Passive {
            outer.voice = Voice::Passive;
        }
        chains.remove(b);
    }
    chains
}

/// A progressive's gerund in this parse: the first gerund it spans after
/// its auxiliaries. When that gerund is itself a progressive's auxiliary
/// (estando in sigue estando lloviendo), [`chains`] stacks the two.
fn gerund(lattice: &Lattice, consumer: &HashMap<usize, usize>, c: &Construction) -> Option<usize> {
    consumer
        .keys()
        .copied()
        .filter(|&e| {
            let edge = &lattice.edges[e];
            edge.from >= c.start
                && edge.to <= c.end
                && e != c.aux
                && !c.layers.contains(&e)
                && edge.morph.cat == "Verb"
                && edge.morph.feats.get("form").map(String::as_str) == Some("ger")
        })
        .min_by_key(|&e| lattice.edges[e].from)
}

/// An item's words in rule order, one list per combination of the phrases
/// it passes the auxiliary up through (usually one).
fn slots(
    grammar: &Grammar,
    items: &HashMap<usize, &ChartItem>,
    item: &ChartItem,
) -> Vec<Vec<Slot>> {
    let rule = &grammar.rules[item.rule];
    let vars_of = |layer: bool| -> HashSet<&str> {
        rule.lhs
            .constraints
            .iter()
            .filter(|(k, _)| k.starts_with("aux_") && (k == "aux_layer") == layer)
            .filter_map(|(_, v)| match v {
                Value::Var(v) => Some(v.as_str()),
                _ => None,
            })
            .collect()
    };
    let (aux_vars, layer_vars) = (vars_of(false), vars_of(true));
    let feeds = |cat: &Cat, vars: &HashSet<&str>| {
        cat.constraints
            .iter()
            .any(|(_, v)| matches!(v, Value::Var(v) if vars.contains(v.as_str())))
    };
    let mut out: Vec<Vec<Slot>> = vec![Vec::new()];
    for (at, cat) in rule.rhs.iter().enumerate() {
        let edges: Vec<usize> = item.children[at]
            .iter()
            .filter_map(|p| match p {
                ItemChild::Edge(e) => Some(*e),
                ItemChild::Item(_) => None,
            })
            .collect();
        let slot = if cat.name == "Verb" && is_participle_slot(cat) {
            Some(Slot::Participle(edges, rule.name.clone()))
        } else if cat.name == "Verb" && feeds(cat, &layer_vars) {
            Some(Slot::Layer(edges, rule.name.clone()))
        } else if cat.name == "Verb" && feeds(cat, &aux_vars) {
            Some(Slot::Aux(edges, rule.name.clone()))
        } else if cat.name == "Conj" {
            Some(Slot::Coordinator)
        } else {
            None
        };
        if let Some(slot) = slot {
            for list in &mut out {
                list.push(slot.clone());
            }
            continue;
        }
        if !passes_up(cat) {
            continue;
        }
        let inner: Vec<Vec<Slot>> = item.children[at]
            .iter()
            .filter_map(|p| match p {
                ItemChild::Item(c) => items.get(c),
                ItemChild::Edge(_) => None,
            })
            .flat_map(|child| slots(grammar, items, child))
            .collect();
        out = out
            .iter()
            .flat_map(|l| {
                inner
                    .iter()
                    .map(move |i| l.iter().chain(i).cloned().collect())
            })
            .collect();
    }
    out
}

/// A verb item's slots as auxiliary + participle(s), or `None` when it has
/// no participle of its own (ha sido alone, before its participle).
fn verb_group(
    lang: Lang,
    lattice: &Lattice,
    item: &ChartItem,
    slots: &[Slot],
) -> Option<Construction> {
    // The governing auxiliary is the last before the participles.
    let last_participle = slots
        .iter()
        .rposition(|s| matches!(s, Slot::Participle(..)))?;
    let aux_at = slots[..last_participle]
        .iter()
        .rposition(|s| matches!(s, Slot::Aux(..)))?;
    let Slot::Aux(aux_edges, aux_rule) = &slots[aux_at] else {
        return None;
    };
    let aux = *aux_edges.first()?;
    let mut rules = vec![aux_rule.clone()];
    let (mut layers, mut participles) = (Vec::new(), Vec::new());
    // A participle that another follows without a coordinator between is a
    // stacked auxiliary (stato in è stato venduto); across one, a verb of its
    // own (sono venuto e partito).
    let rest = &slots[aux_at + 1..];
    let mut perfect = false;
    for (i, slot) in rest.iter().enumerate() {
        let (edges, rule) = match slot {
            Slot::Participle(edges, rule) => (edges, rule),
            Slot::Layer(edges, rule) => {
                rules.push(rule.clone());
                layers.extend(edges);
                continue;
            }
            _ => continue,
        };
        rules.push(rule.clone());
        let stacked = rest[i + 1..]
            .iter()
            .find(|s| !matches!(s, Slot::Aux(..) | Slot::Layer(..)))
            .is_some_and(|s| matches!(s, Slot::Participle(..)));
        let is_aux =
            |e: &usize| auxiliaries(lang).contains(&lattice.edges[*e].morph.lemma.as_str());
        if stacked && edges.iter().all(is_aux) {
            layers.extend(edges);
            perfect = true;
        } else {
            participles.extend(edges);
        }
    }
    if participles.is_empty() {
        return None;
    }
    let voice = voice_of(item);
    // A participle layer (stato, sido, été) makes a passive compound; a
    // gerund one (siendo) does not.
    let compound = voice == Voice::Active || perfect;
    let aux_cell = auxiliary_cell(item, lang);
    let cell = aux_cell.as_deref().and_then(|c| {
        if compound {
            compound_cell(lang, c)
        } else {
            Some(c.to_owned())
        }
    });
    rules.sort();
    rules.dedup();
    Some(Construction {
        rules,
        aux,
        layers,
        participles,
        voice,
        cell,
        aux_cell,
        progressive: false,
        start: item.start,
        end: item.end,
    })
}

/// A progressive's auxiliaries: the verb that carries the tense and any
/// participle before the gerund (estado in he estado trabajando). An
/// auxiliary layer (siendo) is the gerund itself, a verb of its own. Its cell
/// is the auxiliaries' tense: the auxiliary's, or the compound tense it makes
/// with a participle (he estado).
fn progressive_group(lang: Lang, item: &ChartItem, slots: &[Slot]) -> Option<Construction> {
    let mut edges = Vec::new();
    let mut rules = Vec::new();
    for slot in slots {
        if let Slot::Aux(e, rule) | Slot::Participle(e, rule) = slot {
            edges.extend(e);
            rules.push(rule.clone());
        }
    }
    let (&aux, layers) = edges.split_first()?;
    rules.sort();
    rules.dedup();
    let aux_cell = auxiliary_cell(item, lang);
    let cell = aux_cell.as_deref().and_then(|c| {
        if layers.is_empty() {
            Some(c.to_owned())
        } else {
            compound_cell(lang, c)
        }
    });
    Some(Construction {
        rules,
        aux,
        layers: layers.to_vec(),
        participles: Vec::new(),
        voice: voice_of(item),
        cell,
        aux_cell,
        progressive: true,
        start: item.start,
        end: item.end,
    })
}

fn voice_of(item: &ChartItem) -> Voice {
    if item.feats.get("voice").map(String::as_str) == Some("passive") {
        Voice::Passive
    } else {
        Voice::Active
    }
}

/// The cell of the auxiliary that carries the tense, from the item's `aux_*`
/// features.
fn auxiliary_cell(item: &ChartItem, lang: Lang) -> Option<String> {
    let aux_feats: FeatMap = item
        .feats
        .iter()
        .filter(|(k, _)| k.as_str() != "aux_layer")
        .filter_map(|(k, v)| k.strip_prefix("aux_").map(|k| (k.to_owned(), v.clone())))
        .collect();
    cell_of(&aux_feats, lang)
}

/// The compound tense an auxiliary in `aux_cell` makes, in the same person
/// and number: `indicative.present.1s` → `indicative.present_perfect.1s`.
fn compound_cell(lang: Lang, aux_cell: &str) -> Option<String> {
    let tense = tense_of(aux_cell);
    compound_of(lang, tense).map(|t| format!("{}{}", t.id, &aux_cell[tense.len()..]))
}

/// A category the rule passes an auxiliary's features or a voice through
/// (Spanish `SerV[aux_tense=?t]`, `PassP[voice=passive]`).
fn passes_up(cat: &Cat) -> bool {
    cat.constraints
        .iter()
        .any(|(k, _)| k.starts_with("aux_") || k == "voice")
}

/// A `Verb[form=pastpart]` position.
fn is_participle_slot(cat: &Cat) -> bool {
    cat.constraints.iter().any(|(k, v)| {
        k == "form" && matches!(v, Value::Any(a) if a.iter().any(|x| x == "pastpart"))
    })
}
