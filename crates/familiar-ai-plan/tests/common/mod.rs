//! Fixtures shared by the plan tests; each test binary uses a subset, so dead code is allowed.
#![allow(dead_code)]

use familiar_ai_plan::{Claim, PLAN_KINDS};
use serde_json::{json, Map, Value};

pub fn fields(pairs: &[(&str, Value)]) -> Map<String, Value> {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.clone()))
        .collect()
}

/// One claim of every allowed kind, valid against the facts crate's forms.
pub fn one_of_each() -> Vec<Claim> {
    let spec: Vec<(&str, Map<String, Value>)> = vec![
        (
            "intent_submitted",
            fields(&[("text", json!("ship login")), ("sha", json!("abc123"))]),
        ),
        (
            "behavior",
            fields(&[("behavior_id", json!("b1")), ("text", json!("logs in"))]),
        ),
        (
            "promise",
            fields(&[
                ("promise_id", json!("p1")),
                ("behavior_id", json!("b1")),
                ("accepts", json!("a")),
                ("gives_back", json!("g")),
                ("refuses", json!("r")),
            ]),
        ),
        (
            "criterion",
            fields(&[
                ("criterion_id", json!("c1")),
                ("text", json!("full one")),
                ("path", json!("full")),
                ("behavior_id", json!("b1")),
                ("promise_id", json!("p1")),
            ]),
        ),
        (
            "criterion",
            fields(&[
                ("criterion_id", json!("c2")),
                ("text", json!("light one")),
                ("path", json!("light")),
            ]),
        ),
        (
            "place_chosen",
            fields(&[
                ("criterion_id", json!("c1")),
                (
                    "places",
                    json!([{"file_path": "src/login.py", "symbol": "check_password",
                            "line_start": 40, "line_end": 62}]),
                ),
            ]),
        ),
        (
            "new_work_spot_chosen",
            fields(&[
                ("criterion_id", json!("c2")),
                ("file_path", json!("src/mail.py")),
                ("symbol", json!("reset_link")),
                ("beside", json!("send_mail")),
            ]),
        ),
        ("new_work_no_spot", fields(&[("criterion_id", json!("c3")), ("integration_point", json!("src/app.py:main"))])),
        ("criterion_head", fields(&[("criterion_id", json!("c1"))])),
        (
            "criterion_strung_on_head",
            fields(&[("criterion_id", json!("c2")), ("head", json!("c1"))]),
        ),
        (
            "test_stack_detected",
            fields(&[
                ("codebase_id", json!("cb1")),
                ("project_root", json!("src/")),
                ("test_stack", json!("python_pytest")),
                ("test_command", json!("pytest")),
            ]),
        ),
        (
            "test_planned",
            fields(&[
                ("criterion_id", json!("c1")),
                ("file_path", json!("tests/test_c1.py")),
                ("will", json!("create")),
            ]),
        ),
        (
            "touching_test_declared",
            fields(&[
                ("criterion_id", json!("c1")),
                ("file_path", json!("tests/test_login_old.py")),
                ("test_name", json!("t_old")),
            ]),
        ),
        (
            "test_written",
            fields(&[
                ("criterion_id", json!("c1")),
                ("file_path", json!("tests/test_c1.py")),
                ("test_name", json!("t1")),
            ]),
        ),
        (
            "anchor_sha_matched",
            fields(&[
                ("criterion_id", json!("c1")),
                ("file_path", json!("src/login.py")),
                ("symbol", json!("check_password")),
                ("line_start", json!(40)),
                ("line_end", json!(62)),
                ("sha", json!("abc123")),
                ("lines_hash", json!("L0")),
                ("matched", json!(true)),
                ("checked_at", json!("planning")),
            ]),
        ),
        (
            "fingerprint_stamped",
            fields(&[
                ("criterion_id", json!("c1")),
                ("commit", json!("T1")),
                ("proof_not_tampered", json!("S1")),
                ("ran_in", json!("laptop")),
            ]),
        ),
        (
            "criterion_assigned",
            fields(&[
                ("criterion_id", json!("c1")),
                ("agent_conversation_id", json!("a1")),
            ]),
        ),
        (
            "test_change_announced",
            fields(&[
                ("criterion_id", json!("c1")),
                ("reason", json!("spec moved")),
            ]),
        ),
        (
            "fix_location_category",
            fields(&[
                ("criterion_id", json!("c1")),
                ("category", json!("dependency")),
                ("where", json!("the library the anchor calls")),
            ]),
        ),
        // the case-finder's finding and the person's answer on the cases card (the owner,
        // 2026-10-25: "Mirror them") [draft names]
        (
            "cases_found",
            fields(&[
                ("criterion_id", json!("c1")),
                ("outcome", json!("found")),
                (
                    "cases",
                    json!([{"case_id": "a1", "short": "a locked account is refused",
                            "given": "a locked account and the right password",
                            "expect": "refused", "why": "the code checks the lock first"}]),
                ),
                ("why", json!("")),
                ("model", json!("sonnet")),
                ("tokens", json!(1234)),
            ]),
        ),
        (
            "hold_answered",
            fields(&[
                ("criterion_id", json!("c1")),
                ("hold", json!("cases_unconfirmed")),
                ("choice", json!("add_all")),
                ("typed", json!("")),
            ]),
        ),
    ];
    assert_eq!(
        spec.iter()
            .map(|(k, _)| *k)
            .collect::<std::collections::BTreeSet<_>>(),
        PLAN_KINDS.iter().copied().collect(),
        "the fixture must cover every allowed kind"
    );
    spec.into_iter()
        .map(|(k, f)| Claim::new(k, f).unwrap_or_else(|e| panic!("{k}: {e}")))
        .collect()
}

/// Returns a scratch directory under the OS temp dir, unique per call.
pub fn unique_tmp_dir() -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "familiar-ai-plan-test-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    std::fs::create_dir_all(&dir).expect("tmp dir");
    dir
}
