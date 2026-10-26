//! "The most specific stack root winning over the latest" (the owner, CLAUDE.md "Owed").
//! Among the stacks with a command whose root covers every anchor file, the
//! longest root wins; equal roots, the latest. `.` (the empty root) never
//! displaces `src/`.
//!
//! c1 is anchored under `src/`. Stacks claimed: root `src/` command A, then
//! root `` command B: c1's command stays A and does not change under the
//! seal. Then root `src/` command C: C, the latest of equal roots.

use familiar_ai_facts::Log;

use familiar_ai_rules::sample::criterion_fixture;
use familiar_ai_rules::{criterion, lifecycle, View};
use serde_json::{json, Value};

const AT: f64 = 0.0;
const TENANT: &str = "_default_";

fn raw(log: &Log, kind: &str, fields: Value) -> Log {
    let Value::Object(m) = fields else { unreachable!() };
    familiar_ai_facts::append_raw(log, kind, m, AT, TENANT).expect("the door takes it")
}

/// A stack with a runner the table does not derive from, so the command is the claimed one.
fn stack(log: &Log, root: &str, command: &str) -> Log {
    raw(log, "test_stack_detected", json!({ "codebase_id": "cb1", "project_root": root, "test_stack": "make", "test_command": command }))
}

#[test]
fn the_most_specific_stack_root_wins_over_the_latest() {
    let log = criterion_fixture(&["c1"], false, false, AT, TENANT).expect("fixture");
    let log = stack(&log, "src/", "run-a");
    let mark = View::new(&log).facts.last().map(|f| f.t).expect("facts");
    let log = stack(&log, "", "run-b");
    let v = View::new(&log);
    assert_eq!(criterion::command(&v, "c1").as_deref(), Some("run-a"), "the root `` never displaces `src/`");
    assert_eq!(lifecycle::command_at(&v, "c1", i64::MAX), Some("run-a"));
    let after_a = v.facts.iter().find(|f| f.t > mark).map(|f| i64::try_from(f.t).unwrap()).expect("a fact after A");
    assert_eq!(lifecycle::command_at(&v, "c1", after_a), lifecycle::command_at(&v, "c1", i64::MAX), "the command did not change under a seal");

    let log = stack(&log, "src/", "run-c");
    let v = View::new(&log);
    assert_eq!(criterion::command(&v, "c1").as_deref(), Some("run-c"), "equal roots: the latest");
    assert_eq!(lifecycle::command_at(&v, "c1", i64::MAX), Some("run-c"));
}
