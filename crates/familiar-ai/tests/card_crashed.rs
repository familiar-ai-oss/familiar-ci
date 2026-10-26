//! A crash at the tests-first commit is the person's card (plan §6.1): its_the_bug seals on the
//! crash and tells the holder once where to fix; a_dependency records the pick and says the
//! dependency's plan is owed; release lets the part go.

mod common;
mod run_cards;

use familiar_ai_rules::criterion::{sealed_at, Place};
use familiar_ai_rules::{lifecycle, View};
use run_cards::*;
use serde_json::json;

fn crashed(name: &str) -> common::Project {
    let p = sealed(name);
    relabel(&p, "crashed", json!({ "location": { "file_path": "src/app.sh", "line": 2, "function": "greet() {" } }));
    p
}

#[test]
fn a_crash_before_the_seal_opens_the_card_with_its_three_choices() {
    let p = crashed("crash-card");
    assert_eq!(cards(&p), ["crashed"]);
    let status = p.status();
    assert!(status.contains("its test crashed before any fix; the crash points at src/app.sh greet"), "{status}");
    assert!(status.contains("what happened: the test the agent wrote for part 1 crashed before any fix."), "{status}");
    let card = ask(&p);
    let labels: Vec<String> = card["questions"][0]["options"].as_array().into_iter().flatten().filter_map(|o| o["label"].as_str().map(str::to_string)).collect();
    assert_eq!(
        labels,
        ["the crash is the problem part 1 fixes; go on to the fix", "the crash is in something part 1 depends on; plan that first", "drop it", "something else"]
    );
    let stop = p.hook("Stop", json!({}));
    assert_eq!(stop["decision"], "block", "{stop}");
}

#[test]
fn its_the_bug_seals_on_the_crash_and_the_cta_is_told_once() {
    let p = crashed("crash-bug");
    let out = pick(&p, &ask(&p), "the crash is the problem part 1 fixes; go on to the fix");
    let cta = "\"greet says hello\" is sealed on the crash at src/app.sh:2. Fix the code at src/app.sh greet lines 1–3; the test stays as it is.";
    assert_eq!(context(&out).matches(cta).count(), 1, "{out}");
    let log = p.facts();
    let v = View::new(&log);
    assert!(sealed_at(&v, "c1", Place::Laptop).is_some(), "sealed on the crash");
    assert_eq!(lifecycle::state(&v, "c1"), "sealed");
}

#[test]
fn a_dependency_is_recorded_and_the_cta_says_its_plan_is_owed() {
    let p = crashed("crash-dep");
    let out = pick(&p, &ask(&p), "the crash is in something part 1 depends on; plan that first");
    assert_eq!(context(&out).matches("A dependency comes first for \"greet says hello\": its plan is owed.").count(), 1, "{out}");
    assert_eq!(p.card_answers(), [json!({ "criterion_id": "c1", "hold": "crashed", "choice": "a_dependency", "typed": "" })]);
    assert!(cards(&p).is_empty());
}

#[test]
fn release_lets_it_go_and_says_no_part_is_left() {
    let p = crashed("crash-release");
    let out = pick(&p, &ask(&p), "drop it");
    assert_eq!(context(&out).matches("\"greet says hello\" is released. Next: no part is left; the plan is complete.").count(), 1, "{out}");
    let stop = p.hook("Stop", json!({}));
    assert_ne!(stop["decision"], "block", "{stop}");
}
