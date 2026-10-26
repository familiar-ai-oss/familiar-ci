//! One view over a log of facts, with the field accessors every rule uses and the commit-content index.
//! A log is never mutated, so a view caches derived values for its lifetime and never stores them as facts.

use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};

use familiar_ai_facts::Fact;
use serde_json::Value;

use crate::criterion::{Hold, SealKey};

/// A position later than every fact.
pub(crate) const INF: i64 = i64::MAX;

pub fn text<'a>(f: &'a Fact, key: &str) -> Option<&'a str> {
    f.fields.get(key).and_then(Value::as_str)
}

/// Whether the field is JSON `true`.
pub fn flag(f: &Fact, key: &str) -> bool {
    f.fields.get(key).and_then(Value::as_bool).unwrap_or(false)
}

pub(crate) fn int(f: &Fact, key: &str) -> Option<i64> {
    f.fields.get(key).and_then(Value::as_i64)
}

/// A list-of-text field; an absent or non-list field reads as empty.
pub fn texts<'a>(f: &'a Fact, key: &str) -> Vec<&'a str> {
    f.fields
        .get(key)
        .and_then(Value::as_array)
        .map(|xs| xs.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default()
}

/// Whether the fact is about criterion `c`.
pub(crate) fn is_of(f: &Fact, c: &str) -> bool {
    f.criterion_id() == Some(c)
}

/// The fact's position as a signed number, so it compares with `-1` and `INF`.
pub fn t_of(f: &Fact) -> i64 {
    i64::try_from(f.t).unwrap_or(INF)
}

pub(crate) fn last_of<T>(xs: Vec<T>) -> Option<T> {
    xs.into_iter().last()
}

/// The tests' part of a commit's content: sorted `[criterion_id, tests_hash]` pairs from the
/// latest `tests_observed` of each criterion at that commit id. A legacy commit with no
/// observation carries its one hash as `["", hash]`.
fn pairs_of(log: &[Fact], obs: &HashMap<String, BTreeMap<String, usize>>, f: &Fact) -> Value {
    let id = text(f, "commit").unwrap_or("");
    if let Some(m) = obs.get(id).filter(|m| !m.is_empty()) {
        return Value::Array(
            m.iter()
                .map(|(c, &i)| Value::Array(vec![Value::from(c.as_str()), log[i].fields.get("tests_hash").cloned().unwrap_or(Value::Null)]))
                .collect(),
        );
    }
    match f.fields.get("tests_hash") {
        None | Some(Value::Null) => Value::Array(Vec::new()),
        Some(h) => Value::Array(vec![Value::Array(vec![Value::from(""), h.clone()])]),
    }
}

/// A commit's content key from its fact and its fingerprints.
fn content_key_with(f: &Fact, pairs: Value) -> String {
    let mut files: Vec<&str> = texts(f, "files");
    files.sort_unstable();
    let mut overlap: Vec<&str> = texts(f, "anchor_overlap");
    overlap.sort_unstable();
    Value::Array(vec![
        Value::Array(files.into_iter().map(Value::from).collect()),
        Value::Bool(flag(f, "touches_tests")),
        Value::Bool(flag(f, "touches_code")),
        Value::Array(overlap.into_iter().map(Value::from).collect()),
        pairs,
    ])
    .to_string()
}

/// A commit's content key: its files, flags, anchor overlap and fingerprints, never its id.
/// A rebase changes ids, not content, so it moves nothing.
pub fn content_key(v: &View, f: &Fact) -> String {
    let ix = v.idx();
    content_key_with(f, pairs_of(v.facts, &ix.obs, f))
}

/// The commit index: each id's content key and last landing, each content's first landing and
/// ids, and the latest `tests_observed` per commit and per criterion.
#[derive(Default)]
pub struct CommitIndex {
    pub key_of_id: HashMap<String, String>,
    pub first: HashMap<String, i64>,
    pub rep: HashMap<String, usize>,
    pub at_of_id: HashMap<String, i64>,
    pub ids: HashMap<String, Vec<String>>,
    /// Every content key in the order it first landed.
    pub keys: Vec<String>,
    /// commit id -> criterion -> the position of its latest `tests_observed`.
    pub obs: HashMap<String, BTreeMap<String, usize>>,
    /// criterion -> the position of its latest `tests_observed`.
    pub last_obs: HashMap<String, usize>,
}

impl CommitIndex {
    fn build(log: &[Fact]) -> CommitIndex {
        let mut ix = CommitIndex::default();
        // Collect observations first, since an observation follows its commit_seen; the latest
        // per criterion and commit counts.
        let mut cs: Vec<usize> = Vec::new();
        for (i, f) in log.iter().enumerate() {
            if f.kind == "commit_seen" {
                cs.push(i);
            } else if f.kind == "tests_observed" {
                let (Some(id), Some(c)) = (text(f, "commit"), f.criterion_id()) else {
                    continue;
                };
                ix.obs.entry(id.to_string()).or_default().insert(c.to_string(), i);
                ix.last_obs.insert(c.to_string(), i);
            }
        }
        for i in cs {
            let f = &log[i];
            let Some(id) = text(f, "commit") else {
                continue;
            };
            let k = content_key_with(f, pairs_of(log, &ix.obs, f));
            ix.key_of_id.insert(id.to_string(), k.clone());
            ix.at_of_id.insert(id.to_string(), t_of(f));
            if !ix.first.contains_key(&k) {
                ix.first.insert(k.clone(), t_of(f));
                ix.rep.insert(k.clone(), i);
                ix.keys.push(k.clone());
            }
            ix.ids.entry(k).or_default().push(id.to_string());
        }
        ix
    }

    /// Where a content first landed, `INF` when never.
    pub fn first_at(&self, key: Option<&str>) -> i64 {
        key.and_then(|k| self.first.get(k).copied()).unwrap_or(INF)
    }

    /// The content key of a commit id.
    pub fn key_of(&self, id: &str) -> Option<&str> {
        self.key_of_id.get(id).map(String::as_str)
    }
}

/// The values a view caches, keyed by criterion.
#[derive(Default)]
pub(crate) struct Memo {
    pub confirmed: Option<bool>,
    pub runnable: Option<Vec<String>>,
    pub state: HashMap<String, &'static str>,
    pub holds: HashMap<String, std::rc::Rc<Vec<Hold>>>,
    pub untouched: HashMap<String, bool>,
    pub test_change: HashMap<String, Option<&'static str>>,
    /// The seal and the tests-first commit are read many times per hold; the raw seal and the
    /// gated seal are cached separately.
    pub seal_raw: HashMap<SealKey, Option<usize>>,
    pub sealed_at: HashMap<SealKey, Option<usize>>,
    pub tests_first: HashMap<String, Option<crate::criterion::TestsFirst>>,
    /// The way back.
    pub mark: HashMap<String, Option<crate::criterion::TestsFirst>>,
    /// The positions of the open investigator transfers.
    pub chain: HashMap<String, Vec<usize>>,
    pub irung: HashMap<String, Option<&'static str>>,
    /// The positions of the whole climb.
    pub climb: HashMap<String, Vec<usize>>,
    pub tries: HashMap<String, usize>,
    /// Whether each part was touched in this checkout since the seed's mark.
    pub touched: HashMap<String, bool>,
}

/// The hold each transfer was sent for, by position, shared by a view and its prefixes.
/// Sharing is sound because the answer is judged on the log before the transfer.
pub(crate) type JudgedBefore = std::rc::Rc<RefCell<HashMap<usize, Option<&'static str>>>>;

/// A view over one log: the facts, the commit index, and the memo.
pub struct View<'a> {
    pub facts: &'a [Fact],
    index: RefCell<Option<std::rc::Rc<CommitIndex>>>,
    pub(crate) memo: RefCell<Memo>,
    pub(crate) investigates: JudgedBefore,
}

impl<'a> View<'a> {
    pub fn new(facts: &'a [Fact]) -> View<'a> {
        View {
            facts,
            index: RefCell::new(None),
            memo: RefCell::new(Memo::default()),
            investigates: JudgedBefore::default(),
        }
    }

    /// A view over the first `end` facts, with its own index and memo but the shared transfer cache.
    pub fn prefix(&self, end: usize) -> View<'a> {
        let mut v = View::new(&self.facts[..end.min(self.facts.len())]);
        v.investigates = self.investigates.clone();
        v
    }

    /// The commit index, built once per view.
    pub fn idx(&self) -> std::rc::Rc<CommitIndex> {
        let mut slot = self.index.borrow_mut();
        if let Some(ix) = slot.as_ref() {
            return ix.clone();
        }
        let ix = std::rc::Rc::new(CommitIndex::build(self.facts));
        *slot = Some(ix.clone());
        ix
    }

    pub fn len(&self) -> usize {
        self.facts.len()
    }

    pub fn is_empty(&self) -> bool {
        self.facts.is_empty()
    }

    /// The latest fact of a kind.
    pub fn last(&self, kind: &str) -> Option<&'a Fact> {
        self.facts.iter().rev().find(|f| f.kind == kind)
    }

    /// Every fact of a kind for one criterion, in log order.
    pub fn of(&self, kind: &str, c: &str) -> impl Iterator<Item = &'a Fact> + 'a {
        let c = c.to_string();
        let kind = kind.to_string();
        self.facts
            .iter()
            .filter(move |f| f.kind == kind && is_of(f, &c))
    }

    /// Every fact of a kind, in log order.
    pub fn kind(&self, kind: &str) -> impl Iterator<Item = &'a Fact> + 'a {
        let kind = kind.to_string();
        self.facts.iter().filter(move |f| f.kind == kind)
    }

    /// The fact at a position of this view.
    pub fn at(&self, i: usize) -> Option<&'a Fact> {
        self.facts.get(i)
    }
}
