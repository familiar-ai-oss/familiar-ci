//! An existing test's lines change only when announced (the owner, 2026-10-20; the bench's
//! astroid-3152) [draft names]. The binary writes on every `commit_seen` the lines of test
//! definitions that stood at the plan's base and that the commit's tree no longer holds as they
//! were (`existing_tests_changed`: file, tests, lines); the agent's own planned tests are never
//! among them, and a new test function added beside them is no change. The rules read the head's:
//! - while a change there stands with no announcement made after it first landed, or with the
//!   judge's "no" on the latest one, each part at work is held `existing_test_changed`;
//! - the agent clears it by announcing the file with a reason (`existing_test_announced`),
//!   which the judge reads against the person's words (`existing_test_verdict`), or by putting the
//!   lines back. A "no" keeps it held; another announcement or the revert clears it. It is the
//!   agent's hold, never a card, and it pauses nothing.

use familiar_ai_facts::Log;
use familiar_ai_rules::criterion::{answers_of, PAUSING};
use familiar_ai_rules::sample::{apply_move, criterion_fixture};
use familiar_ai_rules::{existing, lifecycle, moves, terminal, Event, View};
use serde_json::{json, Value};

const AT: f64 = 0.0;
const TENANT: &str = "_default_";
const OLD: &str = "tests/test_login_old.py";

fn fact(log: &Log, kind: &str, fields: Value) -> Log {
    familiar_ai_facts::append_raw(log, kind, fields.as_object().cloned().unwrap_or_default(), AT, TENANT).unwrap_or_else(|e| panic!("{kind}: {e}"))
}

fn step(log: &Log, label: &str, outcome: Option<&str>) -> Log {
    let v = View::new(log);
    let m = moves::criterion_admissible(&v)
        .into_iter()
        .find(|m| m.label.as_deref() == Some(label) && outcome.is_none_or(|o| m.fields.get("outcome").and_then(Value::as_str) == Some(o)));
    match m {
        Some(m) => apply_move(log, &m, AT, TENANT).unwrap_or_default(),
        None => panic!("no {label} move at:\n{}", terminal::status(&v)),
    }
}

fn ran(log: &Log, outcome: &str) -> Log {
    let log = step(log, "pass_please_pass", Some(outcome));
    let log = step(&log, "pass_please_pass", Some("passed"));
    step(&log, "fingerprint_stamped", None)
}

/// c1 sealed red, then green at a code commit.
fn green() -> Log {
    let log = criterion_fixture(&["c1"], true, false, AT, TENANT).unwrap_or_default();
    let log = step(&log, "start_check", None);
    let log = step(&log, "tests_only", None);
    let log = step(&log, "test_written", None);
    let log = ran(&log, "failed");
    let log = ran(&step(&log, "code", None), "passed");
    assert_eq!(state(&log), "green", "{}", terminal::status(&View::new(&log)));
    log
}

fn state(log: &Log) -> &'static str {
    lifecycle::state(&View::new(log), "c1")
}

fn whys(log: &Log) -> Vec<String> {
    lifecycle::whys(&View::new(log), "c1")
}

/// The change the binary reads in `OLD`: the test and the base's lines.
fn change(test: &str, lines: [u64; 2]) -> Value {
    json!({ "file_path": OLD, "tests": [test], "lines": [lines] })
}

/// A commit whose tree stands with `changes` against the base: the last code commit's facts
/// again, under a new id, with `OLD` among its files when it changed it.
fn commit(log: &Log, changes: Vec<Value>) -> Log {
    let at = log.iter().rposition(|f| f.kind == "commit_seen" && f.fields.get("touches_code") == Some(&json!(true))).expect("a code commit");
    let old = log[at].fields.get("commit").cloned().unwrap_or_default();
    let id = format!("k-existing-{}", log.len());
    let mut out = log.clone();
    for f in log[at..].iter().filter(|f| ["commit_seen", "tests_observed"].contains(&f.kind) && f.fields.get("commit") == Some(&old)) {
        let mut fields = Value::Object(f.fields.clone());
        fields["commit"] = json!(id);
        if f.kind == "commit_seen" {
            fields["files"] = json!(["src/login.py", OLD]);
            match fields.as_object_mut() {
                Some(m) if changes.is_empty() => {
                    m.remove("existing_tests_changed");
                }
                Some(m) => {
                    m.insert("existing_tests_changed".into(), json!(changes));
                }
                None => {}
            }
        }
        out = fact(&out, f.kind, fields);
    }
    settled(&out)
}

/// The binary's runs owed at the new commit, each passing, then its stamp.
fn settled(log: &Log) -> Log {
    let mut log = log.clone();
    loop {
        let v = View::new(&log);
        let owed = moves::binary_moves(&v);
        let Some(m) = owed.iter().find(|m| m.fields.get("outcome").and_then(Value::as_str) == Some("passed")).or(owed.first()).cloned() else {
            return log;
        };
        log = apply_move(&log, &m, AT, TENANT).unwrap_or_default();
    }
}

fn announce(log: &Log) -> Log {
    fact(log, existing::ANNOUNCED, json!({ "file_path": OLD, "reason": "the person asked that the sentinel no longer shows" }))
}

fn verdict(log: &Log, covers: &str) -> Log {
    let head = log.iter().rev().find(|f| f.kind == "commit_seen").and_then(|f| f.fields.get("commit")).cloned().unwrap_or_default();
    let mut f = json!({ "file_path": OLD, "commit": head, "model": "claude-opus-4-6", "covers": covers, "reason": "the judge's reason" });
    match covers {
        "no" => f["gap"] = json!("the ask never names the default the test checks"),
        "not_judged" => f["why"] = json!("timeout"),
        _ => {}
    }
    fact(log, existing::VERDICT, f)
}

#[test]
fn the_hold_is_the_agents_and_pauses_nothing() {
    assert_eq!(existing::HOLD, "existing_test_changed");
    assert_eq!(answers_of(existing::HOLD), None, "never a card");
    assert!(!PAUSING.contains(&existing::HOLD));
}

#[test]
fn a_changed_line_of_an_existing_test_holds_the_part_naming_file_test_and_lines() {
    let log = commit(&green(), vec![change("t_old", [12, 13])]);
    assert_eq!(whys(&log), [existing::HOLD]);
    assert_eq!(state(&log), "held");
    let v = View::new(&log);
    let h = lifecycle::holds(&v, "c1");
    let clears: Vec<&str> = h[0].clears.iter().filter_map(|c| c.get("kind").and_then(Value::as_str)).collect();
    assert_eq!(clears, [existing::ANNOUNCED, "commit_seen"]);
    assert_eq!(h[0].clears[0].get("file_path").and_then(Value::as_str), Some(OLD));
    // the announcement is an admissible move, and the agent's next commit carries the change on
    assert!(moves::criterion_admissible(&v).iter().any(|m| m.kind == existing::ANNOUNCED && m.fields.get("file_path").and_then(Value::as_str) == Some(OLD)));
    let words = terminal::refusal(&v, &Event::Stops { main: true }).unwrap_or_default();
    assert!(words.contains(OLD) && words.contains("t_old") && words.contains("lines 12–13"), "{words}");
    let next = settled(&step(&log, "code", None));
    assert_eq!(whys(&next), [existing::HOLD], "a later commit with the change standing holds as well");
}

#[test]
fn no_change_at_the_head_holds_nothing() {
    let log = commit(&green(), Vec::new());
    assert!(whys(&log).is_empty(), "{:?}", whys(&log));
    assert_eq!(state(&log), "green");
}

#[test]
fn an_announcement_clears_it_and_a_judges_covers_keeps_it_clear() {
    let log = announce(&commit(&green(), vec![change("t_old", [12, 12])]));
    assert!(whys(&log).is_empty(), "announced, not judged no: {:?}", whys(&log));
    assert_eq!(state(&log), "green");
    let yes = verdict(&log, "yes");
    assert!(whys(&yes).is_empty());
    assert!(whys(&verdict(&log, "not_judged")).is_empty(), "not judged never holds");
}

#[test]
fn a_judges_no_holds_again_until_another_announcement_or_the_revert() {
    let no = verdict(&announce(&commit(&green(), vec![change("t_old", [12, 12])])), "no");
    assert_eq!(whys(&no), [existing::HOLD]);
    // another announcement, with another reason, is read again
    assert!(whys(&announce(&no)).is_empty());
    assert_eq!(whys(&verdict(&announce(&no), "no")), [existing::HOLD]);
    // the lines put back: no change stands
    let reverted = commit(&no, Vec::new());
    assert!(whys(&reverted).is_empty(), "{:?}", whys(&reverted));
    assert_eq!(state(&reverted), "green");
}

#[test]
fn an_announcement_covers_the_change_that_stood_when_it_was_made() {
    let log = announce(&commit(&green(), vec![change("t_old", [12, 12])]));
    // another line of the file changed after it: announced again
    let more = commit(&log, vec![change("t_old", [12, 12]), change("t_other", [30, 30])]);
    assert_eq!(whys(&more), [existing::HOLD]);
    assert!(whys(&announce(&more)).is_empty());
    // an announcement made before the change landed covers nothing
    let early = commit(&announce(&green()), vec![change("t_old", [12, 12])]);
    assert_eq!(whys(&early), [existing::HOLD]);
}

#[test]
fn a_rebase_keeps_the_change_standing() {
    let log = commit(&green(), vec![change("t_old", [12, 12])]);
    let rebased = step(&log, "rebase", None);
    assert_eq!(whys(&rebased), [existing::HOLD]);
}
