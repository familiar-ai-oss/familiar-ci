//! The plan example does not choose where the fix lives (finding 35, the 44
//! benchmark): agents copied `"category": "this_repository"` even when the fix
//! needed a dependency's code or a tool the sandbox lacks. The printed form
//! leaves the category empty and the kind's form gives each of the four
//! values its meaning, so a copied example is refused for an empty category
//! (naming the four values) or carries a real choice.


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

fn printed_and_filled(p: &Project) -> Value {
    p.hook("UserPromptSubmit", json!({ "prompt": "greet with hello" }));
    let (code, form, err) = p.run(&["plan"], "");
    assert_eq!(code, 0, "{form}{err}");
    filled(&serde_json::from_str(&form).expect("a JSON array"))
}

fn category_of(plan: &Value) -> &Value {
    &plan.as_array().expect("array").iter().find(|l| l["kind"] == "fix_location_category").expect("line 11")["category"]
}

#[test]
fn a_plan_built_from_the_printed_form_without_choosing_a_category_is_refused_naming_the_four_values() {
    let p = project("unchosen");
    let plan = printed_and_filled(&p);
    assert_eq!(category_of(&plan), "", "the printed form does not choose");
    let (code, out, err) = p.run(&["plan"], &plan.to_string());
    assert_eq!(code, 1, "{out}{err}");
    let all = format!("{out}{err}");
    assert!(all.contains("line 11 of 11, fix_location_category"), "{all}");
    for v in ["this_repository", "dependency", "environment", "request"] {
        assert!(all.contains(v), "{v}: {all}");
    }
    assert!(!all.contains("the plan is confirmed"), "{all}");
}

#[test]
fn with_a_choice_the_same_plan_lands() {
    let p = project("chosen");
    let mut plan = printed_and_filled(&p);
    for l in plan.as_array_mut().expect("array") {
        if l["kind"] == "fix_location_category" {
            l["category"] = json!("this_repository");
        }
    }
    let (code, out, err) = p.run(&["plan"], &plan.to_string());
    assert_eq!(code, 0, "{out}{err}");
    // definition change (plan cases-card, 2026-10-25): a full-path plan waits on the case-finder and the person's card before it is confirmed [draft names]
    // updated with the definition (the cases card's words, step 4) [draft words]: the hold reads in the binary's words for cases_owed
    let owed = format!("· {}", familiar_ai::teach::words::hold(&familiar_ai_rules::View::new(&p.facts()), Some("c1"), "cases_owed"));
    assert!(out.contains("familiar-ai: the plan is still being written") && out.contains(&owed), "{out}");
    p.cases_kept("c1");
    assert!(p.status().starts_with("familiar-ai: the plan is confirmed"), "{}", p.status());
}

#[test]
fn the_kinds_form_gives_each_value_its_meaning_and_the_example_picks_none() {
    let p = project("form");
    let (code, out, err) = p.run(&["claim", "fix_location_category"], "");
    assert_eq!(code, 0, "{out}{err}");
    for v in ["this_repository:", "dependency:", "environment:", "request:"] {
        assert!(out.contains(v), "{v}: {out}");
    }
    let example = out.split("example:\n").nth(1).expect("an example");
    assert!(!example.contains("this_repository"), "{example}");
    assert!(example.contains("--category ''"), "{example}");
}

