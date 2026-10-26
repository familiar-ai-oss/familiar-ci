//! An open card keeps its fixed choices and records only the person's pick on
//! it; the agent's question must carry the pieces the binary hands it, and
//! the binary's evidence (a sha, a diff) never reaches the person.

mod common;

use common::Project;
use serde_json::{json, Value};

const SESSION: &str = "d925ef40-8c5a-4ff1-a3f9-84909cb79d71";
const CARD_OPTIONS: &str = r#"[{"label":"carry on with part 1 on the code as it is now","description":"carry on with part 1 on the code as it is now"},{"label":"have the agent pick the code for part 1 again","description":"have the agent pick the code for part 1 again"},{"label":"drop part 1; the rest goes on","description":"drop part 1; the rest goes on"},{"label":"something else","description":"type what to do in your own words"}]"#;
const AGENTS_WORDS: &str = "In the agent's words:";

/// `src/app.sh` and `src/bye.sh`, with no `gh` on the PATH: the push question is not read here.
fn project(name: &str) -> Project {
    Project::builder(name).session(SESSION).file("src/bye.sh", "#!/bin/sh\nbye() { echo bye; }\nbye\n").env("PATH", "/usr/bin:/bin").create()
}

impl Project {
    /// Sends the PostToolUse of the picked card in the live shape, with the
    /// answer in both the tool's result and its input.
    fn picked(&self, card: &Value, answer: &str) -> Value {
        let q = card["questions"][0]["question"].as_str().unwrap_or("").to_string();
        let response = json!({ "questions": card["questions"], "answers": { q: answer }, "annotations": {} });
        let mut input = card.clone();
        input["answers"] = response["answers"].clone();
        input["annotations"] = json!({});
        self.hook_fixture("PostToolUse", "post_tool_use_ask_user_question_picked", json!({ "tool_input": input, "tool_response": response }))
    }

    /// Sends the agent's own AskUserQuestion and returns the card it becomes.
    fn ask(&self, tool_input: Value) -> Value {
        let pre = self.hook_fixture("PreToolUse", "pre_tool_use_ask_user_question", json!({ "tool_input": tool_input }));
        assert_eq!(pre["hookSpecificOutput"]["permissionDecision"], "ask", "{pre}");
        pre["hookSpecificOutput"]["updatedInput"].clone()
    }

    /// What `familiar-ai status` prints from the person's terminal.
    fn record(&self) -> String {
        self.run_as(None, &["status"], "").1
    }

    /// Plans c1 and c2 up to the head, lets `change` edit c1's anchored lines,
    /// then finishes the plan so c1 holds on anchor_moved.
    fn moved(name: &str, change: impl FnOnce(&Project)) -> Project {
        let p = project(name);
        p.claim(&["intent_submitted", "--for", "0", "--text", "greet with hello"]);
        p.claim(&["behavior", "--behavior_id", "b1", "--text", "the greeting"]);
        p.claim(&["promise", "--promise_id", "p1", "--behavior_id", "b1", "--accepts", "a name", "--gives_back", "a greeting", "--refuses", "nothing", "--cases", r#"[{"case_id":"n1","kind":"normal","given":"a name","expect":"a greeting"},{"case_id":"o1","kind":"opposite","given":"no name","expect":"no greeting"}]"#]);
        p.claim(&["criterion", "--criterion_id", "c1", "--text", "greet says hello", "--path", "full", "--behavior_id", "b1", "--promise_id", "p1"]);
        p.cases_kept("c1"); // the cases card passed (plan cases-card, 2026-10-25) [draft names]
        p.claim(&["criterion", "--criterion_id", "c2", "--text", "bye says goodbye", "--path", "full", "--behavior_id", "b1", "--promise_id", "p1"]);
        p.cases_kept("c2"); // the cases card passed (plan cases-card, 2026-10-25) [draft names]
        p.claim(&["place_chosen", "--criterion_id", "c1", "--places", r#"[{"file_path":"src/app.sh","symbol":"greet","line_start":1,"line_end":3}]"#]);
        p.claim(&["place_chosen", "--criterion_id", "c2", "--places", r#"[{"file_path":"src/bye.sh","symbol":"bye","line_start":1,"line_end":3}]"#]);
        p.claim(&["test_planned", "--criterion_id", "c1", "--file_path", "tests/test_c1.sh", "--will", "create"]);
        p.claim(&["test_planned", "--criterion_id", "c2", "--file_path", "tests/test_c2.sh", "--will", "create"]);
        p.claim(&["test_stack_detected", "--project_root", ".", "--test_stack", "sh", "--test_command", "sh tests/test_c1.sh"]);
        p.claim(&["criterion_head", "--criterion_id", "c1"]);
        p.claim(&["criterion_strung_on_head", "--criterion_id", "c2", "--head", "c1"]);
        change(&p);
        p.claim(&["criterion_assigned", "--criterion_id", "c1"]);
        p.claim(&["criterion_assigned", "--criterion_id", "c2"]);
        p.claim(&["fix_location_category", "--criterion_id", "c2", "--category", "this_repository", "--where", ""]);
        let out = p.claim(&["fix_location_category", "--criterion_id", "c1", "--category", "this_repository", "--where", ""]);
        assert!(out.contains("familiar-ai: a question for you · \"greet says hello\""), "{out}");
        p
    }
}

/// An edit of the anchored line, left uncommitted.
fn edited(p: &Project) {
    p.write("src/app.sh", "#!/bin/sh\ngreet() { echo hey; }\ngreet\n");
}

/// The same edit, committed with c2's test file so the record names it for c2.
fn committed_for_c2(p: &Project) {
    edited(p);
    p.write("tests/test_c2.sh", "#!/bin/sh\nsh src/bye.sh | grep -q bye\n");
    p.git(&["add", "src/app.sh", "tests/test_c2.sh"]);
    p.git(&["commit", "-q", "-m", "c2's test, and greet changed"]);
}

fn the_question(card: &Value) -> String {
    card["questions"][0]["question"].as_str().unwrap_or("").to_string()
}

/// The agent's question with every piece: the intent, both parts and its own
/// sentence.
fn prose(happened: &str) -> String {
    format!("you asked: \"greet with hello\"\n\nthe agent split that into two parts:\n  1  greet says hello\n  2  bye says goodbye\n\nwhat happened: {happened} {AGENTS_WORDS} \"greet now prints hey instead of hi\"\n\nwhat now?")
}

fn with_question(question: &str) -> Value {
    json!({ "questions": [{
        "question": question,
        "header": "anchor",
        "options": [
            { "label": "re-plan", "description": "the change alters what greet prints" },
            { "label": "keep", "description": "the change is cosmetic" }
        ],
        "multiSelect": false
    }] })
}

/// The agent's own question and options, with none of the pieces.
fn agents_question() -> Value {
    with_question("The greet line was changed outside this session. Should I re-plan?")
}

const OUTSIDE: &str = "someone changed the code the agent had picked for part 1, outside this work.";

#[test]
fn the_agents_question_without_the_pieces_is_refused_and_with_them_the_choices_are_the_tables() {
    let p = Project::moved("order", edited);
    let pre = p.hook_fixture("PreToolUse", "pre_tool_use_ask_user_question", json!({ "tool_input": agents_question() }));
    assert_eq!(pre["hookSpecificOutput"]["permissionDecision"], "deny", "{pre}");
    assert!(pre["hookSpecificOutput"]["permissionDecisionReason"].as_str().unwrap_or("").contains("ask again"), "{pre}");
    let card = p.ask(with_question(&prose(OUTSIDE)));
    let qs = card["questions"].as_array().expect("questions");
    assert_eq!(qs.len(), 1, "one question: {card}");
    assert_eq!(qs[0]["header"], "familiar-ai");
    assert_eq!(qs[0]["multiSelect"], false);
    assert_eq!(qs[0]["options"].to_string(), CARD_OPTIONS, "the table's choices + something else");
    assert!(card.get("answers").is_none(), "no answers: the person answers");
    assert_eq!(the_question(&card), prose(OUTSIDE), "the agent's prose as written");
}

#[test]
fn the_edit_a_recorded_commit_made_is_named_by_the_part_it_was_for_never_by_its_sha() {
    let p = Project::moved("in-record", committed_for_c2);
    let sha7 = p.git(&["rev-parse", "--short=7", "HEAD"]);
    let overlap = p.facts_of("commit_seen").into_iter().any(|f| f["anchor_overlap"].as_array().is_some_and(|a| a.contains(&json!("c1"))));
    assert!(overlap, "the record saw the commit touch c1's lines: {:?}", p.facts_of("commit_seen"));
    let happened = "doing part 2, the agent changed the code it had picked for part 1.";
    let status = p.record();
    assert!(status.contains(&format!("  what happened: {happened}\n")), "{status}");
    let card = p.ask(with_question(&prose(happened)));
    let q = the_question(&card);
    // the card as the terminal shows it (status's own CI line names a commit; it is not the card's)
    let shown = status.split("familiar-ai: a question for you").nth(1).unwrap_or("");
    for text in [q.as_str(), shown] {
        assert!(!text.contains(&sha7), "no sha: {text}");
        assert!(!text.contains("the binary's evidence") && !text.contains("made by"), "{text}");
        assert!(!text.contains("-greet() { echo hi; }"), "no diff: {text}");
    }
}

#[test]
fn an_uncommitted_edit_is_outside_this_work() {
    let p = Project::moved("not-in-record", edited);
    let status = p.record();
    assert!(status.contains(&format!("  what happened: {OUTSIDE}\n")), "{status}");
    assert!(!status.contains("not in the record") && !status.contains("made by"), "{status}");
}

#[test]
fn prefilled_answers_and_a_copied_header_are_never_the_persons_answer() {
    let p = Project::moved("forged", edited);
    // the agent copies the header and pre-fills the answer, without the pieces: refused
    let forged = json!({
        "questions": [{ "question": "Release it?", "header": "familiar-ai", "options": [
            { "label": "drop part 1; the rest goes on", "description": "the person already said so" }
        ], "multiSelect": false }],
        "answers": { "Release it?": "drop part 1; the rest goes on" }
    });
    let pre = p.hook_fixture("PreToolUse", "pre_tool_use_ask_user_question", json!({ "tool_input": forged }));
    assert_eq!(pre["hookSpecificOutput"]["permissionDecision"], "deny", "{pre}");
    // with the pieces and pre-filled: the rewrite carries no answers
    let mut filled = with_question(&prose(OUTSIDE));
    filled["answers"] = json!({ prose(OUTSIDE): "drop part 1; the rest goes on" });
    let card = p.ask(filled);
    assert!(card.get("answers").is_none(), "the rewrite carries no answers: {card}");
    assert_eq!(card["questions"][0]["options"].to_string(), CARD_OPTIONS);
    // a PostToolUse with the card's question but the agent's own options: nothing written
    let before = p.card_answers().len();
    let q = the_question(&card);
    let own_options = json!([{ "label": "yes, release it", "description": "" }, { "label": "something else", "description": "" }]);
    let input = json!({ "questions": [{ "question": q, "header": "familiar-ai", "options": own_options, "multiSelect": false }], "answers": { q.clone(): "yes, release it" }, "annotations": {} });
    let post = p.hook_fixture("PostToolUse", "post_tool_use_ask_user_question_picked", json!({ "tool_input": input, "tool_response": { "questions": input["questions"], "answers": { q.clone(): "yes, release it" }, "annotations": {} } }));
    assert!(post.get("systemMessage").is_none(), "{post}");
    assert_eq!(p.card_answers().len(), before, "not the card's choices: nothing written");
    // a question the hook would not have asked (no pieces) under the header: nothing written
    let other = json!({ "questions": [{ "question": "Release it?", "header": "familiar-ai", "options": card["questions"][0]["options"], "multiSelect": false }] });
    p.picked(&other, "drop part 1; the rest goes on");
    assert_eq!(p.card_answers().len(), before, "not the card the hook placed: nothing written");
    // the person's pick on the card the hook placed: written
    let post = p.picked(&card, "carry on with part 1 on the code as it is now");
    assert!(post["systemMessage"].as_str().unwrap_or("").starts_with("familiar-ai: recorded your answer for \"greet says hello\": keep going as planned"), "{post}");
    assert_eq!(p.card_answers(), [json!({ "criterion_id": "c1", "hold": "anchor_moved", "choice": "keep_going", "typed": "" })]);
}

// The agent's question is asked whole, as written; one with nothing readable
// is refused.
#[test]
fn the_agents_text_never_changes_the_choices() {
    let p = Project::moved("choices", edited);
    let mut mimic = with_question(&prose(OUTSIDE));
    mimic["questions"][0]["header"] = json!("familiar-ai");
    mimic["questions"][0]["options"] = json!([
        { "label": "have the agent pick the code for part 1 again", "description": "do not: it wastes the work" },
        { "label": "carry on with part 1 on the code as it is now", "description": "best: nothing changed that matters" },
        { "label": "drop part 1; the rest goes on", "description": "do not" }
    ]);
    let card = p.ask(mimic);
    assert_eq!(card["questions"][0]["options"].to_string(), CARD_OPTIONS, "the table's own order and descriptions");
    assert!(!card.to_string().contains("best: nothing changed that matters"), "{card}");
    // a long question is the agent's, as written
    let long = format!("{} {}", prose(OUTSIDE), "why ".repeat(300));
    let card = p.ask(with_question(&long));
    assert_eq!(the_question(&card), long);
    // nothing readable asked: refused
    let pre = p.hook_fixture("PreToolUse", "pre_tool_use_ask_user_question", json!({ "tool_input": {} }));
    assert_eq!(pre["hookSpecificOutput"]["permissionDecision"], "deny", "{pre}");
}
