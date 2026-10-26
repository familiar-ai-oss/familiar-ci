//! `familiar-ai try` [draft name] (the owner's plan of 2026-10-23, change C): a pytest part's
//! planned test, written in the working tree and not committed, runs under the kit with the call
//! recorder on. When a test of the part's planned files fails, the binary lists the functions the
//! failing tests ran, ranked by word match with the person's words (`trace_listed`, the top 25),
//! keeps the whole ranked path in `.familiar/runs/<run_id>.trace.json`, and tells the agent the
//! top 12 and the next step: pick the place from the list. A passing try lists nothing and says
//! the test does not show the problem yet. The try is a look, never a run of the record: the seal
//! is still the tests-first commit's run. `try --list all` prints the stored list without running
//! again. `status` shows the try's top 12.
//!
//! Live, through the built binary on a tiny real pytest project; not run, like the kit's tests,
//! when no `python3` on PATH has pytest.

mod common;

use common::Project;
use familiar_ai_facts::Fact;
use familiar_ai_rules::{criterion, plan, trace, View};
use serde_json::{json, Value};

const SESSION: &str = "5a1e0c7d-77a2-4f0e-9d6b-1f0c2a7e9b31";
/// The person's words: an example call (so the one question is not asked) naming the coupon.
const WORDS: &str = "the SAVE10 coupon takes nothing off: `discount_for('SAVE10', 10.0)` gives 10.0, it should give 9.0";
const CASES: &str = r#"[{"case_id":"n1","kind":"normal","given":"SAVE10 on 10.0","expect":"9.0"},{"case_id":"o1","kind":"opposite","given":"no coupon","expect":"10.0"}]"#;
const TEST: &str = "tests/test_pricing.py";

const PRICING: &str = "def subtotal(items):\n    total = 0.0\n    for _, price in items:\n        total += price\n    return total\n\n\ndef discount_for(coupon, amount):\n    if coupon == \"SAVE10\":\n        return amount\n    return amount\n\n\ndef round_money(value):\n    return round(value, 2)\n";
const CART: &str = "from shop.pricing import discount_for, round_money, subtotal\n\n\ndef checkout(items, coupon=None):\n    amount = subtotal(items)\n    if coupon:\n        amount = discount_for(coupon, amount)\n    return round_money(amount)\n\n\ndef receipt(items):\n    lines = []\n    for name, _ in items:\n        lines.append(name)\n    return \"\\n\".join(lines)\n";
/// The planned test, failing: the coupon takes nothing off.
const RED_TEST: &str = "from shop.cart import checkout\n\n\ndef test_save10_takes_ten_percent_off():\n    assert checkout([(\"pen\", 10.0)], \"SAVE10\") == 9.0\n";
/// The planned test, passing: it does not show the problem.
const GREEN_TEST: &str = "from shop.cart import checkout\n\n\ndef test_no_coupon_keeps_the_price():\n    assert checkout([(\"pen\", 10.0)]) == 10.0\n";

/// Whether `python3` on PATH has pytest; the tests are not run without it.
fn has_pytest() -> bool {
    std::process::Command::new("python3")
        .args(["-c", "import pytest"])
        .stdin(std::process::Stdio::null())
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// A pytest project (an ini at its root, so pytest's rootdir is the root) planned up to its test:
/// lines 1–3 and 6–10, no place yet.
fn project(name: &str) -> Project {
    let p = Project::builder(name)
        .session(SESSION)
        .no_app()
        .file("pytest.ini", "[pytest]\n")
        .file("conftest.py", "")
        .file("shop/__init__.py", "")
        .file("shop/pricing.py", PRICING)
        .file("shop/cart.py", CART)
        .create();
    p.hook("UserPromptSubmit", json!({ "prompt": WORDS }));
    p.claim(&["intent_submitted", "--text", WORDS]);
    p.claim(&["behavior", "--behavior_id", "b1", "--text", "a coupon's discount"]);
    p.claim(&["promise", "--promise_id", "p1", "--behavior_id", "b1", "--accepts", "items and a coupon", "--gives_back", "the price to pay", "--refuses", "nothing", "--cases", CASES]);
    p.claim(&["criterion", "--criterion_id", "c1", "--text", "SAVE10 takes 10% off", "--path", "full", "--behavior_id", "b1", "--promise_id", "p1"]);
    p.claim(&["test_planned", "--criterion_id", "c1", "--file_path", TEST, "--will", "create"]);
    p.claim(&["test_stack_detected", "--project_root", ".", "--test_stack", "pytest", "--test_command", "python3 -m pytest -q"]);
    p.claim(&["criterion_head", "--criterion_id", "c1"]);
    p.claim(&["criterion_assigned", "--criterion_id", "c1"]);
    p
}

/// `familiar-ai try` with `args`: (exit, stdout, stderr).
fn try_it(p: &Project, args: &[&str]) -> (i32, String, String) {
    let mut argv = vec!["try"];
    argv.extend_from_slice(args);
    p.run(&argv, "")
}

fn listed(p: &Project) -> Vec<Value> {
    p.facts_of(trace::LISTED)
}

fn with_view<T>(p: &Project, f: impl FnOnce(&View) -> T) -> T {
    let log: Vec<Fact> = p.facts();
    let v = View::new(&log);
    f(&v)
}

/// The deny reason of a hook answer, or `None` when it allows.
fn denied(out: &Value) -> Option<String> {
    (out["hookSpecificOutput"]["permissionDecision"] == "deny").then(|| out["hookSpecificOutput"]["permissionDecisionReason"].as_str().unwrap_or("").to_string())
}

#[test]
fn a_red_try_lists_the_function_the_request_names_first_and_keeps_the_whole_path() {
    if !has_pytest() {
        eprintln!("no python3 with pytest: not run");
        return;
    }
    let p = project("red");
    // the agent's shell may run the try while the code still waits, and the planned test is written
    assert_eq!(denied(&p.bash("PreToolUse", "familiar-ai try")), None, "the try runs from the agent's shell");
    assert_eq!(denied(&p.tool("PreToolUse", "Write", json!({ "file_path": p.path(TEST).to_string_lossy(), "content": RED_TEST }))), None, "the planned test may be written");
    let code_refused = denied(&p.tool("PreToolUse", "Edit", json!({ "file_path": p.path("shop/pricing.py").to_string_lossy(), "old_string": "a", "new_string": "b" }))).expect("the code waits");
    assert!(code_refused.contains("familiar-ai try"), "{code_refused}");
    p.write(TEST, RED_TEST);
    let head = p.head();

    let (code, out, err) = try_it(&p, &[]);
    assert_eq!(code, 0, "{out}{err}");
    let facts = listed(&p);
    assert_eq!(facts.len(), 1, "one listing: {facts:?}");
    let f = &facts[0];
    assert_eq!(f["criterion_id"], "c1");
    assert_eq!(f["red"], true);
    assert_eq!(f["sha"], head.as_str(), "the head the working tree sits on");
    assert_eq!(f["ranker"], "words");
    assert!(f["query"].as_str().unwrap_or("").contains("SAVE10"), "the person's words are the query: {f}");
    let top = f["top"].as_array().cloned().unwrap_or_default();
    assert_eq!(top[0]["symbol"], "discount_for", "the function the words name first: {f}");
    assert_eq!(top[0]["file_path"], "shop/pricing.py");
    assert_eq!((top[0]["rank"].as_u64(), top[0]["line_start"].as_u64(), top[0]["line_end"].as_u64()), (Some(1), Some(8), Some(11)), "{f}");
    let symbols: Vec<&str> = top.iter().filter_map(|t| t["symbol"].as_str()).collect();
    for s in ["checkout", "subtotal", "round_money"] {
        assert!(symbols.contains(&s), "{s} ran: {symbols:?}");
    }
    assert!(!symbols.contains(&"receipt"), "receipt never ran: {symbols:?}");
    assert_eq!(f["path_size"].as_u64(), Some(top.len() as u64), "a small path is listed whole: {f}");

    // the whole ranked path is kept beside the runs, not committed
    let run_id = f["run_id"].as_str().expect("the try's id");
    let kept = p.read(&format!(".familiar/runs/{run_id}.trace.json"));
    let kept: Value = serde_json::from_str(&kept).expect("the trace file is JSON");
    let ranked = kept["ranked"].as_array().cloned().unwrap_or_default();
    assert_eq!(ranked.len() as u64, f["path_size"].as_u64().unwrap_or(0), "{kept}");
    assert_eq!(ranked[0]["symbol"], "discount_for");
    assert_eq!(p.git(&["status", "--porcelain", "--", ".familiar/runs"]).trim(), "", "the trace file is git-ignored");

    // what the agent reads: red, the list as `rank. file:lines symbol`, the next step
    assert!(out.contains("red"), "{out}");
    assert!(out.contains("1. shop/pricing.py:8-11 discount_for"), "{out}");
    assert!(out.contains("familiar-ai claim place_chosen"), "{out}");
    assert!(out.contains("off_list_reason"), "{out}");
    assert!(with_view(&p, |v| trace::try_hold(v, "c1").is_none()), "a red try stands");

    // status shows the try and its list
    let status = p.status();
    assert!(status.contains("1. shop/pricing.py:8-11 discount_for"), "{status}");
    let c = p.criterion_json("c1");
    assert_eq!(c["try"]["red"], true, "{c}");
    assert_eq!(c["try"]["top"][0]["symbol"], "discount_for", "{c}");
}

#[test]
fn a_passing_try_lists_nothing_and_the_part_holds_try_not_red() {
    if !has_pytest() {
        eprintln!("no python3 with pytest: not run");
        return;
    }
    let p = project("passing");
    p.write(TEST, GREEN_TEST);
    let (code, out, err) = try_it(&p, &[]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("the test passes: it doesn't show the problem yet; change the test and try again"), "{out}");
    let facts = listed(&p);
    assert_eq!(facts.len(), 1, "{facts:?}");
    assert_eq!(facts[0]["red"], false);
    assert_eq!(facts[0]["top"], json!([]));
    assert_eq!(with_view(&p, |v| trace::try_hold(v, "c1")), Some(trace::TRY_NOT_RED));
    let c = p.criterion_json("c1");
    assert_eq!(c["try"]["red"], false, "{c}");
}

#[test]
fn after_a_red_try_a_listed_place_holds_line_4_and_an_unlisted_one_waits_for_its_reason() {
    if !has_pytest() {
        eprintln!("no python3 with pytest: not run");
        return;
    }
    let p = project("pick");
    p.write(TEST, RED_TEST);
    // before any try, a place does not hold line 4
    p.claim(&["place_chosen", "--criterion_id", "c1", "--places", r#"[{"file_path":"shop/pricing.py","symbol":"discount_for","line_start":8,"line_end":11}]"#]);
    assert!(!with_view(&p, |v| plan::line_4_of(v, "c1")), "no try yet");
    let (code, out, err) = try_it(&p, &[]);
    assert_eq!(code, 0, "{out}{err}");
    // a try newer than the place: the listed place holds
    assert!(with_view(&p, |v| plan::line_4_of(v, "c1")), "discount_for is on the list");
    // a place the failing test never ran waits for the agent's reason
    p.claim(&["place_chosen", "--criterion_id", "c1", "--places", r#"[{"file_path":"shop/cart.py","symbol":"receipt","line_start":11,"line_end":15}]"#]);
    assert!(!with_view(&p, |v| plan::line_4_of(v, "c1")), "receipt is off the list");
    p.claim(&["off_list_reason", "--criterion_id", "c1", "--reason", "the receipt prints the discount the checkout never shows"]);
    assert!(with_view(&p, |v| plan::line_4_of(v, "c1")), "the reason lets it stand");
}

#[test]
fn the_try_is_invisible_to_the_seal_the_tests_first_commits_run_is_still_the_seal_run() {
    if !has_pytest() {
        eprintln!("no python3 with pytest: not run");
        return;
    }
    let p = project("seal");
    p.write(TEST, RED_TEST);
    let (code, out, err) = try_it(&p, &[]);
    assert_eq!(code, 0, "{out}{err}");
    let try_id = listed(&p)[0]["run_id"].as_str().unwrap_or("").to_string();
    // no run of the record is the try's, and none is the part's yet (the reader's dry run, the
    // project's own, may be going)
    for kind in ["run_reserved", "run_started", "run_ended", "test_log_translation", "pass_please_pass"] {
        let of_try: Vec<Value> = p.facts_of(kind).into_iter().filter(|f| f["run_id"].as_str() == Some(try_id.as_str()) || f["criterion_id"] == "c1").collect();
        assert!(of_try.is_empty(), "the try wrote {kind}: {of_try:?}");
    }
    p.claim(&["place_chosen", "--criterion_id", "c1", "--places", r#"[{"file_path":"shop/pricing.py","symbol":"discount_for","line_start":8,"line_end":11}]"#]);
    let out = p.claim(&["fix_location_category", "--criterion_id", "c1", "--category", "this_repository", "--where", ""]);
    assert!(out.contains("the plan is confirmed"), "{out}");
    common::classify_places(&p);
    p.commit("c1: tests");
    let tf = p.head();
    assert_eq!(p.state_name("c1"), "sealed", "{}", p.status());
    let seal = with_view(&p, |v| criterion::sealed_at(v, "c1", criterion::Place::Laptop).map(|f| (f.fields.get("commit").cloned(), f.fields.get("run_id").cloned())));
    let (commit, run) = seal.expect("sealed");
    assert_eq!(commit, Some(Value::from(tf.as_str())), "the tests-first commit's run is the seal");
    assert_ne!(run, Some(Value::from(try_id.as_str())), "the try is not the seal run");
    assert!(p.facts_of("run_reserved").iter().all(|r| r["run_id"].as_str() != Some(try_id.as_str())), "the try is never a reserved run");
}

#[test]
fn try_list_all_prints_the_stored_list_without_running_again() {
    if !has_pytest() {
        eprintln!("no python3 with pytest: not run");
        return;
    }
    let p = project("list-all");
    p.write(TEST, RED_TEST);
    let (code, out, err) = try_it(&p, &[]);
    assert_eq!(code, 0, "{out}{err}");
    let run_id = listed(&p)[0]["run_id"].as_str().unwrap_or("").to_string();
    let kept: Value = serde_json::from_str(&p.read(&format!(".familiar/runs/{run_id}.trace.json"))).expect("the trace file");
    // the test changed since: a run would pass; the stored list is printed as it was
    p.write(TEST, GREEN_TEST);
    let (code, all, err) = try_it(&p, &["--list", "all"]);
    assert_eq!(code, 0, "{all}{err}");
    assert_eq!(listed(&p).len(), 1, "no new try");
    for r in kept["ranked"].as_array().cloned().unwrap_or_default() {
        let line = format!("{}. {}:{}-{} {}", r["rank"], r["file_path"].as_str().unwrap_or(""), r["line_start"], r["line_end"], r["symbol"].as_str().unwrap_or(""));
        assert!(all.contains(&line), "{line} in {all}");
    }
}

#[test]
fn a_try_is_refused_in_one_line_before_the_test_may_be_written_and_for_a_part_with_no_recorder() {
    let p = Project::builder("refused").session(SESSION).no_app().file("shop/__init__.py", "").create();
    p.hook("UserPromptSubmit", json!({ "prompt": WORDS }));
    p.claim(&["intent_submitted", "--text", WORDS]);
    p.claim(&["behavior", "--behavior_id", "b1", "--text", "a coupon's discount"]);
    p.claim(&["promise", "--promise_id", "p1", "--behavior_id", "b1", "--accepts", "items and a coupon", "--gives_back", "the price to pay", "--refuses", "nothing", "--cases", CASES]);
    p.claim(&["criterion", "--criterion_id", "c1", "--text", "SAVE10 takes 10% off", "--path", "full", "--behavior_id", "b1", "--promise_id", "p1"]);
    p.claim(&["test_planned", "--criterion_id", "c1", "--file_path", TEST, "--will", "create"]);
    p.claim(&["test_stack_detected", "--project_root", ".", "--test_stack", "pytest", "--test_command", "python3 -m pytest -q"]);
    // not assigned yet: the test may not be written
    let (code, out, err) = try_it(&p, &["--criterion", "c1"]);
    assert_ne!(code, 0, "{out}");
    assert_eq!(err.trim().lines().count(), 1, "one line: {err}");
    assert!(err.contains("c1"), "{err}");
    assert!(listed(&p).is_empty());
    // a part whose tests do not run under pytest has no recorder
    let q = Project::builder("no-recorder").session(SESSION).create();
    q.hook("UserPromptSubmit", json!({ "prompt": "running `sh src/app.sh` prints hi, it should print hello" }));
    q.claim(&["intent_submitted", "--text", "running `sh src/app.sh` prints hi, it should print hello"]);
    q.claim(&["behavior", "--behavior_id", "b1", "--text", "the greeting"]);
    q.claim(&["promise", "--promise_id", "p1", "--behavior_id", "b1", "--accepts", "a run", "--gives_back", "hello", "--refuses", "nothing", "--cases", CASES]);
    q.claim(&["criterion", "--criterion_id", "c1", "--text", "the app says hello", "--path", "full", "--behavior_id", "b1", "--promise_id", "p1"]);
    q.claim(&["test_planned", "--criterion_id", "c1", "--file_path", "tests/test_app.sh", "--will", "create"]);
    q.claim(&["test_stack_detected", "--project_root", ".", "--test_stack", "sh", "--test_command", "sh tests/test_app.sh"]);
    q.claim(&["criterion_head", "--criterion_id", "c1"]);
    q.claim(&["criterion_assigned", "--criterion_id", "c1"]);
    let (code, out, err) = try_it(&q, &[]);
    assert_ne!(code, 0, "{out}");
    assert_eq!(err.trim().lines().count(), 1, "one line: {err}");
    assert!(err.contains("pytest"), "{err}");
    assert!(q.facts_of(trace::LISTED).is_empty());
}
