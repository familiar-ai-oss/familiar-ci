//! A card is the person's to answer: `hold_answered` from an agent's shell
//! (`CLAUDE_CODE_SESSION_ID` set) is refused, from a person's terminal it is
//! accepted, and only on the bench may a stop go with the card still open.

mod common;

use common::Project;

const SESSION: &str = "84be6339-0cc7-40ec-91b9-cae44365931b";

/// Off the bench unless a hook says otherwise.
fn project(name: &str) -> Project {
    Project::builder(name).session(SESSION).env("FAMILIAR_AI_BENCH", "0").create()
}

impl Project {
    /// The hook, on the bench (`FAMILIAR_AI_BENCH=1`) or not.
    fn hook_on(&self, event: &str, rest: serde_json::Value, bench: bool) -> serde_json::Value {
        self.set_env("FAMILIAR_AI_BENCH", Some(if bench { "1" } else { "0" }));
        let out = self.hook(event, rest);
        self.set_env("FAMILIAR_AI_BENCH", Some("0"));
        out
    }

    /// The plan, line by line through the door: c1 on src/app.sh greet 1–3.
    fn plan(&self) {
        self.claim(&["intent_submitted", "--for", "0", "--text", "greet with hello"]);
        self.claim(&["behavior", "--behavior_id", "b1", "--text", "the greeting"]);
        self.claim(&["promise", "--promise_id", "p1", "--behavior_id", "b1", "--accepts", "a name", "--gives_back", "a greeting", "--refuses", "nothing", "--cases", r#"[{"case_id":"n1","kind":"normal","given":"a name","expect":"a greeting"},{"case_id":"o1","kind":"opposite","given":"no name","expect":"no greeting"}]"#]);
        self.claim(&["criterion", "--criterion_id", "c1", "--text", "greet says hello", "--path", "full", "--behavior_id", "b1", "--promise_id", "p1"]);
        self.cases_kept("c1"); // the cases card passed (plan cases-card, 2026-10-25) [draft names]
        self.claim(&["place_chosen", "--criterion_id", "c1", "--places", r#"[{"file_path":"src/app.sh","symbol":"greet","line_start":1,"line_end":3}]"#]);
        self.claim(&["test_planned", "--criterion_id", "c1", "--file_path", "tests/test_c1.sh", "--will", "create"]);
        self.claim(&["test_stack_detected", "--project_root", ".", "--test_stack", "sh", "--test_command", "sh tests/test_c1.sh"]);
        self.claim(&["criterion_head", "--criterion_id", "c1"]);
        self.claim(&["criterion_assigned", "--criterion_id", "c1"]);
        let out = self.claim(&["fix_location_category", "--criterion_id", "c1", "--category", "this_repository", "--where", ""]);
        assert!(out.contains("familiar-ai: the plan is confirmed"), "{out}");
    }
}

const ANSWER: [&str; 9] = ["claim", "hold_answered", "--criterion_id", "c1", "--hold", "anchor_moved", "--choice", "keep_going", "--typed"];

fn answer_args() -> Vec<&'static str> {
    let mut a = ANSWER.to_vec();
    a.push("");
    a
}

/// An edit of the anchor's lines before the start check holds `anchor_moved`;
/// the agent's answer is refused and the person's is accepted.
#[test]
fn an_outside_edit_before_the_start_is_anchor_moved_and_the_card_is_the_persons() {
    // the start check runs at the claim that confirms the plan, so the outside edit comes first
    let p = project("moved");
    p.claim(&["intent_submitted", "--for", "0", "--text", "greet with hello"]);
    p.claim(&["behavior", "--behavior_id", "b1", "--text", "the greeting"]);
    p.claim(&["promise", "--promise_id", "p1", "--behavior_id", "b1", "--accepts", "a name", "--gives_back", "a greeting", "--refuses", "nothing", "--cases", r#"[{"case_id":"n1","kind":"normal","given":"a name","expect":"a greeting"},{"case_id":"o1","kind":"opposite","given":"no name","expect":"no greeting"}]"#]);
    p.claim(&["criterion", "--criterion_id", "c1", "--text", "greet says hello", "--path", "full", "--behavior_id", "b1", "--promise_id", "p1"]);
    p.cases_kept("c1"); // the cases card passed (plan cases-card, 2026-10-25) [draft names]
    p.claim(&["place_chosen", "--criterion_id", "c1", "--places", r#"[{"file_path":"src/app.sh","symbol":"greet","line_start":1,"line_end":3}]"#]);
    p.claim(&["test_planned", "--criterion_id", "c1", "--file_path", "tests/test_c1.sh", "--will", "create"]);
    p.claim(&["test_stack_detected", "--project_root", ".", "--test_stack", "sh", "--test_command", "sh tests/test_c1.sh"]);
    p.claim(&["criterion_head", "--criterion_id", "c1"]);
    // an outside edit of the anchored lines before the last line is claimed
    p.write("src/app.sh", "#!/bin/sh\ngreet() { echo hey; }\ngreet\n");
    p.claim(&["criterion_assigned", "--criterion_id", "c1"]);
    let out = p.claim(&["fix_location_category", "--criterion_id", "c1", "--category", "this_repository", "--where", ""]);
    assert!(out.contains("familiar-ai: the plan is confirmed"), "{out}");
    assert!(out.contains("familiar-ai: \"greet says hello\" · the code at the plan's place changed after the plan was made"), "{out}");
    assert!(out.contains("familiar-ai: a question for you · \"greet says hello\": the code at src/app.sh greet changed after the plan was made. What now?"), "{out}");
    // the choices in the person's world
    assert!(out.contains("  what now?\n    1) carry on with part 1 on the code as it is now\n    2) have the agent pick the code for part 1 again\n    3) drop it\n"), "{out}");
    assert!(out.contains("> or type your own answer"), "{out}");

    // the stop is refused on the hold: the person answers the card, asked with AskUserQuestion; the agent is handed no claim of theirs
    let out = p.hook("Stop", serde_json::json!({}));
    assert_eq!(out["decision"], "block", "{out}");
    let reason = out["reason"].as_str().expect("reason");
    assert!(reason.starts_with("familiar-ai: a question for the person is open: ask them now with AskUserQuestion; the hook turns that call into the question card\nfamiliar-ai: \"greet says hello\" · the code at the plan's place changed after the plan was made · next: "), "{reason}");
    assert!(reason.contains("the person answers its card: plan it again, keep going as planned or release it"), "{reason}");
    assert!(!reason.contains("anchor_moved:") && !reason.contains("refused"), "{reason}");
    assert!(!reason.contains("familiar-ai claim hold_answered"), "{reason}");
    assert!(reason.contains("Ask the person with AskUserQuestion: the hook turns that call into the question card"), "{reason}");

    // the agent answers from its own shell: refused, naming the person's command and showing the card
    let (code, stdout, stderr) = p.run_as(Some(SESSION), &answer_args(), "");
    assert_eq!(code, 1);
    assert_eq!(stdout, "");
    assert!(
        stderr.starts_with("familiar-ai: not claimed yet · this card is the person's to answer: ask the person with AskUserQuestion; the hook turns that call into the question card and records their pick (in a terminal of their own the person can also run: familiar-ai claim hold_answered --criterion_id c1 --hold anchor_moved --choice keep_going --typed '')\nfamiliar-ai: a question for you · \"greet says hello\": the code at src/app.sh greet changed after the plan was made. What now?\n  you asked: \"greet with hello\"\n  the agent split that into one part:\n    1  greet says hello\n  what happened: someone changed the code the agent had picked for part 1, outside this work.\n  what now?\n    1) carry on with part 1 on the code as it is now\n    2) have the agent pick the code for part 1 again\n    3) drop it\n  > or type your own answer\nfamiliar-ai claim hold_answered: "),
        "{stderr}"
    );
    assert!(!stderr.contains("--as-person"), "{stderr}");
    let out = p.hook("Stop", serde_json::json!({}));
    assert!(out["reason"].as_str().unwrap_or("").starts_with("familiar-ai: a question for the person is open: ask them now with AskUserQuestion; the hook turns that call into the question card\nfamiliar-ai: \"greet says hello\" · the code at the plan's place changed"), "still held: {out}");

    // a hold that is not a card hold (not_stamped) is refused whoever asks
    // definition change (the owner's plan of 2026-10-25, the cases card) [draft name]: cases_unconfirmed is a card hold
    for session in [Some(SESSION), None] {
        let (code, _, stderr) = p.run_as(session, &["claim", "hold_answered", "--criterion_id", "c1", "--hold", "not_stamped", "--choice", "keep_going", "--typed", ""], "");
        assert_eq!(code, 1, "{session:?}");
        assert!(stderr.starts_with("familiar-ai: not claimed yet · hold_answered answers a card, and not_stamped is not a card hold; the card holds are anchor_moved, two_failures, two_tries_not_red, fix_elsewhere, anchor_untouched, two_test_broken, two_crashed, two_timed_out, environment_error, run_unclassified, retry_spent, crashed, command_broke, miscoped, reading_gap, setup_changed, review_disputed, cases_unconfirmed and no_test_stack\n"), "{stderr}");
    }

    // the person answers from a terminal of their own: keep going; c1 is testing again
    let (code, stdout, stderr) = p.run_as(None, &answer_args(), "");
    assert_eq!(code, 0, "{stderr}");
    assert!(stdout.contains("familiar-ai: \"greet says hello\" · the test comes first"), "{stdout}");
    let out = p.hook("Stop", serde_json::json!({}));
    assert!(out["reason"].as_str().unwrap_or("").starts_with("familiar-ai: not done yet · \"greet says hello\" · the test comes first"), "{out}");
}

/// A release answered by the person shows in the status line.
#[test]
fn a_release_by_the_person_is_said_so_on_the_status_line() {
    let p = project("released");
    p.plan();
    let (code, _, stderr) = p.run_as(Some(SESSION), &["claim", "hold_answered", "--criterion_id", "c1", "--hold", "two_failures", "--choice", "release", "--typed", ""], "");
    assert_eq!(code, 1, "the agent cannot release: {stderr}");
    assert!(stderr.contains("this card is the person's to answer"), "{stderr}");
    let (code, stdout, _) = p.run_as(None, &["claim", "hold_answered", "--criterion_id", "c1", "--hold", "two_failures", "--choice", "release", "--typed", ""], "");
    assert_eq!(code, 0);
    assert!(stdout.contains("familiar-ai: \"greet says hello\" · released by you"), "{stdout}");
    let (code, status, _) = p.run_as(None, &["status"], "");
    assert_eq!(code, 0);
    assert!(status.contains("familiar-ai: \"greet says hello\" · released by you · unscoped · scope the command to prove in parallel\n"), "{status}");
    let out = p.hook("Stop", serde_json::json!({}));
    assert_eq!(out, serde_json::json!({}), "released: the agent may stop");
}

/// Held on anchor_moved with nothing left for the agent: on the bench the stop
/// goes, carrying the card; off the bench it is refused.
#[test]
fn on_the_bench_a_stop_with_only_the_persons_card_left_goes_carrying_the_card() {
    let p = project("bench");
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
    p.claim(&["fix_location_category", "--criterion_id", "c1", "--category", "this_repository", "--where", ""]);
    // the card, in the person's terms
    let card = "familiar-ai: a question for you · \"greet says hello\": the code at src/app.sh greet changed after the plan was made. What now?\n  you asked: \"greet with hello\"\n  the agent split that into one part:\n    1  greet says hello\n  what happened: someone changed the code the agent had picked for part 1, outside this work.\n  what now?\n    1) carry on with part 1 on the code as it is now\n    2) have the agent pick the code for part 1 again\n    3) drop it\n  > or type your own answer";

    let out = p.hook_on("Stop", serde_json::json!({}), false);
    assert_eq!(out["decision"], "block", "off the bench: refused as before: {out}");

    for event in ["Stop", "SubagentStop"] {
        let out = p.hook_on(event, serde_json::json!({}), true);
        assert!(out.get("decision").is_none(), "{event}: the stop goes: {out}");
        // Claude Code continues the turn on a stop's additionalContext: nothing goes back to the agent
        assert!(out.get("hookSpecificOutput").is_none(), "{event}: {out}");
        assert!(out["systemMessage"].as_str().unwrap_or("").contains(card), "{event}: {out}");
    }

    // the person answers: c1 is the agent's again, and the bench's stop is refused
    let (code, _, stderr) = p.run_as(None, &answer_args(), "");
    assert_eq!(code, 0, "{stderr}");
    let out = p.hook_on("Stop", serde_json::json!({}), true);
    assert_eq!(out["decision"], "block", "{out}");
}

/// With every part cleared, an edit waits for a new part and a stop goes (the owner's answer,
/// 2026-10-28: edits wait, stop goes). The binary's run closes once every part since its intent is
/// done (`person::mode` reads idle), so the edit waits for a new intent and a part
/// (`restrict::intent_first`) while the stop goes; the rules are asked only once a run is open.
#[test]
fn with_every_part_cleared_an_edit_waits_for_a_new_part_and_a_stop_goes() {
    let p = project("cleared");
    p.plan();
    let (code, out, err) = p.run_as(None, &["clear"], "");
    assert_eq!(code, 0, "{out}{err}");
    let input = serde_json::json!({ "file_path": p.path("src/app.sh").to_string_lossy(), "old_string": "a", "new_string": "b" });
    let v = p.hook("PreToolUse", serde_json::json!({ "tool_name": "Edit", "tool_input": input }));
    assert_eq!(v["hookSpecificOutput"]["permissionDecision"], "deny", "{v}");
    assert!(v["hookSpecificOutput"]["permissionDecisionReason"].as_str().unwrap_or("").contains("claim criterion"), "the reason names a new part's claim: {v}");
    assert_eq!(p.hook("Stop", serde_json::json!({})), serde_json::json!({}), "a stop goes");
}
