//! The person's card goes through Claude Code's AskUserQuestion picker in the
//! real binary, with payloads captured live: the agent's own question is
//! refused, its prose with the card's pieces is asked, and the pick is recorded.

mod common;

use common::Project;
use serde_json::{json, Value};

const SESSION: &str = "d925ef40-8c5a-4ff1-a3f9-84909cb79d71";
// The card's question is the agent's prose with the required pieces; the choices are the word table's.
const PROSE: &str = "you asked: \"greet with hello\"\n\nthe agent split that into one part:\n  1  greet says hello\n\nwhat happened: someone changed the code the agent had picked for part 1, outside this work. In the agent's words: \"greet now prints hey instead of hi\"\n\nwhat now?";
const CARD: &str = r#"{"questions":[{"question":"you asked: \"greet with hello\"\n\nthe agent split that into one part:\n  1  greet says hello\n\nwhat happened: someone changed the code the agent had picked for part 1, outside this work. In the agent's words: \"greet now prints hey instead of hi\"\n\nwhat now?","header":"familiar-ai","options":[{"label":"carry on with part 1 on the code as it is now","description":"carry on with part 1 on the code as it is now"},{"label":"have the agent pick the code for part 1 again","description":"have the agent pick the code for part 1 again"},{"label":"drop it","description":"drop it"},{"label":"something else","description":"type what to do in your own words"}],"multiSelect":false}]}"#;
// What the agent is told: the pieces, the shape, and one sentence of its own.
const FACTS: &str = "familiar-ai: a question for the person is open for part 1 of what they asked. Write your question to them as prose, in this shape and order:
  you asked: \"greet with hello\"
  the agent split that into one part:
    1  greet says hello
  what happened: someone changed the code the agent had picked for part 1, outside this work.
  In the agent's words: \"<one sentence of yours, in plain words: what the code it had picked for part 1 now does differently>\"
  what now?
Quote the person's words and each part's text exactly as above, write the one sentence yourself, and leave the choices to the hook. The record's card: \"greet says hello\": the code at src/app.sh greet changed after the plan was made. What now? Ask the person with AskUserQuestion: the hook turns that call into the question card, and their choice is recorded; do not claim hold_answered yourself.";

/// The agent's AskUserQuestion with `question` (the live capture's options).
fn asking(question: &str) -> Value {
    json!({ "tool_input": { "questions": [{ "question": question, "header": "x", "options": [
        { "label": "yes", "description": "Continue" }, { "label": "no", "description": "Do not continue" }
    ], "multiSelect": false }] } })
}

/// No `gh` on the PATH: the push question is not read here.
fn project(name: &str) -> Project {
    Project::builder(name).session(SESSION).env("PATH", "/usr/bin:/bin").create()
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

    /// What `familiar-ai status` prints from the person's terminal.
    fn record(&self) -> String {
        self.run_as(None, &["status"], "").1
    }

    /// The plan, with the anchored lines edited before the last line, so c1 is held on anchor_moved.
    fn moved(name: &str) -> Project {
        let p = project(name);
        p.claim(&["intent_submitted", "--for", "0", "--text", "greet with hello"]);
        p.claim(&["behavior", "--behavior_id", "b1", "--text", "the greeting"]);
        p.claim(&["promise", "--promise_id", "p1", "--behavior_id", "b1", "--accepts", "a name", "--gives_back", "a greeting", "--refuses", "nothing", "--cases", r#"[{"case_id":"n1","kind":"normal","given":"a name","expect":"a greeting"},{"case_id":"o1","kind":"opposite","given":"no name","expect":"no greeting"}]"#]);
        p.claim(&["criterion", "--criterion_id", "c1", "--text", "greet says hello", "--path", "full", "--behavior_id", "b1", "--promise_id", "p1"]);
        p.cases_kept("c1"); // the cases card passed (plan cases-card, 2026-10-25) [draft names]
        p.claim(&["place_chosen", "--criterion_id", "c1", "--places", r#"[{"file_path":"src/app.sh","symbol":"greet","line_start":1,"line_end":3}]"#]);
        p.claim(&["test_planned", "--criterion_id", "c1", "--file_path", "tests/test_c1.sh", "--will", "create"]);
        p.claim(&["test_stack_detected", "--project_root", ".", "--test_stack", "sh", "--test_command", "sh tests/test_c1.sh"]);
        p.claim(&["criterion_head", "--criterion_id", "c1"]);
        p.write("src/app.sh", "#!/bin/sh\ngreet() { echo hey; }\ngreet\n");
        p.claim(&["criterion_assigned", "--criterion_id", "c1"]);
        let out = p.claim(&["fix_location_category", "--criterion_id", "c1", "--category", "this_repository", "--where", ""]);
        assert!(out.contains("familiar-ai: a question for you · \"greet says hello\""), "{out}");
        p
    }
}

#[test]
fn the_agents_question_becomes_the_card_and_the_persons_pick_is_recorded() {
    let p = Project::moved("moved");
    // the agent's own question is refused: ask again
    let pre = p.hook_fixture("PreToolUse", "pre_tool_use_ask_user_question", json!({}));
    assert_eq!(pre["hookSpecificOutput"]["permissionDecision"], "deny", "{pre}");
    // its prose with the pieces: asked as the card
    let pre = p.hook_fixture("PreToolUse", "pre_tool_use_ask_user_question", asking(PROSE));
    let hso = &pre["hookSpecificOutput"];
    assert_eq!(hso["permissionDecision"], "ask", "an allow skips the picker (the live capture): {pre}");
    assert_eq!(hso["updatedInput"].to_string(), CARD);
    assert!(hso["updatedInput"].get("answers").is_none(), "the person answers, never the hook");

    // the stop is blocked with the card's facts after the hold's words
    let stop = p.hook("Stop", json!({}));
    assert_eq!(stop["decision"], "block");
    assert!(stop["reason"].as_str().unwrap_or("").ends_with(FACTS), "{stop}");
    // the person's prompt carries the facts too
    let prompt = p.hook("UserPromptSubmit", json!({ "prompt": "what now?" }));
    assert!(prompt["hookSpecificOutput"]["additionalContext"].as_str().unwrap_or("").contains(FACTS), "{prompt}");

    // the person picks in the picker: recorded as theirs
    let card: Value = serde_json::from_str(CARD).expect("card");
    let post = p.picked(&card, "carry on with part 1 on the code as it is now");
    assert_eq!(post["systemMessage"].as_str().unwrap_or("").lines().next(), Some("familiar-ai: recorded your answer for \"greet says hello\": keep going as planned"), "{post}");
    assert!(post["hookSpecificOutput"]["additionalContext"].as_str().unwrap_or("").contains("familiar-ai: the person answered the question for \"greet says hello\": keep going as planned"), "{post}");
    let status = p.record();
    assert!(status.contains("familiar-ai: \"greet says hello\" · the test comes first"), "{status}");
    assert!(!status.contains("a question for you"), "the card is closed: {status}");
    assert_eq!(p.card_answers(), [json!({ "criterion_id": "c1", "hold": "anchor_moved", "choice": "keep_going", "typed": "" })]);

    // the same pick again: no card open, nothing written
    let again = p.picked(&card, "drop it");
    assert!(again.get("systemMessage").is_none(), "{again}");
    assert_eq!(p.card_answers().len(), 1, "written once");
    let pre = p.hook_fixture("PreToolUse", "pre_tool_use_ask_user_question", asking(PROSE));
    assert_eq!(pre, json!({}), "no card open: untouched");
}

#[test]
fn something_typed_is_the_persons_words_and_another_question_is_not_the_card() {
    let p = Project::moved("typed");
    let card: Value = serde_json::from_str(CARD).expect("card");
    // the agent's own question, header "x": nothing written
    let own = json!({ "questions": [{ "question": "Continue?", "header": "x", "options": [], "multiSelect": false }], "answers": { "Continue?": "release it" } });
    p.hook_fixture("PostToolUse", "post_tool_use_ask_user_question_picked", json!({ "tool_input": own, "tool_response": { "answers": { "Continue?": "release it" } } }));
    assert!(p.record().contains("a question for you"), "still open");
    assert!(p.card_answers().is_empty());
    // "something else" with nothing typed: the question stays open
    let none = p.picked(&card, "something else");
    assert!(none["hookSpecificOutput"]["additionalContext"].as_str().unwrap_or("").contains("typed nothing; nothing is recorded and the question stays open"), "{none}");
    // typed under "Type something.": recorded as typed
    let typed = p.picked(&card, "run the old test first");
    assert_eq!(typed["systemMessage"].as_str().unwrap_or("").lines().next(), Some("familiar-ai: recorded your answer for \"greet says hello\": \"run the old test first\""), "{typed}");
    assert_eq!(p.card_answers(), [json!({ "criterion_id": "c1", "hold": "anchor_moved", "choice": "typed", "typed": "run the old test first" })]);
}

#[test]
fn the_typed_card_records_the_command_through_the_claims_door() {
    let p = project("stack");
    p.claim(&["intent_submitted", "--for", "0", "--text", "greet with hello"]);
    p.claim(&["behavior", "--behavior_id", "b1", "--text", "the greeting"]);
    p.claim(&["promise", "--promise_id", "p1", "--behavior_id", "b1", "--accepts", "a name", "--gives_back", "a greeting", "--refuses", "nothing", "--cases", r#"[{"case_id":"n1","kind":"normal","given":"a name","expect":"a greeting"},{"case_id":"o1","kind":"opposite","given":"no name","expect":"no greeting"}]"#]);
    p.claim(&["criterion", "--criterion_id", "c1", "--text", "greet says hello", "--path", "full", "--behavior_id", "b1", "--promise_id", "p1"]);
    p.cases_kept("c1"); // the cases card passed (plan cases-card, 2026-10-25) [draft names]
    p.claim(&["place_chosen", "--criterion_id", "c1", "--places", r#"[{"file_path":"src/app.sh","symbol":"greet","line_start":1,"line_end":3}]"#]);
    p.claim(&["test_planned", "--criterion_id", "c1", "--file_path", "tests/test_c1.sh", "--will", "create"]);
    p.claim(&["criterion_head", "--criterion_id", "c1"]);
    p.claim(&["criterion_assigned", "--criterion_id", "c1"]);
    p.claim(&["fix_location_category", "--criterion_id", "c1", "--category", "this_repository", "--where", ""]);
    // the card names the agent's own way through: test_stack_detected needs no answer from the person
    let shown = p.record();
    let card_line = shown.lines().position(|l| l.starts_with("familiar-ai: a question for you · \"greet says hello\": no test command is known")).expect("the card is shown");
    let own = shown.lines().nth(card_line + 1).unwrap_or("");
    assert_eq!(own, "  or the agent detects it: test_stack_detected is its own claim (the plan's line 7, the command in the claim it: line); no answer from the person is needed", "{shown}");
    assert!(!shown.contains("tests/test_c1.py") || shown.contains("claim it:"), "the card itself carries no example command to paste: {shown}");
    // the question is the agent's prose
    let typed = "you asked: \"greet with hello\"\n\nthe agent split that into one part:\n  1  greet says hello\n\nwhat happened: the agent found no command that runs the tests for part 1. In the agent's words: \"there is no test runner in this project yet\"\n\nwhat now?";
    let pre = p.hook_fixture("PreToolUse", "pre_tool_use_ask_user_question", asking(typed));
    let card = pre["hookSpecificOutput"]["updatedInput"].clone();
    assert_eq!(
        card.to_string(),
        json!({ "questions": [{ "question": typed, "header": "familiar-ai", "options": [{ "label": "type the command", "description": "type the command that runs the tests in the box below; it is recorded as the test command" }], "multiSelect": false }] }).to_string()
    );
    // a command naming a test file neither in the tree nor planned: the door refuses, nothing written
    let refused = p.picked(&card, "sh tests/nope.sh");
    assert!(refused["systemMessage"].as_str().unwrap_or("").contains("not recorded:"), "{refused}");
    // the planned file: recorded as the test command
    let post = p.picked(&card, "sh tests/test_c1.sh");
    assert_eq!(post["systemMessage"].as_str().unwrap_or("").lines().next(), Some("familiar-ai: recorded your answer for \"greet says hello\": the tests run with `sh tests/test_c1.sh`"), "{post}");
    let stacks = p.facts_of("test_stack_detected");
    assert_eq!(stacks.len(), 1, "the refused command wrote nothing: {stacks:?}");
    assert_eq!((&stacks[0]["project_root"], &stacks[0]["test_stack"], &stacks[0]["test_command"]), (&json!(""), &json!("sh"), &json!("sh tests/test_c1.sh")));
    assert!(p.card_answers().is_empty(), "the typed card writes what the model needs, not an answer");
    assert!(p.record().contains("familiar-ai: the plan is confirmed"), "{}", p.record());
}
