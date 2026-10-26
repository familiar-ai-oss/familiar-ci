//! The fixtures the test files share: one sample of each fact kind, and the time and tenant they are appended with.
#![allow(dead_code)]

use serde_json::{json, Map, Value};

pub const AT: f64 = 1_700_000_000.0;
pub const TENANT: &str = "facts_tests";

pub fn obj(pairs: &[(&str, Value)]) -> Map<String, Value> {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.clone()))
        .collect()
}

/// One sample per kind, in `KINDS` order, with every optional field set so the round trips cover it.
pub fn samples() -> Vec<(&'static str, Map<String, Value>)> {
    vec![
        (
            "intent_submitted",
            obj(&[
                (
                    "text",
                    json!("let a person log in and reset a forgotten password"),
                ),
                ("sha", json!("base")),
                ("for", json!(0)),
            ]),
        ),
        (
            "behavior",
            obj(&[
                ("behavior_id", json!("b1")),
                ("text", json!("a person logs in with a password")),
            ]),
        ),
        (
            "promise",
            obj(&[
                ("promise_id", json!("p1")),
                ("behavior_id", json!("b1")),
                ("accepts", json!("a user name and a password")),
                ("gives_back", json!("a session")),
                ("refuses", json!("a wrong password")),
            ]),
        ),
        (
            "criterion",
            obj(&[
                ("criterion_id", json!("c1")),
                ("text", json!("a right password opens a session")),
                ("path", json!("full")),
                ("behavior_id", json!("b1")),
                ("promise_id", json!("p1")),
                ("for", json!(0)),
            ]),
        ),
        (
            "place_chosen",
            obj(&[
                ("criterion_id", json!("c1")),
                (
                    "places",
                    json!([{ "file_path": "src/login.py", "symbol": "check_password", "line_start": 40, "line_end": 62 }]),
                ),
            ]),
        ),
        (
            "new_work_spot_chosen",
            obj(&[
                ("criterion_id", json!("c1")),
                ("file_path", json!("src/new.py")),
                ("symbol", json!("new_fn")),
                ("beside", json!("check_password")),
            ]),
        ),
        ("new_work_no_spot", obj(&[("criterion_id", json!("c1")), ("integration_point", json!("src/app.py:main"))])),
        ("criterion_head", obj(&[("criterion_id", json!("c1"))])),
        (
            "criterion_strung_on_head",
            obj(&[("criterion_id", json!("c2")), ("head", json!("c1"))]),
        ),
        (
            "test_stack_detected",
            obj(&[
                ("codebase_id", json!("cb1")),
                ("project_root", json!("src/")),
                ("test_stack", json!("python_pytest")),
                ("test_command", json!("pytest")),
            ]),
        ),
        (
            "test_written",
            obj(&[
                ("criterion_id", json!("c1")),
                ("file_path", json!("tests/test_c1.py")),
                ("test_name", json!("t1")),
            ]),
        ),
        (
            "pass_please_pass",
            obj(&[
                ("criterion_id", json!("c1")),
                ("file_path", json!("tests/test_c1.py")),
                ("test_name", json!("t1")),
                ("commit", json!("T1")),
                ("outcome", json!("failed")),
                ("ran_in", json!("laptop")),
                ("with_anchor_reverted", json!(false)),
            ]),
        ),
        (
            "fingerprint_stamped",
            obj(&[
                ("criterion_id", json!("c1")),
                ("commit", json!("T1")),
                ("proof_not_tampered", json!("S1")),
                ("ran_in", json!("laptop")),
            ]),
        ),
        (
            "anchor_sha_matched",
            obj(&[
                ("criterion_id", json!("c1")),
                ("file_path", json!("src/login.py")),
                ("symbol", json!("check_password")),
                ("line_start", json!(40)),
                ("line_end", json!(62)),
                ("sha", json!("base")),
                ("lines_hash", json!("L0")),
                ("matched", json!(true)),
                ("checked_at", json!("planning")),
            ]),
        ),
        (
            "test_planned",
            obj(&[
                ("criterion_id", json!("c1")),
                ("file_path", json!("tests/test_c1.py")),
                ("will", json!("create")),
                ("test_command", json!("pytest tests/test_c1.py")),
            ]),
        ),
        (
            "touching_test_declared",
            obj(&[
                ("criterion_id", json!("c1")),
                ("file_path", json!("tests/test_login_old.py")),
                ("test_name", json!("t_old")),
            ]),
        ),
        (
            "criterion_assigned",
            obj(&[
                ("criterion_id", json!("c1")),
                ("agent_conversation_id", json!("a1")),
            ]),
        ),
        (
            "criterion_transfer",
            obj(&[
                ("criterion_id", json!("c1")),
                ("from_conversation", json!("a1")),
                ("to_conversation", json!("a2")),
            ]),
        ),
        (
            "commit_seen",
            obj(&[
                ("commit", json!("K1")),
                ("files", json!(["src/login.py"])),
                ("touches_tests", json!(false)),
                ("touches_code", json!(true)),
                ("anchor_overlap", json!(["c1"])),
                ("tests_hash", json!("S1")),
                ("by", json!("agent")),
            ]),
        ),
        (
            "hold_answered",
            obj(&[
                ("criterion_id", json!("c1")),
                ("hold", json!("anchor_moved")),
                ("choice", json!("keep_going")),
                ("typed", json!("keep going")),
            ]),
        ),
        ("pipeline_run_seen", obj(&[("head_commit", json!("K2"))])),
        (
            "test_change_announced",
            obj(&[
                ("criterion_id", json!("c1")),
                ("reason", json!("the sealed test names the old error")),
            ]),
        ),
        (
            "model_seen",
            obj(&[
                ("agent_conversation_id", json!("s1/a1")),
                ("model", json!("claude-sonnet-5-5")),
            ]),
        ),
        (
            "judge_verdict",
            obj(&[("criterion_id", json!("c1")), ("model", json!("claude-opus-5-5")), ("answers_intent", json!("yes")), ("reason", json!("it checks the asked behaviour")), ("gap", json!("the reset path"))]),
        ),
        ("person_said", obj(&[("text", json!("fix the alias in the schema"))])),
        ("person_said_answered", obj(&[("for", json!(3)), ("how", json!("no_work"))])),
        ("judge_unanswered", obj(&[("criterion_id", json!("c1")), ("seal", json!("4f1c2ab")), ("why", json!("timeout"))])),
        ("investigator_unfinished", obj(&[("criterion_id", json!("c1")), ("conversation", json!("s1/i1")), ("why", json!("no_seal"))])),
        ("fix_location_category", obj(&[("criterion_id", json!("c1")), ("category", json!("dependency")), ("where", json!("the library the anchor calls")), ("needs", json!("a release"))])),
        ("run_reserved", obj(&[("run_id", json!("r1")), ("criterion_id", json!("c1")), ("criterion_rev", json!("v1")), ("commit", json!("K2")), ("command", json!("true")), ("resources", json!([])), ("wait", json!("1800")), ("by", json!("binary")), ("after", json!("1700000060"))])),
        ("run_started", obj(&[("run_id", json!("r1")), ("pgid", json!("4242")), ("pgroup_start", json!("99"))])),
        (
            "run_ended",
            obj(&[
                ("run_id", json!("r1")),
                ("outcome", json!("failed")),
                ("end", json!("exited")),
                ("exit_code", json!(139)),
                ("signal", json!(11)),
                ("stdout_tail", json!("collected 1 item")),
                ("stderr_tail", json!("Segmentation fault (core dumped)")),
                ("truncated", json!([{ "stream": "stdout", "total_bytes": 9000000, "dropped_bytes": 611392 }])),
            ]),
        ),
        ("run_cancel_requested", obj(&[("run_id", json!("r1"))])),
        ("tests_observed", obj(&[("criterion_id", json!("c1")), ("criterion_rev", json!("R1")), ("commit", json!("K1")), ("tests_hash", json!("S1"))])),
        ("ci_report_seen", obj(&[("sha", json!("K1")), ("run_id", json!("4242")), ("finished_at", json!("2026-10-10T12:00:00Z")), ("outcome", json!("proven")), ("report", json!(["hosted CI-proven at K1, run 4242"]))])),
        ("anchor_is_documentation", obj(&[("criterion_id", json!("c1")), ("file_path", json!("README.md")), ("symbol", json!("Usage")), ("line_start", json!(1)), ("line_end", json!(3)), ("sha", json!("K1")), ("matched", json!(true))])),
        ("familiar_switch", obj(&[("to", json!("off"))])),
        (
            "test_log_translation",
            obj(&[
                ("run_id", json!("r1")),
                ("criterion_id", json!("c1")),
                ("commit", json!("T1")),
                ("by", json!("regex")),
                ("label", json!("missing_target_symbol")),
                ("evidence", json!("ImportError: cannot import name 'check_password' from 'login'")),
                ("failing_tests", json!(["tests/test_c1.py::t1"])),
                ("classified_on", json!("full")),
                ("location", json!({ "file_path": "tests/test_c1.py", "line": 3 })),
                ("missing_name", json!("check_password")),
            ]),
        ),
        (
            "dependency_drafted",
            obj(&[
                ("approval", json!(19)),
                ("criterion_id", json!("c2")),
                ("text", json!("the hashing helper returns text, not bytes")),
                ("test_file", json!("tests/test_c2.py")),
                ("place", json!({ "file_path": "src/hashing.py", "symbol": "digest", "line_start": 3, "line_end": 9 })),
            ]),
        ),
        ("dependency_unanswered", obj(&[("approval", json!(19)), ("why", json!("the drafted place is under .venv"))])),
        ("told", obj(&[("about", json!("r1"))])),
        ("criterion_covered", obj(&[("criterion_id", json!("c1")), ("by_criterion", json!("c2"))])),
        ("subagent_ended", obj(&[("agent_id", json!("a1")), ("agent_type", json!("general-purpose"))])),
        ("record_seeded", obj(&[("commit", json!("K3")), ("plan_lines", json!(18))])),
        ("holder_resumed", obj(&[("agent_id", json!("a1"))])),
        (
            "investigator_finding",
            obj(&[
                ("criterion_id", json!("c1")),
                ("category", json!("wrong_test")),
                ("model", json!("claude-opus-4-6")),
                ("runs", json!([{"run_id": "r1", "commit": "K2", "label": "passed"}, {"run_id": "r2", "commit": "K3", "label": "passed"}])),
                ("reason", json!("the test checks the name, not the greeting")),
                (
                    "quotes",
                    json!([
                        {"role": "test", "file_path": "tests/test_c1.sh", "line_start": 1, "line_end": 1, "commit": "K2", "text": "[ -n \"$out\" ]"},
                        {"role": "person", "said": 0, "text": "greet with hello"}
                    ]),
                ),
                ("guidance", json!("check that the output is hello")),
            ]),
        ),
        (
            "reader_chosen",
            obj(&[
                ("run_id", json!("d1")),
                ("root", json!(".")),
                ("runner", json!("pytest")),
                ("reader", json!("pytest_reporter")),
                ("command", json!("pytest -q")),
                ("commit", json!("K3")),
                ("setup", json!("pyproject.toml")),
                ("versions", json!("pytest 8.3.2")),
                ("familiar", json!("0.1.0")),
                ("watched", json!(["pyproject.toml", "conftest.py"])),
                ("gaps", json!(["no_junit"])),
                ("why", json!("the reporter plugin loaded")),
                ("ran_in", json!("laptop")),
            ]),
        ),
        (
            "fix_verdict",
            obj(&[
                ("criterion_id", json!("c1")),
                ("commit", json!("K3")),
                ("model", json!("claude-opus-4-6")),
                ("covers", json!("no")),
                ("reason", json!("the repr shows the new slot")),
                ("gap", json!("the repr the docs promise")),
                ("quotes", json!([{"file_path": "src/model.py", "line": 40, "text": "def __repr__(self):"}])),
            ]),
        ),
        (
            "existing_test_announced",
            obj(&[("file_path", json!("tests/test_old.py")), ("reason", json!("the person asked that leading slashes be kept"))]),
        ),
        (
            "existing_test_verdict",
            obj(&[
                ("file_path", json!("tests/test_old.py")),
                ("commit", json!("K3")),
                ("model", json!("claude-opus-4-6")),
                ("covers", json!("yes")),
                ("reason", json!("the ask names the slashes the old test said were trimmed")),
            ]),
        ),
        ("touching_test_derived", obj(&[("criterion_id", json!("c1")), ("file_path", json!("tests/test_m.py")), ("imports", json!("src/m.py"))])),
        (
            "places_listed",
            obj(&[
                ("criterion_id", json!("c1")),
                ("sites", json!([{"file": "src/m.py", "line": 8, "def": "def other(x):", "names": ["helper_name"]}])),
            ]),
        ),
        (
            "place_classified",
            obj(&[("criterion_id", json!("c1")), ("file", json!("src/m.py")), ("line", json!(8)), ("verdict", json!("not_affected")), ("reason", json!("it only passes the value on"))]),
        ),
        ("reading_alternatives", obj(&[("readings", json!(["the first way to read the ask", "the second way to read it"]))])),
        ("one_reading", obj(&[("why", json!("the ask names the one thing it changes"))])),
        // definition change (the owner's plan of 2026-10-23; owner approved 2026-10-23) [draft]
        (
            "trace_listed",
            obj(&[
                ("criterion_id", json!("c1")),
                ("run_id", json!("try-1")),
                ("sha", json!("K1")),
                ("red", json!(true)),
                ("path_size", json!(12)),
                ("ranker", json!("words")),
                ("query", json!("the export repeats every row")),
                ("top", json!([{"rank": 1, "file_path": "src/m.py", "symbol": "Writer.write_rows", "line_start": 10, "line_end": 30}])),
            ]),
        ),
        ("off_list_reason", obj(&[("criterion_id", json!("c1")), ("reason", json!("the header is rendered by a template the test never reaches"))])),
        ("cases_found", obj(&[("criterion_id", json!("c1")), ("outcome", json!("found")), ("cases", json!([{"case_id": "a1", "short": "a name with a space", "given": "the name Ada Lovelace", "expect": "hello Ada Lovelace", "why": "the code splits on spaces"}])), ("why", json!("")), ("model", json!("claude-sonnet-5-5")), ("tokens", json!(5320))])),
    ]
}

pub fn sample_for(kind: &str) -> Map<String, Value> {
    samples()
        .into_iter()
        .find(|(k, _)| *k == kind)
        .map(|(_, f)| f)
        .unwrap_or_else(|| panic!("no sample for {kind}"))
}
