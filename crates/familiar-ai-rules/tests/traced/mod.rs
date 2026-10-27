//! A part whose tests run under pytest, planned up to its test (lines 1–3 and 6–10) with no place
//! yet, for the tests of the try before the place (the owner's plan of 2026-10-23) [draft names].
#![allow(dead_code)]

use familiar_ai_facts::{append_raw, Log};
use familiar_ai_rules::hooks::{decide, Decision, Event};
use familiar_ai_rules::{lifecycle, View};
use serde_json::{json, Value};

pub const AT: f64 = 0.0;
pub const TENANT: &str = "_default_";
pub const TEST: &str = "tests/test_export.py";
pub const CODE: &str = "src/export.py";
/// Words that show no way to make it happen: no command, code or quoted output.
pub const VAGUE: &str = "the export is broken again, please fix it";
/// Words that show how: a command and what it prints.
pub const SHOWN: &str = "running `python -m export --all` prints every row twice";

pub fn raw(log: &Log, kind: &str, fields: Value) -> Log {
    let Value::Object(m) = fields else { unreachable!() };
    append_raw(log, kind, m, AT, TENANT).unwrap_or_else(|e| panic!("{kind}: {e}"))
}

/// c1 planned on the full path up to its test, under `stack`, the intent naming the person's
/// `words` when given; no place chosen.
pub fn planned_with(words: Option<&str>, stack: &str) -> Log {
    let mut log: Log = Vec::new();
    let mut intent = json!({ "text": "the export writes each row once", "sha": "base" });
    if let Some(w) = words {
        log = raw(&log, "person_said", json!({ "text": w }));
        intent["for"] = json!(log[0].t);
    }
    log = raw(&log, "intent_submitted", intent);
    log = raw(&log, "behavior", json!({ "behavior_id": "b1", "text": "the export writes rows" }));
    log = raw(&log, "promise", json!({ "promise_id": "p1", "behavior_id": "b1", "accepts": "rows", "gives_back": "each row once", "refuses": "nothing" }));
    log = raw(&log, "criterion", json!({ "criterion_id": "c1", "text": "the export writes each row once", "path": "full", "behavior_id": "b1", "promise_id": "p1" }));
    log = raw(&log, "test_planned", json!({ "criterion_id": "c1", "file_path": TEST, "will": "create" }));
    log = raw(&log, "test_stack_detected", json!({ "codebase_id": "cb1", "project_root": "", "test_stack": stack, "test_command": "python -m pytest" }));
    log = raw(&log, "criterion_head", json!({ "criterion_id": "c1" }));
    log = raw(&log, "criterion_assigned", json!({ "criterion_id": "c1", "agent_conversation_id": "a1" }));
    raw(&log, "fix_location_category", json!({ "criterion_id": "c1", "category": "this_repository", "where": "" }))
}

/// c1 under pytest, with words that show how to make it happen, so no question is owed.
pub fn planned() -> Log {
    planned_with(Some(SHOWN), "pytest")
}

/// The binary's listing after a try of c1.
pub fn tried(log: &Log, red: bool, top: Value) -> Log {
    raw(log, "trace_listed", json!({ "criterion_id": "c1", "run_id": format!("try-{}", log.len()), "sha": "base", "red": red, "path_size": 12, "ranker": "words", "query": "the export", "top": top }))
}

/// A listing of `Writer.write_rows` (lines 10–30) and `header` (lines 32–40) in `CODE`.
pub fn top() -> Value {
    json!([
        { "rank": 1, "file_path": CODE, "symbol": "Writer.write_rows", "line_start": 10, "line_end": 30 },
        { "rank": 2, "file_path": CODE, "symbol": "header", "line_start": 32, "line_end": 40 }
    ])
}

/// The agent's place, and the binary's planning check that its lines matched.
pub fn placed(log: &Log, file: &str, symbol: &str, lines: (u64, u64)) -> Log {
    let place = json!({ "file_path": file, "symbol": symbol, "line_start": lines.0, "line_end": lines.1 });
    let log = raw(log, "place_chosen", json!({ "criterion_id": "c1", "places": [place] }));
    raw(&log, "anchor_sha_matched", json!({ "criterion_id": "c1", "file_path": file, "symbol": symbol, "line_start": lines.0, "line_end": lines.1, "sha": "base", "lines_hash": "L0", "matched": true, "checked_at": "planning" }))
}

pub fn whys(log: &Log) -> Vec<String> {
    lifecycle::holds(&View::new(log), "c1").iter().map(|h| h.why.to_string()).collect()
}

pub fn edit(log: &Log, file: &str) -> Decision {
    decide(&View::new(log), &Event::FileEdit { criterion_id: Some("c1".into()), file_path: file.into() })
}

pub fn hold_of(d: &Decision) -> Option<String> {
    match d {
        Decision::Refuse(r) => r.hold.clone(),
        Decision::Allow { .. } => None,
    }
}

/// The lens panel's five lenses, by id (the owner's plan of 2026-10-26, approved).
pub const LENSES: [&str; 5] = ["ambiguity", "completeness", "happy_path", "edge_cases", "order_and_coupling"];

/// One lens's answer on `c`, as the binary records it: found with one finding, else none or failed.
pub fn lens(log: &Log, c: &str, lens: &str, outcome: &str) -> Log {
    let findings = if outcome == "found" {
        json!([{ "kind": lens, "note": format!("{lens} noticed something"), "file_path": CODE, "target_symbol": "Writer.write_rows", "candidate_question": "" }])
    } else {
        json!([])
    };
    raw(log, "lens_found", json!({ "criterion_id": c, "lens": lens, "outcome": outcome, "findings": findings, "model": "claude-opus-5-5", "tokens": 3000 }))
}

/// The five lenses' answers on `c` (none each), the panel before its synthesis.
pub fn lenses(log: &Log, c: &str) -> Log {
    LENSES.iter().fold(log.clone(), |l, x| lens(&l, c, x, "none"))
}
