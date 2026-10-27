//! The lens panel (the owner's plan of 2026-10-26, lens-panel.md, approved with its names): five
//! read-only lenses (`ambiguity`, `completeness`, `happy_path`, `edge_cases`, `order_and_coupling`)
//! answer in parallel, then the synthesis writes the cases and up to three questions for the person.
//! The binary records which Agent call got which lens (`lens_released`: the part, the lens or the
//! synthesis, the call's tool_use_id) and each lens's answer (`lens_found`: the part, the lens, the
//! outcome found / none / failed, its findings, the model, the tokens). The synthesis stays recorded
//! as `cases_found`, which gains the optional `questions` (each its id q1..q3, its text, two to four
//! options and why). The person's pick on a question is `question_answered` (the part, the
//! question's id, the choice, the words typed). The three kinds come after `cases_found`.

use familiar_ai_facts::*;
use serde_json::{json, Map, Value};

const AT: f64 = 1.0;
const TENANT: &str = "_default_";

fn fields(v: Value) -> Map<String, Value> {
    v.as_object().cloned().unwrap_or_default()
}

fn ok(kind: &str, v: Value) -> Log {
    append_raw(&Vec::new(), kind, fields(v), AT, TENANT).unwrap_or_else(|e| panic!("{kind}: {e}"))
}

fn refused(kind: &str, v: Value) -> bool {
    append_raw(&Vec::new(), kind, fields(v), AT, TENANT).is_err()
}

const LENS_IDS: [&str; 5] = ["ambiguity", "completeness", "happy_path", "edge_cases", "order_and_coupling"];

fn released() -> Value {
    json!({ "criterion_id": "c1", "lens": "ambiguity", "tool_use_id": "toolu_01" })
}

fn lens_found() -> Value {
    json!({
        "criterion_id": "c1",
        "lens": "edge_cases",
        "outcome": "found",
        "findings": [
            { "kind": "edge_case", "note": "a nested group whose frames are all hidden", "file_path": "src/_pytest/_code/code.py", "target_symbol": "FormattedExcinfo.repr_excinfo", "candidate_question": "Should a nested group show the message too?" },
            { "kind": "input", "note": "an empty group", "file_path": "", "target_symbol": "", "candidate_question": "" }
        ],
        "model": "claude-opus-5-5",
        "tokens": 8100
    })
}

fn synthesis() -> Value {
    json!({
        "criterion_id": "c1",
        "outcome": "found",
        "cases": [{ "case_id": "a1", "short": "a nested group", "given": "a nested all-hidden group", "expect": "the message", "why": "repr_excinfo recurses" }],
        "why": "",
        "model": "claude-opus-5-5",
        "tokens": 6400,
        "questions": [
            { "question_id": "q1", "text": "Which --tb styles should show the message?", "options": ["every style", "only long"], "why": "the report names none" },
            { "question_id": "q2", "text": "Does a nested group count?", "options": ["yes", "no", "only when all hidden"], "why": "the code recurses" }
        ]
    })
}

fn answered() -> Value {
    json!({ "criterion_id": "c1", "question_id": "q1", "choice": "every style", "typed": "" })
}

#[test]
fn lens_released_names_the_part_the_lens_or_the_synthesis_and_the_call() {
    let log = ok("lens_released", released());
    let typed: LensReleased = log[0].fields_as().expect("its typed form");
    assert_eq!((typed.criterion_id.as_str(), typed.lens.as_str(), typed.tool_use_id.as_str()), ("c1", "ambiguity", "toolu_01"));
    for lens in LENS_IDS.iter().chain(["synthesis"].iter()) {
        let mut f = released();
        f["lens"] = json!(lens);
        ok("lens_released", f);
    }
    let mut f = released();
    f["lens"] = json!("case_finder");
    assert!(refused("lens_released", f), "the lens is one of the five or the synthesis");
    for k in ["criterion_id", "lens", "tool_use_id"] {
        let mut f = released();
        f.as_object_mut().unwrap().remove(k);
        assert!(refused("lens_released", f), "{k} is required");
    }
}

#[test]
fn lens_found_round_trips_flat_and_typed_for_each_lens_and_outcome() {
    let log = ok("lens_found", lens_found());
    let typed: LensFound = log[0].fields_as().expect("its typed form");
    assert_eq!(typed.lens, "edge_cases");
    assert_eq!(typed.outcome, "found");
    assert_eq!(typed.findings.len(), 2);
    assert_eq!(typed.findings[0].target_symbol, "FormattedExcinfo.repr_excinfo");
    assert_eq!(typed.findings[0].candidate_question, "Should a nested group show the message too?");
    assert_eq!(typed.tokens, 8100);
    for lens in LENS_IDS {
        for outcome in ["found", "none", "failed"] {
            let mut f = lens_found();
            f["lens"] = json!(lens);
            f["outcome"] = json!(outcome);
            if outcome != "found" {
                f["findings"] = json!([]);
            }
            ok("lens_found", f);
        }
    }
}

#[test]
fn lens_found_refuses_the_synthesis_another_lens_another_outcome_and_a_wrong_shape() {
    for (k, bad) in [("lens", json!("synthesis")), ("lens", json!("case_finder")), ("outcome", json!("passed")), ("findings", json!("none")), ("tokens", json!("many"))] {
        let mut f = lens_found();
        f[k] = bad.clone();
        assert!(refused("lens_found", f), "{k} = {bad}");
    }
    for k in ["criterion_id", "lens", "outcome", "findings", "model", "tokens"] {
        let mut f = lens_found();
        f.as_object_mut().unwrap().remove(k);
        assert!(refused("lens_found", f), "{k} is required");
    }
    let mut f = lens_found();
    f["findings"] = json!([{ "kind": "edge_case", "note": "n" }]);
    assert!(refused("lens_found", f), "each finding names its kind, note, file_path, target_symbol and candidate_question");
    let mut f = lens_found();
    f["findings"][0]["verdict"] = json!("yes");
    assert!(refused("lens_found", f), "and nothing else");
    let mut f = lens_found();
    f["questions"] = json!([]);
    assert!(refused("lens_found", f), "a lens asks no question of the person: the synthesis does");
}

#[test]
fn cases_found_may_carry_up_to_three_questions_with_two_to_four_options() {
    let log = ok("cases_found", synthesis());
    let typed: CasesFound = log[0].fields_as().expect("its typed form");
    assert_eq!(typed.questions.len(), 2);
    assert_eq!(typed.questions[1].question_id, "q2");
    assert_eq!(typed.questions[1].options, ["yes", "no", "only when all hidden"]);
    assert_eq!(typed.questions[0].why, "the report names none");
    // without questions, as the case-finder recorded it: the field is optional and stays absent
    let mut none = synthesis();
    none.as_object_mut().unwrap().remove("questions");
    let log = ok("cases_found", none.clone());
    let typed: CasesFound = log[0].fields_as().expect("its typed form");
    assert!(typed.questions.is_empty());
    let back = append(&Vec::new(), &typed, AT, TENANT).expect("typed append");
    assert!(!back[0].fields.contains_key("questions"), "no questions writes no field");
    assert_eq!(form_row("cases_found").map(|r| r.optional.to_vec()), Some(vec!["questions"]));
    // three questions, four options each
    let mut three = synthesis();
    three["questions"] = json!([
        { "question_id": "q1", "text": "a", "options": ["1", "2", "3", "4"], "why": "w" },
        { "question_id": "q2", "text": "b", "options": ["1", "2"], "why": "w" },
        { "question_id": "q3", "text": "c", "options": ["1", "2"], "why": "w" }
    ]);
    ok("cases_found", three);
}

#[test]
fn cases_found_refuses_a_question_out_of_its_shape() {
    let q = |id: &str, n: usize| json!({ "question_id": id, "text": "t", "options": (1..=n).map(|i| format!("o{i}")).collect::<Vec<_>>(), "why": "w" });
    for (what, qs) in [
        ("four questions", json!([q("q1", 2), q("q2", 2), q("q3", 2), q("q3", 2)])),
        ("an id outside q1..q3", json!([q("q4", 2)])),
        ("an id twice", json!([q("q1", 2), q("q1", 2)])),
        ("one option", json!([q("q1", 1)])),
        ("five options", json!([q("q1", 5)])),
        ("a missing why", json!([{ "question_id": "q1", "text": "t", "options": ["a", "b"] }])),
        ("an unknown key", json!([{ "question_id": "q1", "text": "t", "options": ["a", "b"], "why": "w", "answer": "a" }])),
        ("not a list", json!("q1")),
    ] {
        let mut f = synthesis();
        f["questions"] = qs;
        assert!(refused("cases_found", f), "{what}");
    }
}

#[test]
fn question_answered_is_the_persons_pick_on_one_question() {
    let log = ok("question_answered", answered());
    let typed: QuestionAnswered = log[0].fields_as().expect("its typed form");
    assert_eq!((typed.question_id.as_str(), typed.choice.as_str(), typed.typed.as_str()), ("q1", "every style", ""));
    ok("question_answered", json!({ "criterion_id": "c1", "question_id": "q3", "choice": "typed", "typed": "only under --tb=long" }));
    let mut f = answered();
    f["question_id"] = json!("q4");
    assert!(refused("question_answered", f), "the question's id is q1..q3");
    for k in ["criterion_id", "question_id", "choice", "typed"] {
        let mut f = answered();
        f.as_object_mut().unwrap().remove(k);
        assert!(refused("question_answered", f), "{k} is required");
    }
}

#[test]
fn the_three_kinds_come_after_cases_found_in_the_form() {
    let at = |k: &str| KINDS.iter().position(|x| *x == k);
    let i = at("cases_found").expect("cases_found");
    assert_eq!(at("lens_released"), Some(i + 1));
    assert_eq!(at("lens_found"), Some(i + 2));
    assert_eq!(at("question_answered"), Some(i + 3));
    assert_eq!(at("question_answered"), Some(KINDS.len() - 1));
    for k in ["lens_released", "lens_found", "question_answered"] {
        assert_eq!(form_row(k).map(|r| r.optional.len()), Some(0), "{k}");
    }
}
