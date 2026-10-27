//! A part planned on the full path with a whole contract, for the cases card's words (the owner's
//! plan of 2026-10-25) [draft names]: the person's words, the intent at its sha, the promise's two
//! cases (n1 with a short line, o1 with one too), and the case-finder's finding.
#![allow(dead_code)]

use familiar_ai_facts::{append_raw, Log};
use serde_json::{json, Value};

pub const SAID: &str = "error groups with every frame hidden crash with an IndexError";
pub const PART: &str = "error groups with every frame hidden show a clear message";
pub const SHA: &str = "4f1c2ab";

pub fn raw(log: &Log, kind: &str, fields: Value) -> Log {
    let Value::Object(m) = fields else { unreachable!() };
    append_raw(log, kind, m, 0.0, "_default_").unwrap_or_else(|e| panic!("{kind}: {e}"))
}

/// c1 planned up to its test, its promise whole: n1 and o1, each with its short line, `o1_short`
/// left out when false.
pub fn planned_with(o1_short: bool) -> Log {
    let mut log = raw(&Vec::new(), "person_said", json!({ "text": SAID }));
    let t = log[0].t;
    log = raw(&log, "intent_submitted", json!({ "text": PART, "sha": SHA, "for": t }));
    log = raw(&log, "behavior", json!({ "behavior_id": "b1", "text": "a hidden traceback reads plainly" }));
    let mut o1 = json!({ "case_id": "o1", "kind": "opposite", "given": "a group with visible frames", "expect": "the traceback as before" });
    if o1_short {
        o1["short"] = json!("a normal group looks the same as before");
    }
    log = raw(
        &log,
        "promise",
        json!({ "promise_id": "p1", "behavior_id": "b1", "accepts": "an error group whose frames are all hidden", "gives_back": "a clear message in place of the traceback", "refuses": "nothing", "contract_version": 1, "cases": [
            { "case_id": "n1", "kind": "normal", "short": "a group where every frame is hidden shows the message", "given": "an ExceptionGroup whose every frame is hidden", "expect": "the hidden-entries message" },
            o1
        ] }),
    );
    log = raw(&log, "criterion", json!({ "criterion_id": "c1", "text": PART, "path": "full", "behavior_id": "b1", "promise_id": "p1" }));
    log = raw(&log, "test_planned", json!({ "criterion_id": "c1", "file_path": "testing/test_groups.py", "will": "create" }));
    log = raw(&log, "test_stack_detected", json!({ "codebase_id": "cb1", "project_root": "", "test_stack": "pytest", "test_command": "python -m pytest" }));
    log = raw(&log, "criterion_head", json!({ "criterion_id": "c1" }));
    log = raw(&log, "criterion_assigned", json!({ "criterion_id": "c1", "agent_conversation_id": "a1" }));
    raw(&log, "fix_location_category", json!({ "criterion_id": "c1", "category": "this_repository", "where": "" }))
}

pub fn planned() -> Log {
    planned_with(true)
}

pub const FOUND: [(&str, &str, &str); 3] = [
    ("the same with the other --tb styles (short, line, native)", "an all-hidden group under --tb=short, line and native", "the message in each style"),
    ("a group inside another group", "a nested group whose frames are all hidden", "the message, no IndexError"),
    ("a plain error with every frame hidden", "a plain exception whose frames are all hidden", "the message, no IndexError"),
];

/// The lens panel's five lenses, in release order.
pub const LENSES: [&str; 5] = ["ambiguity", "completeness", "happy_path", "edge_cases", "order_and_coupling"];

/// Each lens's answer on c1, none, as the binary records it.
pub fn lensed(log: &Log) -> Log {
    LENSES.iter().fold(log.clone(), |l, x| raw(&l, "lens_found", json!({ "criterion_id": "c1", "lens": x, "outcome": "none", "findings": [], "model": "claude-opus-5-5", "tokens": 3000 })))
}

/// The lens panel's synthesis on c1 (updated with the definition, the owner's plan of 2026-10-26:
/// after the five lenses' answers): `outcome`, with the first `n` of `FOUND` as a1..an.
pub fn found(log: &Log, outcome: &str, n: usize) -> Log {
    let log = &lensed(log);
    let cs: Vec<Value> = FOUND
        .iter()
        .take(n)
        .enumerate()
        .map(|(i, (short, given, expect))| json!({ "case_id": format!("a{}", i + 1), "short": short, "given": given, "expect": expect, "why": "the code branches here" }))
        .collect();
    let why = if n == 0 { "the planned cases cover it" } else { "" };
    raw(log, "cases_found", json!({ "criterion_id": "c1", "outcome": outcome, "cases": cs, "why": why, "model": "claude-sonnet-5-5", "tokens": 4000 }))
}

pub fn answered(log: &Log, choice: &str, typed: &str) -> Log {
    raw(log, "hold_answered", json!({ "criterion_id": "c1", "hold": "cases_unconfirmed", "choice": choice, "typed": typed }))
}
