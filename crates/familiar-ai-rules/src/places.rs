//! Places: where else the behavior lives, listed before the fix (the owner's plan of 2026-10-21,
//! step 3) [draft names]. Once the plan is confirmed and a part's place checked, the binary lists
//! the sites that name what the place names (`LISTED`); the agent classifies each (`CLASSIFIED`):
//! - `applies`: the behavior asked for reaches it; a contract case of kind `KIND` that needs its
//!   own test like any case (`place_cases`), confirmed by the runner as a normal case is;
//! - `not_affected`: with a one-line reason, which the judge reads at the seal against the code.
//!
//! The rules read the part's listing since its latest anchor exit that is not a kept place, and,
//! per listed site, the latest classification of it after that listing; one `not_affected` with a
//! blank reason, or a verdict that is neither, classifies nothing. While a listed site of a part in
//! its turn is unclassified and no commit of its tests has landed since the listing, the part holds `HOLD`, the
//! agent's (never a card): its tests-first commit is refused until every site is classified. A
//! listing with no site holds nothing.

use familiar_ai_facts::Fact;
use serde_json::Value;

use crate::plan;
use crate::reader::{text, t_of, View};

/// The binary's listing of a part's sites [draft name].
pub const LISTED: &str = "places_listed";
/// The agent's classification of one listed site [draft name].
pub const CLASSIFIED: &str = "place_classified";
/// The hold while a listed site is unclassified [draft name].
pub const HOLD: &str = "places_unclassified";
/// The kind of the contract case a site that applies adds [draft name].
pub const KIND: &str = "place";
/// The verdicts [draft names].
pub const APPLIES: &str = "applies";
pub const NOT_AFFECTED: &str = "not_affected";

/// One listed site, as the rules read it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Site {
    pub file: String,
    pub line: u64,
    /// The line defining what the site is in, trimmed; empty when none.
    pub def: String,
    pub names: Vec<String>,
}

impl Site {
    /// `file:line`.
    pub fn at(&self) -> String {
        format!("{}:{}", self.file, self.line)
    }

    /// The site in words: `src/a.py:12 (in def f(x):) names a, b`.
    pub fn words(&self) -> String {
        let def = if self.def.is_empty() { String::new() } else { format!(" (in {})", self.def) };
        format!("{}{def} names {}", self.at(), self.names.join(", "))
    }
}

/// The anchor exit a listing of `c` belongs to: its latest place that is not a kept place, while
/// a place is chosen (none after a re_plan sends planning back to choosing the code).
pub fn exit_of<'a>(v: &View<'a>, c: &str) -> Option<&'a Fact> {
    plan::anchor_of(v, c)?;
    plan::epoch_exit(v, c).filter(|a| a.kind == "place_chosen")
}

/// The binary's latest listing of `c` since its anchor exit.
pub fn listing<'a>(v: &View<'a>, c: &str) -> Option<&'a Fact> {
    let a = exit_of(v, c)?;
    v.of(LISTED, c).filter(|f| t_of(f) > t_of(a)).last()
}

/// The sites of a listing, in its order.
pub fn sites_in(f: &Fact) -> Vec<Site> {
    let s = |x: &Value, k: &str| x.get(k).and_then(Value::as_str).unwrap_or("").to_string();
    f.fields
        .get("sites")
        .and_then(Value::as_array)
        .map(|xs| {
            xs.iter()
                .map(|x| Site {
                    file: s(x, "file"),
                    line: x.get("line").and_then(Value::as_u64).unwrap_or(0),
                    def: s(x, "def"),
                    names: x.get("names").and_then(Value::as_array).map(|ns| ns.iter().filter_map(Value::as_str).map(str::to_string).collect()).unwrap_or_default(),
                })
                .collect()
        })
        .unwrap_or_default()
}

/// The sites of `c`'s listing; empty with none.
pub fn sites(v: &View, c: &str) -> Vec<Site> {
    listing(v, c).map(sites_in).unwrap_or_default()
}

/// Whether a classification says something: `applies`, or `not_affected` with a reason.
fn counts(f: &Fact) -> bool {
    match text(f, "verdict") {
        Some(APPLIES) => true,
        Some(NOT_AFFECTED) => text(f, "reason").is_some_and(|r| !r.trim().is_empty()),
        _ => false,
    }
}

/// The latest classification of `site` that counts, made after `c`'s listing.
pub fn classified<'a>(v: &View<'a>, c: &str, site: &Site) -> Option<&'a Fact> {
    let l = listing(v, c)?;
    v.of(CLASSIFIED, c)
        .filter(|f| t_of(f) > t_of(l) && text(f, "file") == Some(site.file.as_str()) && f.fields.get("line").and_then(Value::as_u64) == Some(site.line) && counts(f))
        .last()
}

/// The listed sites of `c` no classification covers, in the listing's order.
pub fn unclassified(v: &View, c: &str) -> Vec<Site> {
    sites(v, c).into_iter().filter(|s| classified(v, c, s).is_none()).collect()
}

/// Whether a commit of `c`'s tests landed after `t`.
fn tests_landed_after(v: &View, c: &str, t: i64) -> bool {
    v.kind("commit_seen").any(|f| t_of(f) > t && crate::criterion::touches_tests_of(v, c, f))
}

/// Whether `c` holds `HOLD`: its turn, no commit of its tests since its listing, and a listed site
/// left unclassified. The hold gates the tests-first commit; one that landed past it (a commit no
/// hook saw) leaves nothing to gate.
pub fn holds(v: &View, c: &str) -> bool {
    crate::lifecycle::turn(v, c) && listing(v, c).is_some_and(|l| !tests_landed_after(v, c, t_of(l))) && !unclassified(v, c).is_empty()
}

/// The contract cases of `c`'s sites that apply, in the listing's order: case `place<n>`, `n` the
/// site's place in the listing; given the same call through the site, expecting what the agent's
/// reason says, else what the promise gives back.
pub fn place_cases(v: &View, c: &str) -> Vec<plan::Case> {
    let gives_back = plan::promise_of(v, c).and_then(|p| text(p, "gives_back")).unwrap_or("").to_string();
    sites(v, c)
        .into_iter()
        .enumerate()
        .filter_map(|(i, s)| {
            let f = classified(v, c, &s).filter(|f| text(f, "verdict") == Some(APPLIES))?;
            let def = if s.def.is_empty() { String::new() } else { format!(" ({})", s.def) };
            let reason = text(f, "reason").map(str::trim).filter(|r| !r.is_empty()).map(str::to_string).unwrap_or_else(|| gives_back.clone());
            Some(plan::Case { case_id: format!("place{}", i + 1), kind: KIND.to_string(), given: format!("the same call through {}{def}", s.at()), expect: reason })
        })
        .collect()
}

/// The sites of `c` the agent judged not affected, each with its reason, in the listing's order.
pub fn not_affected(v: &View, c: &str) -> Vec<(Site, String)> {
    sites(v, c)
        .into_iter()
        .filter_map(|s| {
            let f = classified(v, c, &s).filter(|f| text(f, "verdict") == Some(NOT_AFFECTED))?;
            let reason = text(f, "reason").unwrap_or("").trim().to_string();
            Some((s, reason))
        })
        .collect()
}

/// Why the door refuses a classification of `file:line` for `c`, in the agent's words; `None`
/// when it may be claimed.
pub fn refusal(v: &View, c: &str, file: &str, line: u64, verdict: &str, reason: &str) -> Option<String> {
    let all = sites(v, c);
    if !all.iter().any(|s| s.file == file && s.line == line) {
        let listed: Vec<String> = all.iter().map(Site::at).collect();
        return Some(if listed.is_empty() {
            format!("{file}:{line} is not a listed site of {c}: no site of {c} is listed")
        } else {
            format!("{file}:{line} is not a listed site of {c}; its sites: {}", listed.join(", "))
        });
    }
    if verdict == NOT_AFFECTED && reason.trim().is_empty() {
        return Some(format!("not_affected needs its reason: one line on why the behavior asked for does not reach {file}:{line}"));
    }
    None
}

/// The binary's listing owed for `c`: the sha its place's lines were checked at for planning,
/// once the plan is confirmed, while its anchor exit (not a kept place) has every place checked
/// and matched, no listing since, no tests-first commit and no commit of its tests since the exit.
/// `None` otherwise.
pub fn owed(v: &View, c: &str) -> Option<String> {
    // once the plan is confirmed, so every part's place is known; before any fix: a part whose
    // tests-first commit landed is listed no more
    if !crate::criterion::confirmed(v) {
        return None;
    }
    let a = exit_of(v, c)?;
    if v.of(LISTED, c).any(|f| t_of(f) > t_of(a)) || crate::criterion::tests_first_commit(v, c).is_some() || tests_landed_after(v, c, t_of(a)) {
        return None;
    }
    let places = plan::places_of(a);
    if places.is_empty() {
        return None;
    }
    let mut sha: Option<String> = None;
    for p in &places {
        let ch = plan::check_of(v, c, p).filter(|ch| t_of(ch) > t_of(a))?;
        if ch.fields.get("matched") != Some(&Value::Bool(true)) {
            return None;
        }
        sha.get_or_insert_with(|| text(ch, "sha").unwrap_or("").to_string());
    }
    sha.filter(|s| !s.is_empty())
}
