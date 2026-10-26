//! The picker asks the right question for each card hold and for the push question, and reads the person's
//! pick back, on the sample's c1 and the payloads captured live.

use super::*;
use familiar_ai_facts::append_raw;
use familiar_ai_rules::moves::criterion_admissible;
use familiar_ai_rules::sample::{apply_move, criterion_fixture};
use serde_json::json;

fn step(log: &Log, label: &str, outcome: Option<&str>) -> Log {
    let v = View::new(log);
    let m = criterion_admissible(&v)
        .into_iter()
        .find(|m| m.label.as_deref() == Some(label) && outcome.is_none_or(|o| m.fields.get("outcome") == Some(&json!(o))));
    m.and_then(|m| apply_move(log, &m, 0.0, "t").ok()).unwrap_or_else(|| panic!("no {label}"))
}

fn commit(log: &Log, label: &str, outcome: &str) -> Log {
    let mut log = step(log, label, None);
    if label == "tests_only" {
        log = step(&log, "test_written", None);
    }
    log = step(&log, "pass_please_pass", Some(outcome));
    log = step(&log, "pass_please_pass", Some("passed"));
    step(&log, "fingerprint_stamped", None)
}

/// c1 held on two_failures, its holder `a1` on `model`.
fn two_failures(model: &str) -> Log {
    let log = criterion_fixture(&["c1"], true, false, 0.0, "t").unwrap_or_default();
    let seen = json!({ "agent_conversation_id": "a1", "model": model });
    let log = append_raw(&log, "model_seen", seen.as_object().cloned().unwrap_or_default(), 0.0, "t").unwrap_or_default();
    let log = step(&log, "start_check", None);
    let log = commit(&log, "tests_only", "failed");
    let log = commit(&log, "code", "failed");
    commit(&log, "code", "failed")
}

/// c1 held on two_failures with the ladder spent: haiku, then every
/// investigator up to the second fable failing twice too.
fn ladder_spent() -> Log {
    let mut log = two_failures("claude-haiku-4-5-20251001");
    for _ in 0..6 {
        if ladder::investigator_rung(&View::new(&log), "c1").is_none() {
            break;
        }
        log = step(&log, "investigator", None);
        // the investigator's first act is its claim (here)
        log = step(&log, "fix_location_category", None);
        log = commit(&log, "code", "failed");
        log = commit(&log, "code", "failed");
    }
    log
}

/// c1 held on anchor_moved (the start check sees the lines moved).
fn anchor_moved() -> Log {
    let log = criterion_fixture(&["c1"], true, false, 0.0, "t").unwrap_or_default();
    step(&log, "outside_edit_seen", None)
}

/// c1 held on no_test_stack (no test command claimed).
fn no_test_stack() -> Log {
    criterion_fixture(&["c1"], false, false, 0.0, "t").unwrap_or_default()
}

fn fixture(name: &str) -> Value {
    let path = format!("{}/tests/fixtures/{name}.json", env!("CARGO_MANIFEST_DIR"));
    serde_json::from_str(&std::fs::read_to_string(&path).unwrap_or_default()).unwrap_or_default()
}

/// A payload from a capture, as the conversation that holds c1.
fn as_holder(name: &str, log: &Log, tool_input: Option<Value>, tool_response: Option<Value>) -> HookInput {
    let mut j = fixture(name);
    let holder = plan::assigned_to(&View::new(log), "c1").unwrap_or("a1").to_string();
    let (session, agent) = holder.split_once('/').map_or((holder.as_str(), None), |(s, a)| (s, Some(a)));
    j["session_id"] = json!(session);
    if let Some(a) = agent {
        j["agent_id"] = json!(a);
        j["agent_type"] = json!("familiar-ai:investigator");
    }
    if let Some(i) = tool_input {
        j["tool_input"] = i;
    }
    if let Some(r) = tool_response {
        j["tool_response"] = r;
    }
    let event = HookEvent::from_name(j["hook_event_name"].as_str().unwrap_or("")).unwrap_or(HookEvent::Stop);
    HookInput::parse(&j.to_string(), event).unwrap_or_else(|e| panic!("{e}"))
}

fn rewritten(log: &Log) -> Option<Rewrite> {
    let input = as_holder("pre_tool_use_ask_user_question", log, None, None);
    rewrite(&input, log, || panic!("a card is open: the push is not read"), Vec::new)
}

/// The agent's question carrying every piece of the sample's c1 card.
fn prose(happened: &str) -> String {
    format!("you asked: \"let a person log in and reset a forgotten password\"\n\nthe agent split that into one part:\n  1  a right password opens a session\n\nwhat happened: {happened} In the agent's words: \"it changed\"\n\nwhat now?")
}

/// The card the hook asks for `log` when the agent asks `question`.
fn asked_with(log: &Log, question: &str) -> String {
    let input = json!({ "questions": [{ "question": question, "header": "x", "options": [], "multiSelect": false }] });
    let holder = as_holder("pre_tool_use_ask_user_question", log, Some(input), None);
    match rewrite(&holder, log, || panic!("a card is open: the push is not read"), Vec::new) {
        Some(Rewrite::Ask(v)) => v.to_string(),
        other => panic!("not asked: {other:?}"),
    }
}

fn card_json(question: &str, options: &[(&str, &str)]) -> String {
    let options: Vec<Value> = options.iter().map(|(l, d)| json!({ "label": l, "description": d })).collect();
    json!({ "questions": [{ "question": question, "header": "familiar-ai", "options": options, "multiSelect": false }] }).to_string()
}

const OUTSIDE: &str = "someone changed the code the agent had picked for part 1, outside this work.";
const ELSE: (&str, &str) = ("something else", "type what to do in your own words");

// the question is the agent's prose, asked only with every piece; the
// choices are the word table's, in the person's world
fn anchor_moved_card() -> String {
    card_json(
        &prose(OUTSIDE),
        &[
            ("carry on with part 1 on the code as it is now", "carry on with part 1 on the code as it is now"),
            ("have the agent pick the code for part 1 again", "have the agent pick the code for part 1 again"),
            ("drop it", "drop it"),
            ELSE,
        ],
    )
}

#[test]
fn the_agents_question_is_asked_as_the_anchor_moved_card_only_with_the_pieces() {
    let log = anchor_moved();
    match rewritten(&log) {
        Some(Rewrite::Refuse(r)) => assert!(r.starts_with("familiar-ai: ask again; your question must quote: "), "{r}"),
        other => panic!("the capture's \"Continue?\" carries no piece: {other:?}"),
    }
    assert_eq!(asked_with(&log, &prose(OUTSIDE)), anchor_moved_card());
}

#[test]
fn the_two_failures_card_needs_the_person_only_once_the_ladder_is_spent() {
    // on the ladder, an investigator is the way out: the call is untouched
    let held = two_failures("claude-haiku-4-5-20251001");
    assert!(ladder::investigator_rung(&View::new(&held), "c1").is_some());
    let input = as_holder("pre_tool_use_ask_user_question", &held, None, None);
    assert_eq!(rewrite(&input, &held, || false, Vec::new), None);
    // the ladder spent (the second fable failed twice too): the last
    // investigator holds it, and the person's main conversation asks it
    let spent = ladder_spent();
    assert!(plan::assigned_to(&View::new(&spent), "c1").is_some_and(|h| h.starts_with("a1/")));
    let main = HookInput { agent_id: None, agent_type: None, ..as_holder("pre_tool_use_ask_user_question", &spent, None, None) };
    assert_eq!(rewrite(&main, &spent, || false, Vec::new), rewritten(&spent));
    let q = prose("the agent tried part 1 twice and its tests still fail.");
    assert_eq!(
        asked_with(&spent, &q),
        card_json(&q, &[("let the agent try part 1 once more", "let the agent try part 1 once more"), ("you take over part 1 yourself", "you take over part 1 yourself"), ("what fails is in something part 1 depends on; plan that first", "what fails is in something part 1 depends on; plan that first"), ("drop it", "drop it")])
    );
}

#[test]
fn the_two_failures_card_off_the_ladder_is_the_persons_too() {
    let log = two_failures("gpt-5");
    let q = prose("the agent tried part 1 twice and its tests still fail.");
    assert_eq!(
        asked_with(&log, &q),
        card_json(&q, &[("let the agent try part 1 once more", "let the agent try part 1 once more"), ("you take over part 1 yourself", "you take over part 1 yourself"), ("what fails is in something part 1 depends on; plan that first", "what fails is in something part 1 depends on; plan that first"), ("drop it", "drop it")])
    );
}

#[test]
fn the_no_test_stack_card_is_typed() {
    let q = prose("the agent found no command that runs the tests for part 1.");
    assert_eq!(
        asked_with(&no_test_stack(), &q),
        card_json(&q, &[("type the command", "type the command that runs the tests in the box below; it is recorded as the test command")])
    );
}

#[test]
fn with_no_card_open_the_push_question_or_nothing() {
    let log = criterion_fixture(&["c1"], true, false, 0.0, "t").unwrap_or_default();
    let input = as_holder("pre_tool_use_ask_user_question", &log, None, None);
    assert_eq!(rewrite(&input, &log, || false, Vec::new), None, "untouched");
    assert_eq!(
        rewrite(&input, &log, || true, Vec::new),
        Some(Rewrite::Ask(serde_json::from_str(r#"{"questions":[{"question":"Every criterion is locally green. Push and open the pull request so CI proves it?","header":"familiar-ai","options":[{"label":"yes, push and open it","description":"the agent pushes the branch and opens the pull request"},{"label":"not now","description":"nothing is pushed now"}],"multiSelect":false}]}"#).unwrap_or_default()))
    );
    // another conversation, or another tool, is never rewritten
    let moved = anchor_moved();
    let mut other = as_holder("pre_tool_use_ask_user_question", &moved, None, None);
    other.session_id = Some("b2".into());
    assert_eq!(rewrite(&other, &moved, || false, Vec::new), None);
    let edit = HookInput { tool_name: Some("Edit".into()), ..as_holder("pre_tool_use_ask_user_question", &moved, None, None) };
    assert_eq!(rewrite(&edit, &moved, || true, Vec::new), None);
}

fn answered(log: &Log, card: &str, answer: &str) -> Option<Picked> {
    let input: Value = serde_json::from_str(card).unwrap_or_default();
    let q = input["questions"][0]["question"].as_str().unwrap_or("").to_string();
    let response = json!({ "questions": input["questions"], "answers": { q: answer }, "annotations": {} });
    picked(&as_holder("post_tool_use_ask_user_question_picked", log, Some(input), Some(response)), log, log)
}

#[test]
fn the_persons_pick_is_read_back_to_the_choice_or_the_typed_words() {
    let log = anchor_moved();
    let answer = |choice: &str, typed: &str| Some(Picked::Answer { criterion_id: "c1".into(), hold: "anchor_moved".into(), choice: choice.into(), typed: typed.into() });
    // the choice lines are in the person's world
    let card = anchor_moved_card();
    assert_eq!(answered(&log, &card, "have the agent pick the code for part 1 again"), answer("re_plan", ""));
    assert_eq!(answered(&log, &card, "carry on with part 1 on the code as it is now"), answer("keep_going", ""));
    assert_eq!(answered(&log, &card, "drop it"), answer("release", ""));
    assert_eq!(answered(&log, &card, "run pytest -q first"), answer(crate::teach::picker::TYPED_CHOICE, "run pytest -q first"));
    assert_eq!(answered(&log, &card, "something else"), Some(Picked::NothingTyped { criterion_id: "c1".into(), label: "something else".into() }));
    // the typed card: the text is the command
    let typed = no_test_stack();
    let card = asked_with(&typed, &prose("the agent found no command that runs the tests for part 1."));
    assert_eq!(answered(&typed, &card, "pytest -q tests/test_login.py"), Some(Picked::Command { criterion_id: "c1".into(), command: "pytest -q tests/test_login.py".into() }));
    assert_eq!(answered(&typed, &card, "type the command"), Some(Picked::NothingTyped { criterion_id: "c1".into(), label: "type the command".into() }));
    // the push question: told, never written
    let clear = criterion_fixture(&["c1"], true, false, 0.0, "t").unwrap_or_default();
    let push = crate::teach::picker::push_input().to_string();
    assert_eq!(answered(&clear, &push, "not now"), Some(Picked::Push("not now".into())));
}

#[test]
fn only_the_card_the_hook_put_in_the_picker_is_the_persons_answer() {
    let log = anchor_moved();
    // the live capture is the integration project's card ("greet says
    // hello"), not this log's open card: not read as an answer here
    let captured = as_holder("post_tool_use_ask_user_question_picked", &log, None, None);
    assert_eq!(picked(&captured, &log, &log), None);
    // the agent's own question (another header): not the hook's
    let mut own = captured.clone();
    own.tool_input = Some(json!({ "questions": [{ "question": "Continue?", "header": "x", "options": [], "multiSelect": false }] }));
    own.tool_response = Some(json!({ "answers": { "Continue?": "release it" } }));
    assert_eq!(picked(&own, &log, &log), None, "not the hook's question");
    // a card this very call opened was never put in the picker by the hook:
    // the agent's own question with the card's words is not the person's pick
    // (the live input carries the answers on PostToolUse whoever gave them)
    let input: Value = serde_json::from_str(&anchor_moved_card()).unwrap_or_default();
    let q = input["questions"][0]["question"].as_str().unwrap_or("").to_string();
    let mut forged = input.clone();
    forged["answers"] = json!({ q.clone(): "drop it" });
    let pre_answered = as_holder("post_tool_use_ask_user_question_picked", &log, Some(forged), Some(json!({ "answers": { q.clone(): "drop it" } })));
    let before_the_card = criterion_fixture(&["c1"], true, false, 0.0, "t").unwrap_or_default();
    assert_eq!(picked(&pre_answered, &before_the_card, &log), None);
    assert!(picked(&pre_answered, &log, &log).is_some(), "open before and now: the hook's own card");
    // the picker run with no answer (the "allow" capture: answers {})
    let empty = as_holder("post_tool_use_ask_user_question_allowed_unasked", &log, Some(input), Some(json!({ "answers": {} })));
    assert_eq!(picked(&empty, &log, &log), None);
    // once answered, the card is closed: the same pick again writes nothing
    let done = append_raw(&log, "hold_answered", json!({ "criterion_id": "c1", "hold": "anchor_moved", "choice": "keep_going", "typed": "" }).as_object().cloned().unwrap_or_default(), 1.0, "t").unwrap_or_default();
    assert_eq!(answered(&done, &anchor_moved_card(), "drop it"), None);
    // the table's options under a question with no piece: not a card the hook asked
    let mut none: Value = serde_json::from_str(&anchor_moved_card()).unwrap_or_default();
    none["questions"][0]["question"] = json!("Continue?");
    assert_eq!(answered(&log, &none.to_string(), "drop it"), None);
}
