//! A card open in another checkout reaches the person's session, whatever
//! the switch of the checkout that session is in: `status --json` lists it
//! with that checkout's folder, the status line names it, the agent is told
//! to ask it at the prompt and the session's start, the person's own
//! AskUserQuestion with the `familiar-ai` header becomes the card, and the
//! pick is written to that checkout's record, not this one's. A subagent's
//! question is not the person's. A headless lane's stop with only the
//! person's card left goes, leaving the card in its record. The band draws
//! the same `status --json`; its drawing is tested in plugin/tests.

mod common;

use std::path::{Path, PathBuf};

use common::Project;
use familiar_ai_facts::Record;
use serde_json::{json, Value};

const LANE: &str = "9b2c4d6e-1f3a-4b5c-8d7e-0a1b2c3d4e5f";
const PERSON: &str = "3e5f7a9b-2c4d-4e6f-9a1b-3c5d7e9f1a2b";
const PROSE: &str = "you asked: \"greet with hello\"\n\nthe agent split that into one part:\n  1  greet says hello\n\nwhat happened: someone changed the code the agent had picked for part 1, outside this work. In the agent's words: \"greet now prints hey instead of hi\"\n\nwhat now?";
const CARD: &str = r#"{"questions":[{"question":"you asked: \"greet with hello\"\n\nthe agent split that into one part:\n  1  greet says hello\n\nwhat happened: someone changed the code the agent had picked for part 1, outside this work. In the agent's words: \"greet now prints hey instead of hi\"\n\nwhat now?","header":"familiar-ai","options":[{"label":"carry on with part 1 on the code as it is now","description":"carry on with part 1 on the code as it is now"},{"label":"have the agent pick the code for part 1 again","description":"have the agent pick the code for part 1 again"},{"label":"drop it","description":"drop it"},{"label":"something else","description":"type what to do in your own words"}],"multiSelect":false}]}"#;
const TOLD: &str = "familiar-ai: in the checkout lane, a question for the person is open for part 1 of what they asked. Write your question to them as prose, in this shape and order:
  you asked: \"greet with hello\"
  the agent split that into one part:
    1  greet says hello
  what happened: someone changed the code the agent had picked for part 1, outside this work.
  In the agent's words: \"<one sentence of yours, in plain words: what the code it had picked for part 1 now does differently>\"
  what now?
Quote the person's words and each part's text exactly as above, write the one sentence yourself, and leave the choices to the hook. The record's card: \"greet says hello\": the code at src/app.sh greet changed after the plan was made. What now? Ask the person with AskUserQuestion: the hook turns that call into the question card, and their choice is recorded; do not claim hold_answered yourself. Give the question the header familiar-ai; the answer is recorded in the checkout lane.";
const CARD_TEXT: &str = "familiar-ai: a question for you · \"greet says hello\": the code at src/app.sh greet changed after the plan was made. What now?\n  you asked: \"greet with hello\"\n  the agent split that into one part:\n    1  greet says hello\n  what happened: someone changed the code the agent had picked for part 1, outside this work.\n  what now?\n    1) carry on with part 1 on the code as it is now\n    2) have the agent pick the code for part 1 again\n    3) drop it\n  > or type your own answer";

/// main set up and switched off by the person; no `gh` on the PATH.
fn project(name: &str) -> Project {
    let p = Project::builder(name).no_session().env("PATH", "/usr/bin:/bin").env("FAMILIAR_AI_BENCH", "0").create();
    assert_eq!(p.run(&["off"], "").0, 0);
    p
}

impl Project {
    /// A worktree on branch `lane`, set up and switched on by the person, its
    /// plan confirmed after an outside edit of the anchor: c1 is held on
    /// anchor_moved, its card open, held by the lane's session. Definition change (the owner,
    /// 2026-10-15, plan §6.8): only a checkout with a live or resumable agent has its cards
    /// relayed, so the lane carries the session file `launch_lane` writes.
    fn lane_with_a_card(&self) -> PathBuf {
        let wt = self.dir.join("lane");
        self.git(&["worktree", "add", "-q", "-b", "lane", &wt.to_string_lossy()]);
        assert_eq!(self.run_in(&wt, &["setup"], "").0, 0);
        std::fs::create_dir_all(wt.join(".familiar/runs/lane")).expect("the lane's folder");
        std::fs::write(wt.join(".familiar/runs/lane/session"), LANE).expect("the lane's session file");
        assert_eq!(self.run_in(&wt, &["on"], "").0, 0);
        let claim = |args: &[&str]| {
            let mut argv = vec!["claim"];
            argv.extend_from_slice(args);
            let (code, out, err) = self.run_in(&wt, &argv, "");
            assert_eq!(code, 0, "claim {args:?}: {out}{err}");
            out
        };
        claim(&["intent_submitted", "--for", "0", "--text", "greet with hello"]);
        claim(&["behavior", "--behavior_id", "b1", "--text", "the greeting"]);
        claim(&["promise", "--promise_id", "p1", "--behavior_id", "b1", "--accepts", "a name", "--gives_back", "a greeting", "--refuses", "nothing", "--cases", r#"[{"case_id":"n1","kind":"normal","given":"a name","expect":"a greeting"},{"case_id":"o1","kind":"opposite","given":"no name","expect":"no greeting"}]"#]);
        claim(&["criterion", "--criterion_id", "c1", "--text", "greet says hello", "--path", "full", "--behavior_id", "b1", "--promise_id", "p1"]);
        common::cases_kept_at(&wt, "c1"); // the cases card passed (plan cases-card, 2026-10-25) [draft names]
        claim(&["place_chosen", "--criterion_id", "c1", "--places", r#"[{"file_path":"src/app.sh","symbol":"greet","line_start":1,"line_end":3}]"#]);
        claim(&["test_planned", "--criterion_id", "c1", "--file_path", "tests/test_c1.sh", "--will", "create"]);
        claim(&["test_stack_detected", "--project_root", ".", "--test_stack", "sh", "--test_command", "sh tests/test_c1.sh"]);
        claim(&["criterion_head", "--criterion_id", "c1"]);
        std::fs::write(wt.join("src/app.sh"), "#!/bin/sh\ngreet() { echo hey; }\ngreet\n").expect("outside edit");
        claim(&["criterion_assigned", "--criterion_id", "c1", "--agent_conversation_id", LANE]);
        let out = claim(&["fix_location_category", "--criterion_id", "c1", "--category", "this_repository", "--where", ""]);
        assert!(out.contains("familiar-ai: a question for you · \"greet says hello\""), "{out}");
        wt
    }

    /// A hook at `at` as `session`, the payload's `cwd` there.
    fn hook_at(&self, at: &Path, session: &str, event: &str, fields: Value) -> Value {
        let mut payload = json!({ "hook_event_name": event, "session_id": session, "cwd": at.to_string_lossy() });
        for (k, v) in fields.as_object().cloned().unwrap_or_default() {
            payload[k] = v;
        }
        let (code, out, err) = self.run_in(at, &["hook", event], &payload.to_string());
        assert_eq!(code, 0, "{event}: {err}");
        serde_json::from_str(out.trim()).unwrap_or_else(|e| panic!("{event} printed no JSON ({e}): {out}"))
    }

    /// The person's session in main asking with `header` and `question`.
    fn asks_in_main(&self, header: &str, question: &str, more: Value) -> Value {
        let mut fields = json!({ "tool_name": "AskUserQuestion", "tool_input": { "questions": [{ "question": question, "header": header, "options": [
            { "label": "yes", "description": "Continue" }, { "label": "no", "description": "Do not continue" }
        ], "multiSelect": false }] } });
        for (k, v) in more.as_object().cloned().unwrap_or_default() {
            fields[k] = v;
        }
        self.hook_at(&self.repo(), PERSON, "PreToolUse", fields)
    }

    /// The person's pick of `answer` on the card, after the call, in main.
    fn picked_in_main(&self, answer: &str) -> Value {
        let card: Value = serde_json::from_str(CARD).expect("card");
        let q = card["questions"][0]["question"].as_str().unwrap_or("").to_string();
        let response = json!({ "questions": card["questions"], "answers": { q: answer }, "annotations": {} });
        let mut input = card.clone();
        input["answers"] = response["answers"].clone();
        self.hook_at(&self.repo(), PERSON, "PostToolUse", json!({ "tool_name": "AskUserQuestion", "tool_input": input, "tool_response": response }))
    }

    fn json_at(&self, at: &Path) -> Value {
        let (code, out, err) = self.run_in(at, &["status", "--json"], "");
        assert_eq!(code, 0, "{err}");
        serde_json::from_str(&out).expect("status json")
    }

    fn answers_at(&self, at: &Path) -> Vec<Value> {
        let record = Record::open_waiting(&at.join(".familiar/record"), std::time::Duration::from_secs(10)).expect("record");
        // the cases card's answer, which every full-path plan passes before its test, left out (plan cases-card, 2026-10-25) [draft names]
        record.read_by_kind("hold_answered").expect("read").into_iter().map(|f| Value::Object(f.fields)).filter(|a| a["hold"] != common::project::CASES_CARD).collect()
    }
}

#[test]
fn main_switched_off_sees_the_lanes_card_and_names_it() {
    let p = project("seen");
    p.lane_with_a_card();
    let main = p.json_at(&p.repo());
    assert_eq!(main["off"], json!(true));
    let lane = &main["checkouts"][0];
    assert_eq!(lane["folder"], "lane");
    assert_eq!(lane["counts"]["needs_you"], json!(1));
    let card = &lane["cards"][0];
    assert_eq!(card["criterion_id"], "c1");
    assert_eq!(card["name"], "part 1");
    assert_eq!(card["hold"], "anchor_moved");
    assert_eq!(p.run(&["statusline"], "").1, "repo | Familiar | OFF · lane: 1 needs you\n");
}

#[test]
fn the_persons_session_in_main_is_told_to_ask_and_its_question_becomes_the_lanes_card() {
    let p = project("asked");
    let wt = p.lane_with_a_card();

    // told at the prompt and at the session's start, though main is off
    let prompt = p.hook_at(&p.repo(), PERSON, "UserPromptSubmit", json!({ "prompt": "how is the lane doing?" }));
    assert_eq!(prompt["hookSpecificOutput"]["additionalContext"], TOLD, "{prompt}");
    let start = p.hook_at(&p.repo(), PERSON, "SessionStart", json!({ "source": "startup" }));
    let context = start["hookSpecificOutput"]["additionalContext"].as_str().unwrap_or("");
    assert!(context.starts_with("Familiar is turned off for this project. Every hook allows everything"), "{context}");
    assert!(context.ends_with(TOLD), "{context}");

    // a subagent's question, or one without the header, is left alone
    assert_eq!(p.asks_in_main("familiar-ai", PROSE, json!({ "agent_id": "a1b2c3", "agent_type": "general-purpose" })), json!({}));
    assert_eq!(p.asks_in_main("x", PROSE, json!({})), json!({}));
    // without the card's pieces: ask again
    let again = p.asks_in_main("familiar-ai", "what now?", json!({}));
    assert_eq!(again["hookSpecificOutput"]["permissionDecision"], "deny", "{again}");
    assert!(again["hookSpecificOutput"]["permissionDecisionReason"].as_str().unwrap_or("").starts_with("familiar-ai: ask again; your question must quote: "), "{again}");
    // the person's question with the pieces: the lane's card, asked
    let pre = p.asks_in_main("familiar-ai", PROSE, json!({}));
    let hso = &pre["hookSpecificOutput"];
    assert_eq!(hso["permissionDecision"], "ask", "{pre}");
    assert_eq!(hso["updatedInput"].to_string(), CARD);

    // a subagent's pick is not the person's
    let card: Value = serde_json::from_str(CARD).expect("card");
    let q = card["questions"][0]["question"].as_str().unwrap_or("").to_string();
    let response = json!({ "questions": card["questions"], "answers": { q: "drop it" }, "annotations": {} });
    p.hook_at(&p.repo(), PERSON, "PostToolUse", json!({ "tool_name": "AskUserQuestion", "agent_id": "a1b2c3", "tool_input": card, "tool_response": response }));
    assert!(p.answers_at(&wt).is_empty(), "a subagent's pick was recorded");

    // the person picks: written to the lane's record, not main's
    let post = p.picked_in_main("carry on with part 1 on the code as it is now");
    assert_eq!(post["systemMessage"], "familiar-ai: recorded your answer for \"greet says hello\": keep going as planned · in the checkout lane", "{post}");
    assert_eq!(
        post["hookSpecificOutput"]["additionalContext"],
        "familiar-ai: the person answered the question for \"greet says hello\": keep going as planned. The answer is recorded; nothing is left to claim for it. It went to the checkout lane.",
        "{post}"
    );
    assert_eq!(p.answers_at(&wt), [json!({ "criterion_id": "c1", "hold": "anchor_moved", "choice": "keep_going", "typed": "" })]);
    assert!(p.answers_at(&p.repo()).is_empty(), "main's record has no answer");
    assert_eq!(p.json_at(&p.repo())["checkouts"][0]["cards"], json!([]));
    assert_eq!(p.run(&["statusline"], "").1, "repo | Familiar | OFF · lane: 1 testing\n");
    // nothing left to ask
    assert_eq!(p.asks_in_main("familiar-ai", PROSE, json!({})), json!({}));
    let prompt = p.hook_at(&p.repo(), PERSON, "UserPromptSubmit", json!({ "prompt": "and now?" }));
    assert_eq!(prompt, json!({}));
}

#[test]
fn a_headless_lanes_stop_with_only_the_persons_card_left_goes_and_the_card_waits() {
    let p = project("headless");
    let wt = p.lane_with_a_card();
    let stop = p.hook_at(&wt, LANE, "Stop", json!({}));
    assert_eq!(stop["decision"], "block", "an interactive lane is refused as before: {stop}");
    p.set_env("CLAUDE_CODE_ENTRYPOINT", Some("sdk-cli"));
    let stop = p.hook_at(&wt, LANE, "Stop", json!({}));
    p.set_env("CLAUDE_CODE_ENTRYPOINT", None);
    assert!(stop.get("decision").is_none(), "the headless stop goes: {stop}");
    // Claude Code continues the turn on a stop's additionalContext: the card goes to the person only
    assert!(stop.get("hookSpecificOutput").is_none(), "{stop}");
    assert!(stop["systemMessage"].as_str().unwrap_or("").contains(CARD_TEXT), "{stop}");
    assert_eq!(p.json_at(&p.repo())["checkouts"][0]["cards"][0]["criterion_id"], "c1", "the card waits in the lane's record");
}
