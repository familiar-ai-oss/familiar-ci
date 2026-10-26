//! Places: where else the behavior lives, listed before the fix (the owner's plan of 2026-10-21,
//! step 3) [draft names]. Once a part's place is chosen and its lines checked, the binary searches
//! the planning commit for the sites that name what the place names (its symbol, and the
//! distinctive identifiers on its lines), outside the place's own lines, the line defining it and
//! every test, and records them as `places_listed`, ranked (the place's file first) and capped.
//! The agent classifies each with `place_classified`: `applies` (a contract case of kind `place`,
//! which needs its own test) or `not_affected` with a one-line reason. Until every listed site is
//! classified, the commit is refused (`places_unclassified`, the agent's hold, never a card); a
//! listing with no site holds nothing. At the seal the judge reads each `not_affected` reason
//! against the code shown, and a reason it rejects is a "no" naming the site. Runs the real binary
//! with a fake `claude` first on PATH; no model is called.

mod common;

use common::Project;
use serde_json::{json, Value};

const SESSION: &str = "84be6339-0cc7-40ec-91b9-cae44365931b";

const CALC: &str = "\
def apply_discount(order, rate):
    total = order.subtotal * rate
    return round_cents(total)


def apply_discount_flat(order):
    total = order.subtotal
    return round_cents(total)
";

const CHECKOUT: &str = "\
from pkg import calc


def pay(order):
    return calc.apply_discount(order, 0.9)
";

fn said(answer: Value) -> String {
    json!({ "type": "result", "is_error": false, "result": answer.to_string(), "modelUsage": { "claude-opus-4-6": {} } }).to_string()
}

const CASES: &str = r#"[{"case_id":"n1","kind":"normal","given":"an order and a rate","expect":"the discounted total"},{"case_id":"o1","kind":"opposite","given":"a rate of one","expect":"the total as before"}]"#;

/// A project planned to c1, anchored on `apply_discount` (lines 1–3: the planning check finds the
/// symbol on its lines), with a same-file look-alike, a caller in the same folder and tests that
/// name both.
fn project(name: &str) -> Project {
    let b = Project::builder(name)
        .session(SESSION)
        .no_app()
        .file("src/pkg/calc.py", CALC)
        .file("src/pkg/checkout.py", CHECKOUT)
        .file("src/pkg/test_money.py", "def test_cents():\n    assert round_cents(1.234) == 1.23\n")
        .file("tests/test_calc.py", "from pkg.calc import apply_discount, round_cents\n\n\ndef test_discount():\n    assert apply_discount\n");
    let d = b.dir().join("bin");
    let fake = format!("#!/bin/sh\nd='{d}'\nprintf '%s\\n' \"$*\" >> \"$d/calls\"\ncat > \"$d/prompt\"\ncat \"$d/answer\"\n", d = d.display());
    let b = b.bin("claude", &fake);
    std::fs::write(d.join("answer"), said(json!({ "answers_intent": true, "gap": "", "reason": "r" }))).expect("answer");
    let p = b.env_remove("FAMILIAR_AI_JUDGE").create();
    p.claim(&["intent_submitted", "--for", "0", "--text", "apply the discount"]);
    p.claim(&["behavior", "--behavior_id", "b1", "--text", "the discount"]);
    p.claim(&["promise", "--promise_id", "p1", "--behavior_id", "b1", "--accepts", "an order and a rate", "--gives_back", "the discounted total", "--refuses", "nothing", "--cases", CASES]);
    p.claim(&["criterion", "--criterion_id", "c1", "--text", "the discount applies", "--path", "full", "--behavior_id", "b1", "--promise_id", "p1"]);
    p.cases_kept("c1"); // the cases card passed (plan cases-card, 2026-10-25) [draft names]
    p.claim(&["place_chosen", "--criterion_id", "c1", "--places", r#"[{"file_path":"src/pkg/calc.py","symbol":"apply_discount","line_start":1,"line_end":3}]"#]);
    p.claim(&["test_planned", "--criterion_id", "c1", "--file_path", "tests/test_c1.sh", "--will", "create"]);
    p.claim(&["test_stack_detected", "--project_root", ".", "--test_stack", "sh", "--test_command", "sh tests/test_c1.sh"]);
    p.claim(&["criterion_head", "--criterion_id", "c1"]);
    p.claim(&["criterion_assigned", "--criterion_id", "c1"]);
    p.claim(&["fix_location_category", "--criterion_id", "c1", "--category", "this_repository", "--where", ""]);
    p.hook("PostModelSwitch", json!({ "from_model": "claude-opus-4-1", "to_model": "claude-haiku-4-5-20251001" }));
    p
}

fn denied(out: &Value) -> Option<String> {
    (out["hookSpecificOutput"]["permissionDecision"] == "deny").then(|| out["hookSpecificOutput"]["permissionDecisionReason"].as_str().unwrap_or("").to_string())
}

fn commit_asked(p: &Project) -> Option<String> {
    denied(&p.bash("PreToolUse", "git commit -m 'c1: tests'"))
}

fn classify(p: &Project, file: &str, line: &str, verdict: &str, reason: &str) -> (i32, String, String) {
    p.try_claim(&["place_classified", "--criterion_id", "c1", "--file", file, "--line", line, "--verdict", verdict, "--reason", reason])
}

#[test]
fn the_places_are_listed_before_any_fix_outside_the_places_own_lines_and_the_tests() {
    let p = project("places-listed");
    let listed = p.facts_of("places_listed");
    assert_eq!(listed.len(), 1, "one listing, written once the place's lines were checked: {listed:?}");
    assert_eq!(
        listed[0]["sites"],
        json!([
            { "file": "src/pkg/calc.py", "line": 8, "def": "def apply_discount_flat(order):", "names": ["round_cents"] },
            { "file": "src/pkg/checkout.py", "line": 5, "def": "def pay(order):", "names": ["apply_discount"] }
        ]),
        "the same file's look-alike first, then the caller beside it; never the place's own lines 1–3 or a test"
    );
    assert!(p.facts_of("commit_seen").iter().all(|f| f["touches_code"] != true), "before any fix");
    // a later hook call writes it again only for a new place
    p.hook("UserPromptSubmit", json!({ "prompt": "go on" }));
    assert_eq!(p.facts_of("places_listed").len(), 1);
}

#[test]
fn the_commit_is_refused_until_every_listed_site_is_classified() {
    let p = project("places-hold");
    assert_eq!(p.state_name("c1"), "held", "{}", p.status());
    let why = commit_asked(&p).expect("the tests-first commit is refused");
    assert!(why.contains("places unclassified"), "{why}");
    assert!(why.contains("src/pkg/calc.py:8 (in def apply_discount_flat(order):) names round_cents"), "{why}");
    assert!(why.contains("src/pkg/checkout.py:5 (in def pay(order):) names apply_discount"), "{why}");
    assert!(why.contains("familiar-ai claim place_classified --criterion_id c1 --file src/pkg/calc.py --line 8 --verdict"), "{why}");

    // not_affected needs a reason; a site never listed is refused, naming the listed ones
    let (code, _, err) = classify(&p, "src/pkg/checkout.py", "5", "not_affected", " ");
    assert_eq!(code, 1, "{err}");
    assert!(err.contains("not_affected needs its reason"), "{err}");
    // the example's reason pasted is no reason
    let example = familiar_ai::teach::form::PLACE_REASON_EXAMPLE;
    assert!(why.contains(example), "{why}");
    let (code, _, err) = classify(&p, "src/pkg/checkout.py", "5", "not_affected", example);
    assert_eq!(code, 1, "{err}");
    assert!(err.contains("is the example"), "{err}");
    let (code, _, err) = classify(&p, "src/pkg/checkout.py", "1", "applies", "");
    assert_eq!(code, 1, "{err}");
    assert!(err.contains("src/pkg/checkout.py:1 is not a listed site of c1") && err.contains("src/pkg/calc.py:8"), "{err}");

    let (code, _, err) = classify(&p, "src/pkg/calc.py", "8", "applies", "the flat discount rounds its total the same way");
    assert_eq!(code, 0, "{err}");
    let why = commit_asked(&p).expect("one site is left");
    assert!(!why.contains("src/pkg/calc.py:8 (in") && why.contains("src/pkg/checkout.py:5"), "{why}");
    let (code, _, err) = classify(&p, "src/pkg/checkout.py", "5", "not_affected", "it passes the order to apply_discount and returns its answer unchanged");
    assert_eq!(code, 0, "{err}");
    assert_eq!(commit_asked(&p), None);
    assert_eq!(p.state_name("c1"), "testing", "{}", p.status());
}

#[test]
fn with_no_site_listed_nothing_holds() {
    let p = Project::new("places-none");
    p.claim(&["intent_submitted", "--for", "0", "--text", "greet with hello"]);
    p.claim(&["behavior", "--behavior_id", "b1", "--text", "the greeting"]);
    p.claim(&["promise", "--promise_id", "p1", "--behavior_id", "b1", "--accepts", "a name", "--gives_back", "a greeting", "--refuses", "nothing", "--cases", CASES]);
    p.claim(&["criterion", "--criterion_id", "c1", "--text", "greet says hello", "--path", "full", "--behavior_id", "b1", "--promise_id", "p1"]);
    p.cases_kept("c1"); // the cases card passed (plan cases-card, 2026-10-25) [draft names]
    p.claim(&["place_chosen", "--criterion_id", "c1", "--places", r#"[{"file_path":"src/app.sh","symbol":"greet","line_start":1,"line_end":3}]"#]);
    p.claim(&["test_planned", "--criterion_id", "c1", "--file_path", "tests/test_c1.sh", "--will", "create"]);
    p.claim(&["test_stack_detected", "--project_root", ".", "--test_stack", "sh", "--test_command", "sh tests/test_c1.sh"]);
    p.claim(&["criterion_head", "--criterion_id", "c1"]);
    p.claim(&["criterion_assigned", "--criterion_id", "c1"]);
    p.claim(&["fix_location_category", "--criterion_id", "c1", "--category", "this_repository", "--where", ""]);
    assert_eq!(p.facts_of("places_listed").iter().map(|f| f["sites"].clone()).collect::<Vec<_>>(), vec![json!([])]);
    assert_eq!(p.state_name("c1"), "testing", "{}", p.status());
    assert_eq!(commit_asked(&p), None);
}

/// Every site classified, the tests committed alone and seen failing: the judge reads the seal.
fn sealed(p: &Project, answer: Value) -> String {
    assert_eq!(classify(p, "src/pkg/calc.py", "8", "applies", "the flat discount rounds its total the same way").0, 0);
    assert_eq!(classify(p, "src/pkg/checkout.py", "5", "not_affected", "it passes the order to apply_discount and returns its answer unchanged").0, 0);
    std::fs::write(p.dir.join("bin/answer"), said(answer)).expect("answer");
    p.write("tests/test_c1.sh", "grep -q 'rate, flat' src/pkg/calc.py\n");
    p.commit("c1: tests");
    std::fs::read_to_string(p.dir.join("bin/prompt")).unwrap_or_default()
}

fn cases(place1: &str) -> Value {
    json!([{ "case_id": "n1", "test": "test_rate", "ok": true }, { "case_id": "o1", "test": "test_rate_one", "ok": true }, { "case_id": "place1", "test": place1, "ok": place1 != "none" }])
}

#[test]
fn the_judge_reads_each_not_affected_reason_against_the_code_and_a_rejected_one_is_a_no() {
    let p = project("places-judged-no");
    let answer = json!({ "answers_intent": true, "gap": "", "reason": "r", "cases": cases("test_flat"), "places": [{ "file": "src/pkg/checkout.py", "line": 5, "ok": false }] });
    let prompt = sealed(&p, answer);
    assert!(
        prompt.contains(
            "PLACES THE AGENT JUDGED NOT AFFECTED (sites the binary listed before the fix that name what the planned place names; for each, the agent's reason the behavior THE CONTRACT promises does not reach it, and its code at the tests-first commit)\n\
             src/pkg/checkout.py:5, def pay(order):, reason: it passes the order to apply_discount and returns its answer unchanged\n    \
             4| def pay(order):\n    \
             5|     return calc.apply_discount(order, 0.9)\n"
        ),
        "{prompt}"
    );
    assert!(prompt.contains("case place1 (place): given the same call through src/pkg/calc.py:8 (def apply_discount_flat(order):) → expect the flat discount rounds its total the same way"), "{prompt}");
    assert!(prompt.contains("4. For each place under PLACES THE AGENT JUDGED NOT AFFECTED, read its code shown:"), "{prompt}");
    let calls = std::fs::read_to_string(p.dir.join("bin/calls")).unwrap_or_default();
    assert!(calls.contains("\"places\""), "the answer's schema asks for the places: {calls}");
    let v = p.facts_of("judge_verdict");
    let last = v.last().expect("a verdict");
    assert_eq!(last["answers_intent"], "no", "{last}");
    let gap = last["gap"].as_str().unwrap_or("");
    assert!(gap.contains("place src/pkg/checkout.py:5 (def pay(order):): the reason it is not affected does not hold for its code"), "{gap}");
    assert_eq!(last["places"], json!([{ "file": "src/pkg/checkout.py", "line": 5, "ok": false }]));
}

#[test]
fn a_sound_reason_and_a_tested_place_case_are_a_yes() {
    let p = project("places-judged-yes");
    let answer = json!({ "answers_intent": true, "gap": "", "reason": "r", "cases": cases("test_flat"), "places": [{ "file": "src/pkg/checkout.py", "line": 5, "ok": true }] });
    sealed(&p, answer);
    let v = p.facts_of("judge_verdict");
    assert_eq!(v.last().map(|f| f["answers_intent"].clone()), Some(json!("yes")), "{v:?}");
    // the place case named none is a no, naming it
    let p = project("places-case-none");
    let answer = json!({ "answers_intent": true, "gap": "", "reason": "r", "cases": cases("none"), "places": [{ "file": "src/pkg/checkout.py", "line": 5, "ok": true }] });
    sealed(&p, answer);
    let v = p.facts_of("judge_verdict");
    let last = v.last().expect("a verdict");
    assert_eq!(last["answers_intent"], "no", "{last}");
    assert!(last["gap"].as_str().unwrap_or("").contains("case place1 (given the same call through src/pkg/calc.py:8"), "{last}");
}

