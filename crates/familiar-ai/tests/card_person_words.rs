//! A card is asked in the person's terms: their ask, its parts, what happened,
//! the agent's own sentence and the choices. The agent writes the prose; the
//! binary checks the pieces are there and refuses a question missing one.

mod common;

use common::Project;
use serde_json::{json, Value};

const SESSION: &str = "d925ef40-8c5a-4ff1-a3f9-84909cb79d71";
const ASKED: &str = "fix the greeting so it says hello, and make bye say goodbye";
const HAPPENED: &str = "what happened: doing part 2, the agent changed the code it had picked for part 1.";
const KEEP: &str = "carry on with part 1 on the code as it is now";
const AGAIN: &str = "have the agent pick the code for part 1 again";
const DROP: &str = "drop part 1; the rest goes on";

/// `src/app.sh` and `src/bye.sh`, in UTC, with no `gh` on the PATH: the push question is not read here.
fn project(name: &str) -> Project {
    Project::builder(name).session(SESSION).file("src/bye.sh", "#!/bin/sh\nbye() { echo bye; }\nbye\n").env("PATH", "/usr/bin:/bin").env("TZ", "UTC").create()
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

    /// The agent's AskUserQuestion, as the hook answers it.
    fn pre(&self, tool_input: Value) -> Value {
        self.hook_fixture("PreToolUse", "pre_tool_use_ask_user_question", json!({ "tool_input": tool_input }))
    }

    /// The agent's AskUserQuestion: the card it comes back as.
    fn ask(&self, tool_input: Value) -> Value {
        let pre = self.pre(tool_input);
        assert_eq!(pre["hookSpecificOutput"]["permissionDecision"], "ask", "{pre}");
        pre["hookSpecificOutput"]["updatedInput"].clone()
    }

    /// What `familiar-ai status` prints from the person's terminal.
    fn terminal(&self) -> String {
        self.run_as(None, &["status"], "").1
    }

    /// The person asks, the agent splits it into c1 (src/app.sh 1-3) and c2
    /// (src/bye.sh), c1's lines are edited, and c1 is held on anchor_moved.
    fn moved(name: &str, change: impl FnOnce(&Project)) -> Project {
        let p = project(name);
        p.hook_fixture("UserPromptSubmit", "user_prompt_submit", json!({ "prompt": ASKED }));
        let said = p.facts().iter().rev().find(|f| f.kind == "person_said").map(|f| f.t).expect("the prompt is recorded");
        p.claim(&["intent_submitted", "--for", &said.to_string(), "--text", "greet says hello; bye says goodbye"]);
        p.claim(&["one_reading", "--why", "the ask names what must change and how"]);
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

/// c1's lines edited and committed with c2's test file, a commit the record names for c2.
fn committed_for_c2(p: &Project) {
    p.write("src/app.sh", "#!/bin/sh\ngreet() { echo hey; }\ngreet\n");
    p.write("tests/test_c2.sh", "#!/bin/sh\nsh src/bye.sh | grep -q bye\n");
    p.git(&["add", "src/app.sh", "tests/test_c2.sh"]);
    p.git(&["commit", "-q", "-m", "c2's test, and greet changed"]);
}

/// The agent's question, as prose in the shape the agent is told.
fn prose() -> String {
    format!(
        "you asked (12:36): \"{ASKED}\"\n\nthe agent split that into two parts:\n  1  greet says hello          2  bye says goodbye\n\n{HAPPENED} In the agent's words: \"greet now prints hey instead of hi\"\n\nwhat now?"
    )
}

fn asking(question: &str) -> Value {
    json!({ "questions": [{ "question": question, "header": "greeting", "options": [
        { "label": "carry on", "description": "the change is cosmetic" },
        { "label": "re-plan", "description": "the change alters what greet prints" }
    ], "multiSelect": false }] })
}

fn the_question(card: &Value) -> String {
    card["questions"][0]["question"].as_str().unwrap_or("").to_string()
}

/// The rejected wording never reaches the person: no evidence lead, commit, sha or raw JSON.
fn clean(text: &str, shas: &[String]) {
    for rejected in ["the binary's evidence", "made by commit", "the agent says: "] {
        assert!(!text.contains(rejected), "{rejected:?} in:\n{text}");
    }
    for sha in shas {
        assert!(!text.contains(sha.as_str()), "the sha {sha} in:\n{text}");
    }
    assert!(!text.contains('{') && !text.contains('}') && !text.contains("\"kind\""), "JSON in:\n{text}");
}

/// Every sha7 the project's history has.
fn shas(p: &Project) -> Vec<String> {
    p.git(&["log", "--format=%h", "--abbrev=7"]).lines().map(str::to_string).collect()
}

#[test]
fn a_the_agents_prose_with_every_piece_is_asked_as_written_with_the_tables_choices() {
    let p = Project::moved("asked", committed_for_c2);
    let card = p.ask(asking(&prose()));
    let qs = card["questions"].as_array().expect("questions");
    assert_eq!(qs.len(), 1, "one question: {card}");
    assert_eq!(the_question(&card), prose(), "the agent's prose, as written");
    assert_eq!(qs[0]["header"], "familiar-ai");
    assert_eq!(qs[0]["multiSelect"], false);
    let labels: Vec<&str> = qs[0]["options"].as_array().expect("options").iter().filter_map(|o| o["label"].as_str()).collect();
    assert_eq!(labels, [KEEP, AGAIN, DROP, "something else"], "the table's choices in the person's world, something else last");
    for o in qs[0]["options"].as_array().expect("options").iter().take(3) {
        assert_eq!(o["description"], o["label"], "a choice's description is its own line: {o}");
    }
    assert!(card.get("answers").is_none(), "no answers: the person answers");
    clean(&the_question(&card), &shas(&p));
}

#[test]
fn b_a_question_missing_a_piece_is_refused_naming_it() {
    let p = Project::moved("refused", committed_for_c2);
    // the person's ask left out
    let without = prose().replace(&format!("\"{ASKED}\""), "your request");
    let pre = p.pre(asking(&without));
    let hso = &pre["hookSpecificOutput"];
    assert_eq!(hso["permissionDecision"], "deny", "{pre}");
    let reason = hso["permissionDecisionReason"].as_str().unwrap_or("");
    assert!(reason.starts_with("familiar-ai: ask again; your question must quote: "), "{reason}");
    assert!(reason.contains(&format!("\"{ASKED}\"")), "names the person's words: {reason}");
    assert!(!reason.contains("part 1 \"greet says hello\""), "the parts were there: {reason}");
    // a part left out: named
    let pre = p.pre(asking(&prose().replace("2  bye says goodbye", "2  the other one")));
    let reason = pre["hookSpecificOutput"]["permissionDecisionReason"].as_str().unwrap_or("").to_string();
    assert!(reason.contains("ask again") && reason.contains("part 2 \"bye says goodbye\""), "{reason}");
    // no sentence of the agent's: named
    let pre = p.pre(asking(&prose().replace(" In the agent's words: \"greet now prints hey instead of hi\"", "")));
    let reason = pre["hookSpecificOutput"]["permissionDecisionReason"].as_str().unwrap_or("").to_string();
    assert!(reason.contains("ask again") && reason.contains("In the agent's words:"), "{reason}");
    // nothing readable asked: refused the same way
    let pre = p.pre(json!({}));
    assert_eq!(pre["hookSpecificOutput"]["permissionDecision"], "deny", "{pre}");
    assert!(pre["hookSpecificOutput"]["permissionDecisionReason"].as_str().unwrap_or("").contains("ask again"), "{pre}");
    assert!(p.card_answers().is_empty());
}

#[test]
fn c_the_cards_facts_hand_the_agent_the_pieces_and_the_order_to_write_one_sentence() {
    let p = Project::moved("facts", committed_for_c2);
    let prompt = p.hook_fixture("UserPromptSubmit", "user_prompt_submit", json!({ "prompt": "what now?" }));
    let context = prompt["hookSpecificOutput"]["additionalContext"].as_str().unwrap_or("").to_string();
    // the prompt itself is told first, as any prompt during an open run is; the card's facts follow it
    assert!(context.starts_with("familiar-ai: the person said \"what now?\" ("), "{context}");
    let facts = context[context.find("familiar-ai: a question for the person is open").unwrap_or_else(|| panic!("the card's facts: {context}"))..].to_string();
    let at = facts.find("you asked (").unwrap_or_else(|| panic!("no 'you asked (': {facts}"));
    let after = &facts[at + "you asked (".len()..];
    let (clock, rest) = after.split_once(')').unwrap_or_else(|| panic!("{facts}"));
    let (h, m) = clock.split_once(':').unwrap_or_else(|| panic!("h:mm: {clock}"));
    assert!(!h.is_empty() && h.len() <= 2 && h.chars().all(|c| c.is_ascii_digit()) && m.len() == 2 && m.chars().all(|c| c.is_ascii_digit()), "h:mm: {clock}");
    assert!(rest.starts_with(&format!(": \"{ASKED}\"")), "the person's words verbatim: {facts}");
    assert!(facts.contains("the agent split that into two parts:"), "{facts}");
    assert!(facts.contains("1  greet says hello") && facts.contains("2  bye says goodbye"), "the parts: {facts}");
    assert!(facts.contains(HAPPENED), "{facts}");
    assert!(facts.contains("In the agent's words:") && facts.contains("one sentence"), "the order to write one sentence: {facts}");
    assert!(facts.contains("what the code it had picked for part 1 now does differently"), "what the sentence says: {facts}");
    assert!(facts.contains("what now?"), "{facts}");
    let order = ["you asked (", "the agent split that", HAPPENED, "In the agent's words:", "what now?"].map(|w| facts.find(w).unwrap_or(usize::MAX));
    assert!(order.windows(2).all(|w| w[0] < w[1]), "the five labels in order: {facts}");
    assert!(facts.contains("do not claim hold_answered yourself"), "{facts}");
}

#[test]
fn d_the_terminal_reads_the_template_in_full_sentences() {
    let p = Project::moved("terminal", committed_for_c2);
    let status = p.terminal();
    let start = status.find("familiar-ai: a question for you · \"greet says hello\"").unwrap_or_else(|| panic!("{status}"));
    let end = status[start..].find("  > or type your own answer").map(|i| start + i + "  > or type your own answer".len()).unwrap_or_else(|| panic!("{status}"));
    let card = &status[start..end];
    let lines: Vec<&str> = card.lines().collect();
    let from = lines.iter().position(|l| l.starts_with("  you asked (")).unwrap_or_else(|| panic!("{card}"));
    assert!(lines[from].ends_with(&format!("): \"{ASKED}\"")), "{card}");
    assert_eq!(
        lines[from + 1..].join("\n"),
        format!("  the agent split that into two parts:\n    1  greet says hello\n    2  bye says goodbye\n  {HAPPENED}\n  what now?\n    1) {KEEP}\n    2) {AGAIN}\n    3) {DROP}\n  > or type your own answer")
    );
    clean(card, &shas(&p));
    for word in ["criterion", "anchor", "hold", "planning", "string", "sha", "commit"] {
        assert!(!lines[from..].join("\n").contains(word), "{word:?} in:\n{card}");
    }
    // the stop's reason: the same pieces, in full sentences
    let stop = p.hook("Stop", json!({}));
    assert_eq!(stop["decision"], "block", "{stop}");
    let reason = stop["reason"].as_str().unwrap_or("").to_string();
    assert!(reason.contains(&format!("\"{ASKED}\"")) && reason.contains("1  greet says hello") && reason.contains(HAPPENED), "{reason}");
    clean(&reason, &shas(&p));
}

/// c1's state, its holds and whether it is released, as the rules derive
/// them from the record; and c2's state.
fn derived(p: &Project) -> (String, Vec<String>, bool, String) {
    let log = p.facts();
    let v = familiar_ai_rules::View::new(&log);
    use familiar_ai_rules::lifecycle;
    (lifecycle::state(&v, "c1").to_string(), lifecycle::whys(&v, "c1"), lifecycle::released(&v, "c1"), lifecycle::state(&v, "c2").to_string())
}

#[test]
fn e_each_pick_writes_its_choice_and_the_rules_derive_where_it_leads() {
    for (label, choice) in [(KEEP, "keep_going"), (AGAIN, "re_plan"), (DROP, "release")] {
        let p = Project::moved(&format!("pick-{choice}"), committed_for_c2);
        assert_eq!(derived(&p), ("held".to_string(), vec!["anchor_moved".to_string()], false, "waiting".to_string()));
        let card = p.ask(asking(&prose()));
        let post = p.picked(&card, label);
        assert!(post["systemMessage"].as_str().unwrap_or("").starts_with("familiar-ai: recorded your answer for \"greet says hello\""), "{post}");
        assert_eq!(p.card_answers(), [json!({ "criterion_id": "c1", "hold": "anchor_moved", "choice": choice, "typed": "" })]);
        let (state, whys, released, c2) = derived(&p);
        match choice {
            // the hold clears; part 1 waits for its test
            "keep_going" => assert_eq!((state.as_str(), whys.len(), released, c2.as_str()), ("testing", 0, false, "waiting")),
            // part 1's place no longer counts, so the plan is open again on that line
            "re_plan" => assert_eq!((state.as_str(), whys, released, c2.as_str()), ("unplanned", vec!["planning_incomplete".to_string()], false, "unplanned")),
            // released; part 2 starts
            _ => assert_eq!((released, c2.as_str()), (true, "testing")),
        }
        clean(post["systemMessage"].as_str().unwrap_or(""), &shas(&p));
    }
}

#[test]
fn e_a_pick_on_options_that_are_not_the_tables_writes_nothing() {
    let p = Project::moved("not-the-tables", committed_for_c2);
    let card = p.ask(asking(&prose()));
    let mut own = card.clone();
    own["questions"][0]["options"] = json!([{ "label": "re-plan", "description": "" }, { "label": "something else", "description": "" }]);
    p.picked(&own, "re-plan");
    assert!(p.card_answers().is_empty(), "not the table's choices: nothing written");
    let mut header = card.clone();
    header["questions"][0]["header"] = json!("greeting");
    p.picked(&header, KEEP);
    assert!(p.card_answers().is_empty(), "not the hook's header: nothing written");
    p.picked(&card, "something else");
    assert!(p.card_answers().is_empty(), "something else with nothing typed: open");
    p.picked(&card, "run the old test first");
    assert_eq!(p.card_answers(), [json!({ "criterion_id": "c1", "hold": "anchor_moved", "choice": "typed", "typed": "run the old test first" })]);
}

#[test]
fn f_an_edit_no_recorded_commit_made_is_outside_this_work() {
    let p = Project::moved("outside", |p| {
        p.write("src/app.sh", "#!/bin/sh\ngreet() { echo hey; }\ngreet\n");
    });
    let status = p.terminal();
    assert!(status.contains("  what happened: someone changed the code the agent had picked for part 1, outside this work.\n"), "{status}");
    let card = status.split("familiar-ai: a question for you").nth(1).and_then(|c| c.split("  > or type your own answer").next()).unwrap_or("");
    clean(card, &shas(&p));
}
