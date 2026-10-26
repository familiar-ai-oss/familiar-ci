//! The plan example a code anchor needs lands on the first call (finding 35,
//! the 44 benchmark): the form `familiar-ai plan` prints with nothing on stdin
//! is right for a code anchor. The criterion on it is the full path and names
//! the behavior and promise claimed before it; an agent that fills only its
//! own values (text, file, symbol, lines, test file, command) and feeds it
//! back gets "the plan is confirmed" on that one call.

mod common;

use common::Project;
use serde_json::{json, Value};

const SESSION: &str = "11111111-2222-4333-8444-555555555555";

fn project(name: &str) -> Project {
    Project::builder(name).session(SESSION).env("PATH", "/usr/bin:/bin").create()
}

/// The form `plan` prints, cut to the lines one code criterion needs (the two
/// other ways to anchor and the further criterion's string are left out, as
/// an agent leaves them out), with only the case's own values put in.
fn filled(form: &Value) -> Value {
    let mut lines: Vec<Value> = Vec::new();
    for line in form.as_array().expect("an array") {
        let kind = line["kind"].as_str().expect("kind");
        if ["new_work_spot_chosen", "new_work_no_spot", "criterion_strung_on_head"].contains(&kind) {
            continue;
        }
        let mut line = line.clone();
        match kind {
            "intent_submitted" => line["text"] = json!("greet with hello"),
            "criterion" => line["text"] = json!("greet says hello"),
            "behavior" => line["text"] = json!("the greeting"),
            "place_chosen" => line["places"] = json!([{"file_path": "src/app.sh", "symbol": "greet", "line_start": 2, "line_end": 2}]),
            "test_planned" => line["file_path"] = json!("tests/test_c1.sh"),
            // the agent chooses where the fix lives; the printed form does not
            "fix_location_category" => line["category"] = json!("this_repository"),
            "test_stack_detected" => {
                line["test_stack"] = json!("sh");
                line["test_command"] = json!("sh tests/test_c1.sh");
            }
            _ => {}
        }
        lines.push(line);
    }
    Value::Array(lines)
}

#[test]
fn the_form_plan_prints_lands_on_a_code_anchor_in_one_call() {
    let p = project("lands");
    p.hook("UserPromptSubmit", json!({ "prompt": "greet with hello" }));
    let (code, form, err) = p.run(&["plan"], "");
    assert_eq!(code, 0, "{form}{err}");
    let form: Value = serde_json::from_str(&form).expect("a JSON array");
    let (code, out, err) = p.run(&["plan"], &filled(&form).to_string());
    assert_eq!(code, 0, "the form as printed, with the case's own values, is not refused: {out}{err}");
    // definition change (plan cases-card, 2026-10-25): a full-path plan waits on the case-finder and the person's card before it is confirmed [draft names]
    // updated with the definition (the cases card's words, step 4) [draft words]: the hold reads in the binary's words for cases_owed
    let owed = format!("· {}", familiar_ai::teach::words::hold(&familiar_ai_rules::View::new(&p.facts()), Some("c1"), "cases_owed"));
    assert!(out.contains("familiar-ai: the plan is still being written") && out.contains(&owed), "{out}");
    p.cases_kept("c1");
    assert!(p.status().starts_with("familiar-ai: the plan is confirmed"), "{}", p.status());
}

#[test]
fn the_forms_criterion_is_the_full_path_with_its_behavior_and_promise_before_it() {
    let p = project("order");
    let (_, form, _) = p.run(&["plan"], "");
    let form: Value = serde_json::from_str(&form).expect("a JSON array");
    let kinds: Vec<&str> = form.as_array().expect("array").iter().map(|l| l["kind"].as_str().expect("kind")).collect();
    let at = |k: &str| kinds.iter().position(|x| *x == k).expect(k);
    assert!(at("behavior") < at("promise") && at("promise") < at("criterion"), "{kinds:?}");
    let criterion = &form[at("criterion")];
    assert_eq!(criterion["path"], "full", "{criterion}");
    assert_eq!(criterion["behavior_id"], "b1", "{criterion}");
    assert_eq!(criterion["promise_id"], "p1", "{criterion}");
}

#[test]
fn claim_criterion_with_no_fields_shows_the_full_path_example() {
    let p = project("claim_form");
    let (code, out, err) = p.run(&["claim", "criterion"], "");
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("--path full --behavior_id b1 --promise_id p1"), "{out}");
    assert!(!out.contains("familiar-ai claim criterion --criterion_id c1 --text 'greet says hello' --path light"), "{out}");
}
