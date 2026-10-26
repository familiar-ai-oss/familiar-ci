//! A clarifying question at plan time, only on a signal (the owner's plan of 2026-10-21, step 2)
//! [draft names], through the real binary. The person's prompt is short and has no example, so a
//! trigger fires: the tests-first commit is refused (`reading_unclear`, the agent's hold) until the
//! agent says how the ask reads, `one_reading` with why (no card), or lists the readings it sees,
//! `reading_alternatives`, which is the person's card with the readings as its choices. The
//! person's pick is recorded as theirs and the commit goes on.

mod common;

use common::Project;
use serde_json::{json, Value};

const SESSION: &str = "84be6339-0cc7-40ec-91b9-cae44365931b";
const ASKED: &str = "make the report shorter";
const FIRST: &str = "the report lists fewer rows";
const SECOND: &str = "the report's file is smaller on disk";
const CASES: &str = r#"[{"case_id":"n1","kind":"normal","given":"a long report","expect":"a shorter one"},{"case_id":"o1","kind":"opposite","given":"a short report","expect":"the same report"}]"#;

/// No `gh` on the PATH: the push question is not read here.
fn project(name: &str) -> Project {
    let p = Project::builder(name).session(SESSION).env("PATH", "/usr/bin:/bin").create();
    p.hook("UserPromptSubmit", json!({ "prompt": ASKED }));
    p.claim(&["intent_submitted", "--text", ASKED]);
    p.claim(&["behavior", "--behavior_id", "b1", "--text", "the report"]);
    p.claim(&["promise", "--promise_id", "p1", "--behavior_id", "b1", "--accepts", "a report", "--gives_back", "a shorter report", "--refuses", "nothing", "--cases", CASES]);
    p.claim(&["criterion", "--criterion_id", "c1", "--text", "the report is shorter", "--path", "full", "--behavior_id", "b1", "--promise_id", "p1"]);
    p.cases_kept("c1"); // the cases card passed (plan cases-card, 2026-10-25) [draft names]
    p.claim(&["place_chosen", "--criterion_id", "c1", "--places", r#"[{"file_path":"src/app.sh","symbol":"greet","line_start":1,"line_end":3}]"#]);
    p.claim(&["test_planned", "--criterion_id", "c1", "--file_path", "tests/test_c1.sh", "--will", "create"]);
    p.claim(&["test_stack_detected", "--project_root", ".", "--test_stack", "sh", "--test_command", "sh tests/test_c1.sh"]);
    p.claim(&["criterion_head", "--criterion_id", "c1"]);
    p.claim(&["criterion_assigned", "--criterion_id", "c1"]);
    p.claim(&["fix_location_category", "--criterion_id", "c1", "--category", "this_repository", "--where", ""]);
    p
}

fn commit_asked(p: &Project) -> Option<String> {
    let out = p.bash("PreToolUse", "git commit -m 'c1: tests'");
    (out["hookSpecificOutput"]["permissionDecision"] == "deny").then(|| out["hookSpecificOutput"]["permissionDecisionReason"].as_str().unwrap_or("").to_string())
}

impl Project {
    /// The PostToolUse of the card picked, with the answer in both the tool's result and input.
    fn picked(&self, card: &Value, answer: &str) -> Value {
        let q = card["questions"][0]["question"].as_str().unwrap_or("").to_string();
        let response = json!({ "questions": card["questions"], "answers": { q: answer }, "annotations": {} });
        let mut input = card.clone();
        input["answers"] = response["answers"].clone();
        input["annotations"] = json!({});
        self.hook_fixture("PostToolUse", "post_tool_use_ask_user_question_picked", json!({ "tool_input": input, "tool_response": response }))
    }
}

#[test]
fn a_short_ask_holds_the_commit_until_the_agent_says_how_it_reads() {
    let p = project("reading-one");
    assert_eq!(p.state_name("c1"), "held", "{}", p.status());
    let why = commit_asked(&p).expect("the tests-first commit is refused");
    assert!(why.contains("reading unclear"), "{why}");
    assert!(why.contains("few words and no example"), "{why}");
    assert!(why.contains("familiar-ai claim one_reading --why"), "{why}");
    assert!(why.contains("familiar-ai claim reading_alternatives --readings"), "{why}");
    assert!(!p.status().contains("a question for you"), "the agent's hold, never a card: {}", p.status());

    // one reading listed is no alternatives; a blank why is no why
    let (code, _, err) = p.try_claim(&["reading_alternatives", "--readings", &json!([FIRST]).to_string()]);
    assert_eq!(code, 1, "{err}");
    assert!(err.contains("two or three"), "{err}");
    let (code, _, err) = p.try_claim(&["one_reading", "--why", " "]);
    assert_eq!(code, 1, "{err}");

    p.claim(&["one_reading", "--why", "the report is the one page the export writes; shorter means fewer rows"]);
    assert_eq!(commit_asked(&p), None, "{}", p.status());
    assert!(!p.status().contains("a question for you"), "no card: {}", p.status());
}

#[test]
fn the_readings_are_the_persons_card_and_their_pick_lets_the_commit_go_on() {
    let p = project("reading-card");
    p.claim(&["reading_alternatives", "--readings", &json!([FIRST, SECOND]).to_string()]);
    let status = p.status();
    assert!(status.contains("a question for you"), "{status}");
    assert!(commit_asked(&p).is_some_and(|w| w.contains("reading unclear")), "the commit waits on the person");

    // the agent's prose with the card's pieces is asked; the choices are the readings, verbatim
    let prose = format!("you asked: \"{ASKED}\"\n\nthe agent split that into one part:\n  1  the report is shorter\n\nwhat happened: your ask can be read more than one way, and the agent will plan for the one you pick. In the agent's words: \"one keeps fewer rows, the other writes a smaller file\"\n\nwhat now?");
    let pre = p.hook_fixture("PreToolUse", "pre_tool_use_ask_user_question", json!({ "tool_input": { "questions": [{ "question": prose, "header": "x", "options": [], "multiSelect": false }] } }));
    let hso = &pre["hookSpecificOutput"];
    assert_eq!(hso["permissionDecision"], "ask", "{pre}");
    let card = hso["updatedInput"].clone();
    let labels: Vec<&str> = card["questions"][0]["options"].as_array().map(|o| o.iter().filter_map(|x| x["label"].as_str()).collect()).unwrap_or_default();
    assert_eq!(labels, [FIRST, SECOND, "something else"]);

    let post = p.picked(&card, SECOND);
    assert!(post["systemMessage"].as_str().unwrap_or("").contains(&format!("\"{SECOND}\"")), "{post}");
    assert_eq!(p.card_answers(), [json!({ "criterion_id": "c1", "hold": "reading_unclear", "choice": SECOND, "typed": "" })]);
    assert!(!p.status().contains("a question for you"), "{}", p.status());
    assert_eq!(commit_asked(&p), None, "{}", p.status());
}
