//! A part whose tests pass while no commit changed the code at its anchor is
//! held on `anchor_untouched`, a card the person answers. The stop is refused
//! with the card's words; on the bench (`claude -p`) it goes carrying the
//! card; the person's AskUserQuestion pick is recorded as `hold_answered`.
//! See docs/decisions.md#holds-are-the-persons

mod common;

use common::Project;
use serde_json::{json, Value};

const SESSION: &str = "5b0f7a3e-2d61-4c1e-9a8f-6e2b1c0d4f17";

const HAPPENED: &str = "the tests for part 1 pass, but none of the changes touched the code the agent had picked for it.";
const KEEP: &str = "keep the change for part 1 where it landed";
const AGAIN: &str = "have the agent pick the code for part 1 again";

/// What the agent is told at the stop: the pieces, the shape, and one sentence of its own.
fn facts() -> String {
    format!(
        "familiar-ai: a question for the person is open for part 1 of what they asked. Write your question to them as prose, in this shape and order:
  you asked: \"greet with hello\"
  the agent split that into one part:
    1  greet says hello
  what happened: {HAPPENED}
  In the agent's words: \"<one sentence of yours, in plain words: where the change for part 1 landed instead>\"
  what now?
Quote the person's words and each part's text exactly as above, write the one sentence yourself, and leave the choices to the hook. The record's card: \"greet says hello\": its tests pass, but no commit changed the code at src/app.sh greet. What now? Ask the person with AskUserQuestion: the hook turns that call into the question card, and their choice is recorded; do not claim hold_answered yourself."
    )
}

/// The card in the person's terms.
fn card() -> String {
    format!(
        "familiar-ai: a question for you · \"greet says hello\": its tests pass, but no commit changed the code at src/app.sh greet. What now?
  you asked: \"greet with hello\"
  the agent split that into one part:
    1  greet says hello
  what happened: {HAPPENED}
  what now?
    1) {KEEP}
    2) {AGAIN}
    3) drop it
  > or type your own answer"
    )
}

impl Project {
    /// The hook, on the bench (`FAMILIAR_AI_BENCH=1`) or not.
    fn hook_bench(&self, event: &str, rest: Value, bench: bool) -> Value {
        self.set_env("FAMILIAR_AI_BENCH", Some(if bench { "1" } else { "0" }));
        let out = self.hook(event, rest);
        self.set_env("FAMILIAR_AI_BENCH", Some("0"));
        out
    }

    /// c1 sealed red on src/app.sh greet 2–2, then fixed beside its anchor, its tests passing.
    fn untouched(name: &str) -> Project {
        let p = Project::builder(name).session(SESSION).env("FAMILIAR_AI_BENCH", "0").env("PATH", "/usr/bin:/bin").create();
        p.write("src/app.sh", "#!/bin/sh\ngreet() { echo hi; }\ngreet\n");
        p.write("src/other.sh", "echo other\n");
        p.git(&["add", "-A"]);
        p.git(&["commit", "-q", "-m", "a second file"]);
        p.claim(&["intent_submitted", "--for", "0", "--text", "greet with hello"]);
        p.claim(&["behavior", "--behavior_id", "b1", "--text", "the greeting"]);
        p.claim(&["promise", "--promise_id", "p1", "--behavior_id", "b1", "--accepts", "a name", "--gives_back", "a greeting", "--refuses", "nothing", "--cases", r#"[{"case_id":"n1","kind":"normal","given":"a name","expect":"a greeting"},{"case_id":"o1","kind":"opposite","given":"no name","expect":"no greeting"}]"#]);
        p.claim(&["criterion", "--criterion_id", "c1", "--text", "greet says hello", "--path", "full", "--behavior_id", "b1", "--promise_id", "p1"]);
        p.cases_kept("c1"); // the cases card passed (plan cases-card, 2026-10-25) [draft names]
        p.claim(&["place_chosen", "--criterion_id", "c1", "--places", r#"[{"file_path":"src/app.sh","symbol":"greet","line_start":2,"line_end":2}]"#]);
        p.claim(&["test_planned", "--criterion_id", "c1", "--file_path", "tests/test_c1.sh", "--will", "create"]);
        p.claim(&["test_stack_detected", "--project_root", ".", "--test_stack", "sh", "--test_command", "sh tests/test_c1.sh"]);
        p.claim(&["criterion_head", "--criterion_id", "c1"]);
        p.claim(&["criterion_assigned", "--criterion_id", "c1"]);
        let out = p.claim(&["fix_location_category", "--criterion_id", "c1", "--category", "this_repository", "--where", ""]);
        assert!(out.contains("familiar-ai: the plan is confirmed"), "{out}");
        p.write("tests/test_c1.sh", "grep -q hello src/app.sh\n");
        p.git(&["add", "-A"]);
        p.git(&["commit", "-q", "-m", "c1: tests first"]);
        p.hook("PostToolUse", json!({ "tool_name": "Bash", "tool_input": { "command": "git add -A && git commit -m 'c1: tests first'" }, "tool_response": {} }));
        p.write("src/app.sh", "#!/bin/sh\ngreet() { echo hi; }\ngreet; echo hello\n");
        p.write("src/other.sh", "echo other, too\n");
        p.git(&["add", "-A"]);
        p.git(&["commit", "-q", "-m", "c1: the fix, beside the anchor"]);
        let out = p.hook("PostToolUse", json!({ "tool_name": "Bash", "tool_input": { "command": "git commit -am 'c1: the fix'" }, "tool_response": {} }));
        let context = out["hookSpecificOutput"]["additionalContext"].as_str().unwrap_or("");
        assert!(context.contains("the change landed outside the place the plan named"), "{context}");
        p
    }
}

#[test]
fn a_stop_while_held_on_anchor_untouched_is_refused_with_the_cards_words() {
    let p = Project::untouched("refused");
    let out = p.hook_bench("Stop", json!({}), false);
    assert_eq!(out["decision"], "block", "{out}");
    let reason = out["reason"].as_str().unwrap_or("");
    assert!(reason.starts_with("familiar-ai: a question for the person is open: ask them now with AskUserQuestion; the hook turns that call into the question card\n"), "{reason}");
    assert!(reason.contains("the person answers its card: plan it again, keep going as planned or release it"), "{reason}");
    assert!(reason.contains(&facts()), "{reason}");
    assert!(!reason.contains("familiar-ai claim hold_answered"), "{reason}");
}

#[test]
fn on_the_bench_the_stop_goes_carrying_the_card() {
    let p = Project::untouched("bench");
    for event in ["Stop", "SubagentStop"] {
        let out = p.hook_bench(event, json!({}), true);
        assert!(out.get("decision").is_none(), "{event}: the stop goes: {out}");
        // Claude Code continues the turn on a stop's additionalContext: the card goes to the person only
        assert!(out.get("hookSpecificOutput").is_none(), "{event}: {out}");
        assert!(out["systemMessage"].as_str().unwrap_or("").contains(&card()), "{event}: {out}");
    }
}

#[test]
fn the_persons_pick_in_the_picker_is_recorded_as_hold_answered() {
    let p = Project::untouched("picked");
    let prose = format!(
        "you asked: \"greet with hello\"\n\nthe agent split that into one part:\n  1  greet says hello\n\nwhat happened: {HAPPENED} In the agent's words: \"the hello went on the line after greet, not inside it\"\n\nwhat now?"
    );
    let ask = json!({ "tool_input": { "questions": [{ "question": prose, "header": "x", "options": [
        { "label": "yes", "description": "Continue" }, { "label": "no", "description": "Do not continue" }
    ], "multiSelect": false }] } });
    let pre = p.hook_fixture("PreToolUse", "pre_tool_use_ask_user_question", ask);
    let hso = &pre["hookSpecificOutput"];
    assert_eq!(hso["permissionDecision"], "ask", "{pre}");
    let card = hso["updatedInput"].clone();
    let labels: Vec<&str> = card["questions"][0]["options"].as_array().into_iter().flatten().filter_map(|o| o["label"].as_str()).collect();
    assert_eq!(labels, [KEEP, AGAIN, "drop it", "something else"], "{card}");

    let q = card["questions"][0]["question"].as_str().unwrap_or("").to_string();
    let response = json!({ "questions": card["questions"], "answers": { q: KEEP }, "annotations": {} });
    let mut input = card.clone();
    input["answers"] = response["answers"].clone();
    input["annotations"] = json!({});
    let post = p.hook_fixture("PostToolUse", "post_tool_use_ask_user_question_picked", json!({ "tool_input": input, "tool_response": response }));
    assert!(post["systemMessage"].as_str().unwrap_or("").starts_with("familiar-ai: recorded your answer for \"greet says hello\": "), "{post}");
    assert_eq!(p.card_answers(), [json!({ "criterion_id": "c1", "hold": "anchor_untouched", "choice": "keep_going", "typed": "" })]);
    let status = p.run_as(None, &["status"], "").1;
    assert!(!status.contains("a question for you"), "the card is closed: {status}");
    assert!(status.contains("familiar-ai: \"greet says hello\" · locally green"), "keep going: the change where it landed is the part's: {status}");
}
