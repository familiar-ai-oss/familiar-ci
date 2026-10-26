//! A pytest whose rootdir is not the project's root (pytest 7 with no ini file roots at the common
//! folder of the test files named on the command line; pytest 9 at the folder it runs from; here
//! `--rootdir=tests` stands for the first), so a recorder rooted there would see none of
//! the project's code and the try would list nothing. The binary names the project's root to the
//! recorder (`FAMILIAR_AI_TRACE_ROOT`), and the list holds the project's functions, relative to
//! that root (the owner's plan of 2026-10-23, change C) [draft names].
//!
//! Live, through the built binary; not run when no `python3` on PATH has pytest.

mod common;

use common::Project;
use familiar_ai_rules::trace;
use serde_json::json;

const SESSION: &str = "7b2e1d0c-11a3-4c5e-8f9a-2d3c4b5a6e7f";
const WORDS: &str = "the SAVE10 coupon takes nothing off: `discount_for('SAVE10', 10.0)` gives 10.0, it should give 9.0";
const CASES: &str = r#"[{"case_id":"n1","kind":"normal","given":"SAVE10 on 10.0","expect":"9.0"},{"case_id":"o1","kind":"opposite","given":"no coupon","expect":"10.0"}]"#;
const TEST: &str = "tests/test_pricing.py";
const PRICING: &str = "def discount_for(coupon, amount):\n    if coupon == \"SAVE10\":\n        return amount\n    return amount\n";
const RED_TEST: &str = "from shop.pricing import discount_for\n\n\ndef test_save10_takes_ten_percent_off():\n    assert discount_for(\"SAVE10\", 10.0) == 9.0\n";

fn has_pytest() -> bool {
    std::process::Command::new("python3")
        .args(["-c", "import pytest"])
        .stdin(std::process::Stdio::null())
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[test]
fn with_no_ini_the_try_lists_the_projects_functions_from_its_root() {
    if !has_pytest() {
        eprintln!("no python3 with pytest: not run");
        return;
    }
    // no pytest.ini, setup.cfg, tox.ini or pyproject.toml: the rootdir comes from the command's files
    let p = Project::builder("no-ini")
        .session(SESSION)
        .no_app()
        .file("shop/__init__.py", "")
        .file("shop/pricing.py", PRICING)
        .create();
    p.hook("UserPromptSubmit", json!({ "prompt": WORDS }));
    p.claim(&["intent_submitted", "--text", WORDS]);
    p.claim(&["behavior", "--behavior_id", "b1", "--text", "a coupon's discount"]);
    p.claim(&["promise", "--promise_id", "p1", "--behavior_id", "b1", "--accepts", "a coupon and an amount", "--gives_back", "the amount to pay", "--refuses", "nothing", "--cases", CASES]);
    p.claim(&["criterion", "--criterion_id", "c1", "--text", "SAVE10 takes 10% off", "--path", "full", "--behavior_id", "b1", "--promise_id", "p1"]);
    p.claim(&["test_planned", "--criterion_id", "c1", "--file_path", TEST, "--will", "create"]);
    p.claim(&["test_stack_detected", "--project_root", ".", "--test_stack", "pytest", "--test_command", &format!("python3 -m pytest -q --rootdir=tests {TEST}")]);
    p.claim(&["criterion_head", "--criterion_id", "c1"]);
    p.claim(&["criterion_assigned", "--criterion_id", "c1"]);
    p.write(TEST, RED_TEST);

    let (code, out, err) = p.run(&["try"], "");
    assert_eq!(code, 0, "{out}{err}");
    let facts = p.facts_of(trace::LISTED);
    assert_eq!(facts.len(), 1, "{facts:?}");
    let f = &facts[0];
    assert_eq!(f["red"], true, "{f}");
    let top = f["top"].as_array().cloned().unwrap_or_default();
    assert!(!top.is_empty(), "the project's code is listed though pytest's rootdir is tests/: {f}\n{out}");
    assert_eq!(top[0]["symbol"], "discount_for", "{f}");
    assert_eq!(top[0]["file_path"], "shop/pricing.py", "relative to the project's root: {f}");
}
