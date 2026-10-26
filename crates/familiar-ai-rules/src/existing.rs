//! An existing test's lines change only when announced (the owner, 2026-10-20; the bench's
//! astroid-3152) [draft names]. The binary writes on every `commit_seen` the standing change since
//! the plan's base to the test definitions that stood there (`existing_tests_changed`: per file,
//! the tests and the base's lines changed or removed; the agent's own planned tests never, a test
//! added beside them never). The rules read the head's, the latest `commit_seen`:
//! - a file's change stands **unannounced** while no `ANNOUNCED` for that file came after the
//!   change, exactly as it stands now, first landed; or while the judge's latest word after the
//!   latest such announcement is "no". A "not judged" never holds.
//! - while one stands, each part whose turn it is holds `HOLD`, the agent's: it clears by an
//!   announcement with a reason or by a commit that puts the lines back; never a card.

use familiar_ai_facts::Fact;
use serde_json::Value;

use crate::reader::{t_of, text, View};

/// The hold [draft name].
pub const HOLD: &str = "existing_test_changed";
/// The agent's announcement of a change to a pre-existing test file [draft name].
pub const ANNOUNCED: &str = "existing_test_announced";
/// The judge's word on an announcement [draft name].
pub const VERDICT: &str = "existing_test_verdict";
/// The field of `commit_seen` that carries the standing change [draft name].
pub const FIELD: &str = "existing_tests_changed";

/// The standing change of `f`, one entry per file.
pub fn changes_of(f: &Fact) -> Vec<&Value> {
    f.fields.get(FIELD).and_then(Value::as_array).map(|xs| xs.iter().collect()).unwrap_or_default()
}

fn file_of(e: &Value) -> &str {
    e.get("file_path").and_then(Value::as_str).unwrap_or("")
}

/// The standing change at the head: the latest `commit_seen`'s.
pub fn standing<'a>(v: &View<'a>) -> Vec<&'a Value> {
    v.last("commit_seen").map(changes_of).unwrap_or_default()
}

/// Where the change `e` first landed, exactly as it stands: the first `commit_seen` whose entry
/// for its file equals it.
fn first_landed(v: &View, e: &Value) -> i64 {
    v.kind("commit_seen")
        .find(|f| changes_of(f).contains(&e))
        .map(t_of)
        .unwrap_or(crate::reader::INF)
}

/// The latest announcement for `e`'s file made after `e` first landed.
pub fn announcement<'a>(v: &View<'a>, e: &Value) -> Option<&'a Fact> {
    let from = first_landed(v, e);
    v.kind(ANNOUNCED).filter(|a| t_of(a) > from && text(a, "file_path") == Some(file_of(e))).last()
}

/// The judge's latest yes or no on the announcement `a`, read after it.
pub fn verdict_on<'a>(v: &View<'a>, a: &Fact) -> Option<&'a Fact> {
    let file = text(a, "file_path");
    v.kind(VERDICT)
        .filter(|f| f.t > a.t && text(f, "file_path") == file && matches!(text(f, "covers"), Some("yes" | "no")))
        .last()
}

/// The verdicts after `a` on its file that gave no word.
pub fn not_judged_on<'a>(v: &View<'a>, a: &Fact) -> usize {
    let file = text(a, "file_path");
    v.kind(VERDICT).filter(|f| f.t > a.t && text(f, "file_path") == file && text(f, "covers") == Some("not_judged")).count()
}

/// The standing changes no announcement covers, or whose announcement the judge said no to.
pub fn unannounced<'a>(v: &View<'a>) -> Vec<&'a Value> {
    standing(v)
        .into_iter()
        .filter(|e| match announcement(v, e) {
            None => true,
            Some(a) => verdict_on(v, a).is_some_and(|x| text(x, "covers") == Some("no")),
        })
        .collect()
}

/// The judge's "no" that holds `e`, when one does.
pub fn judged_no<'a>(v: &View<'a>, e: &Value) -> Option<&'a Fact> {
    announcement(v, e).and_then(|a| verdict_on(v, a)).filter(|x| text(x, "covers") == Some("no"))
}

/// The announcements standing changes rest on that the judge owes a word: no yes or no after it,
/// and fewer than `twice` calls that gave none. Oldest first.
pub fn owed<'a>(v: &View<'a>, twice: usize) -> Vec<(&'a Value, &'a Fact)> {
    standing(v)
        .into_iter()
        .filter_map(|e| announcement(v, e).map(|a| (e, a)))
        .filter(|(_, a)| verdict_on(v, a).is_none() && not_judged_on(v, a) < twice)
        .collect()
}

/// Whether `c` holds `HOLD`: planning confirmed, its turn, and a change stands unannounced.
pub fn holds(v: &View, c: &str) -> bool {
    crate::criterion::confirmed(v) && crate::lifecycle::turn(v, c) && !unannounced(v).is_empty()
}

/// The lines of an entry as words: `lines 12–13, 20`.
pub fn lines_words(e: &Value) -> String {
    let rs: Vec<String> = e
        .get("lines")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|r| {
            let a = r.get(0)?.as_u64()?;
            let z = r.get(1)?.as_u64()?;
            Some(if a == z { a.to_string() } else { format!("{a}–{z}") })
        })
        .collect();
    let word = if rs.len() == 1 && !rs[0].contains('–') { "line" } else { "lines" };
    format!("{word} {}", rs.join(", "))
}

/// An entry in words: `tests/test_a.py (test_x, lines 12–13)`.
pub fn words(e: &Value) -> String {
    let tests: Vec<&str> = e.get("tests").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str).collect();
    let tests = if tests.is_empty() { String::new() } else { format!("{}, ", tests.join(", ")) };
    format!("{} ({tests}{})", file_of(e), lines_words(e))
}
