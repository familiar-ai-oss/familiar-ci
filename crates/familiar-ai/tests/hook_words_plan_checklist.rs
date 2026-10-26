//! A planning refusal shows the whole plan: after its first line, the plan's
//! eleven lines as a checklist derived from the record, each line that holds
//! with the values the facts give it, each that does not with the claim that
//! completes it (line 5, the binary's, with what it waits for), then the call
//! that claims them all at once and the skill that says more. The first edit
//! while no run is open carries the same checklist, and says the edit waits.

mod common;

use common::Project;
use serde_json::{json, Value};

const SESSION: &str = "3f1c2a4b-5d6e-4f70-8a9b-0c1d2e3f4a5b";

fn project(name: &str) -> Project {
    Project::builder(name).session(SESSION).create()
}

impl Project {
    fn edit(&self, file: &str) -> Value {
        let path = self.path(file).to_string_lossy().into_owned();
        self.hook("PreToolUse", json!({ "tool_name": "Edit", "tool_input": { "file_path": path, "old_string": "echo hi", "new_string": "echo hello" } }))
    }
}

fn denied(out: &Value) -> String {
    assert_eq!(out["hookSpecificOutput"]["permissionDecision"], "deny", "{out}");
    out["hookSpecificOutput"]["permissionDecisionReason"].as_str().unwrap_or("").to_string()
}

const ALL_AT_ONCE: &str = "  all at once: familiar-ai plan takes the missing lines as a JSON array on stdin (familiar-ai plan with nothing on stdin prints the array's form); the familiar-ai:plan skill says more";

#[test]
fn an_edit_while_the_plan_is_incomplete_shows_every_line_that_holds_and_the_claim_of_each_that_does_not() {
    let p = project("incomplete");
    p.claim(&["intent_submitted", "--for", "0", "--text", "greet with hello"]);
    p.claim(&["behavior", "--behavior_id", "b1", "--text", "the greeting"]);
    p.claim(&["promise", "--promise_id", "p1", "--behavior_id", "b1", "--accepts", "a name", "--gives_back", "a greeting", "--refuses", "nothing", "--cases", r#"[{"case_id":"n1","kind":"normal","given":"a name","expect":"a greeting"},{"case_id":"o1","kind":"opposite","given":"no name","expect":"no greeting"}]"#]);
    p.claim(&["criterion", "--criterion_id", "c1", "--text", "greet says hello", "--path", "full", "--behavior_id", "b1", "--promise_id", "p1"]);
    p.cases_kept("c1"); // the cases card passed (plan cases-card, 2026-10-25) [draft names]
    p.claim(&["place_chosen", "--criterion_id", "c1", "--places", r#"[{"file_path":"src/app.sh","symbol":"greet","line_start":1,"line_end":3}]"#]);
    p.claim(&["test_planned", "--criterion_id", "c1", "--file_path", "tests/test_c1.sh", "--will", "create"]);
    let sha: String = p.head().chars().take(7).collect();
    let reason = denied(&p.edit("src/app.sh"));
    assert_eq!(
        reason,
        [
            "familiar-ai: the plan comes first · next: name the command that runs the tests",
            "  the plan's eleven lines, 7 of 11 hold:",
            "  ✓ 1 the intent      \"greet with hello\"",
            // definition change (the owner, 2026-10-11): light is for documentation only; a code anchor is the full path
            "  ✓ 2 a criterion     \"greet says hello\" (full)",
            "  ✓ 3 full path only  \"greet says hello\": behavior b1, promise p1 (cases n1, o1)",
            "  ✓ 4 its anchor      src/app.sh greet lines 1–3",
            &format!("  ✓ 5 the check       the lines matched at {sha}"),
            "  ✓ 6 its tests       tests/test_c1.sh",
            "    7 the command     claim it: familiar-ai claim test_stack_detected --project_root . --test_stack sh --test_command 'sh tests/test_c1.sh'",
            "  ✓ 8 touching tests  none declared; none is allowed",
            "    9 the string      claim it: familiar-ai claim criterion_head --criterion_id c1",
            "   10 who holds it    claim it: familiar-ai claim criterion_assigned --criterion_id c1",
            "   11 where it lives  claim it: familiar-ai claim fix_location_category --criterion_id c1 --category '' --where ''",
            ALL_AT_ONCE,
        ]
        .join("\n")
    );
    // the claim lines are pasteable as they are: line 7's claim is accepted
    p.claim(&["test_stack_detected", "--project_root", ".", "--test_stack", "sh", "--test_command", "sh tests/test_c1.sh"]);
    // a commit is refused with the same checklist, now one line more holding
    let out = p.bash("PreToolUse", "git commit -am 'c1: tests first'");
    let reason = denied(&out);
    assert!(reason.contains("\n  the plan's eleven lines, 8 of 11 hold:\n"), "{reason}");
    assert!(reason.contains("\n  ✓ 7 the command     sh tests/test_c1.sh\n"), "{reason}");
    assert!(reason.ends_with(&format!("\n{ALL_AT_ONCE}")), "{reason}");
    // status and the session's start carry the same refusal
    let status = p.status();
    assert!(status.contains("\n  ✓ 7 the command     sh tests/test_c1.sh\n  ✓ 8 touching tests  none declared; none is allowed\n    9 the string      claim it: familiar-ai claim criterion_head --criterion_id c1\n"), "{status}");
    let start = p.hook("SessionStart", json!({ "source": "startup" }));
    let context = start["hookSpecificOutput"]["additionalContext"].as_str().unwrap_or("");
    assert!(context.contains("\n  the plan's eleven lines, 8 of 11 hold:\n"), "{context}");
    // once the eleven lines hold, nothing of the checklist is said
    p.claim(&["criterion_head", "--criterion_id", "c1"]);
    p.claim(&["criterion_assigned", "--criterion_id", "c1"]);
    let out = p.claim(&["fix_location_category", "--criterion_id", "c1", "--category", "this_repository", "--where", ""]);
    assert!(out.contains("familiar-ai: the plan is confirmed"), "{out}");
    assert!(!p.status().contains("the plan's eleven lines"), "{}", p.status());
}

#[test]
fn the_full_paths_words_and_claims_come_first_then_the_checklist() {
    let p = project("full");
    p.claim(&["intent_submitted", "--for", "0", "--text", "greet with hello"]);
    p.claim(&["criterion", "--criterion_id", "c1", "--text", "greet says hello", "--path", "full"]);
    p.cases_kept("c1"); // the cases card passed (plan cases-card, 2026-10-25) [draft names]
    let reason = denied(&p.edit("src/app.sh"));
    let lines: Vec<&str> = reason.lines().collect();
    assert_eq!(lines[0], "familiar-ai: the plan comes first · next: \"greet says hello\" is on the full path but names no behavior and promise: claim behavior b1 and promise p1, then re-claim the criterion with --behavior_id b1 --promise_id p1");
    assert_eq!(lines[1], "  claim it: familiar-ai claim behavior --behavior_id b1 --text 'greeting by name'");
    assert!(lines[2].starts_with("  then:     familiar-ai claim promise --promise_id p1 --behavior_id b1 "), "{reason}");
    assert_eq!(lines[3], "  then:     familiar-ai claim criterion --criterion_id c1 --text 'greet says hello' --path full --behavior_id b1 --promise_id p1");
    assert_eq!(lines[4], "  the plan's eleven lines, 3 of 11 hold:");
    assert_eq!(lines[7], "    3 full path only  claim it: familiar-ai claim behavior --behavior_id b1 --text 'greeting by name'");
    assert!(lines[8].starts_with("    4 its anchor      claim it: familiar-ai claim place_chosen --criterion_id c1 --places "), "{reason}");
    assert!(lines[9].starts_with("                      or:       familiar-ai claim new_work_spot_chosen --criterion_id c1 "), "{reason}");
    assert!(lines[10].starts_with("                      or:       familiar-ai claim new_work_no_spot --criterion_id c1 "), "{reason}");
    assert_eq!(lines[11], "    5 the check       waits for line 4: the binary checks the anchor's lines once they are claimed");
    // definition change (lane l1, 2026-10-15): a test file is named after what it tests, never a plan id
    // definition change (the owner, 2026-10-15, §9 Q3): the example runs python -m pytest, python3 when no python is on PATH
    assert_eq!(lines[13], &format!("    7 the command     claim it: familiar-ai claim test_stack_detected --project_root . --test_stack pytest --test_command '{py} -m pytest -q tests/test_greet.py'", py = familiar_ai::teach::example::python()));
    assert_eq!(lines[14], "  ✓ 8 touching tests  none declared; none is allowed");
    assert_eq!(lines.last().copied(), Some(ALL_AT_ONCE));
}

#[test]
fn the_first_edit_while_no_run_is_open_carries_the_checklist_and_says_the_edit_waits() {
    let p = project("idle");
    p.hook("UserPromptSubmit", json!({ "prompt": "make greet say hello" }));
    let reason = denied(&p.edit("src/app.sh"));
    let lines: Vec<&str> = reason.lines().collect();
    assert_eq!(lines[0], "familiar-ai: the plan comes first · line 1: the intent · src/app.sh is code");
    let t = lines.iter().find_map(|l| l.strip_prefix("  claim it: familiar-ai claim intent_submitted --for ")).and_then(|r| r.split_whitespace().next()).expect("the intent's claim names the prompt");
    let claim = format!("claim it: familiar-ai claim intent_submitted --for {t} --text 'make greet say hello'");
    let at = lines.iter().position(|l| *l == "  the plan's eleven lines, 0 of 11 hold:").unwrap_or_else(|| panic!("the checklist follows: {reason}"));
    assert_eq!(lines[at - 1], format!("  {claim}"), "after the existing lines: {reason}");
    assert_eq!(
        &lines[at + 1..],
        [
            &format!("    1 the intent      {claim}"),
            // definition change (the owner, 2026-10-13): the example criterion is a code anchor's, the full path with its behavior and promise
            "    2 a criterion     claim it: familiar-ai claim criterion --criterion_id c1 --text 'greet says hello' --path full --behavior_id b1 --promise_id p1",
            "    3 full path only  waits for a criterion (line 2)",
            "    4 its anchor      claim it: familiar-ai claim place_chosen --criterion_id c1 --places '[{\"file_path\":\"src/app.py\",\"symbol\":\"greet\",\"line_start\":10,\"line_end\":20}]'",
            "                      or:       familiar-ai claim new_work_spot_chosen --criterion_id c1 --file_path src/app.py --symbol greet_loudly --beside greet",
            "                      or:       familiar-ai claim new_work_no_spot --criterion_id c1 --integration_point src/app.py:main",
            "    5 the check       waits for line 4: the binary checks the anchor's lines once they are claimed",
            // definition change (lane l1, 2026-10-15): a test file is named after what it tests, never a plan id
            "    6 its tests       claim it: familiar-ai claim test_planned --criterion_id c1 --file_path tests/test_greet.py --will create",
            // definition change (the owner, 2026-10-15, §9 Q3): the example runs python -m pytest, python3 when no python is on PATH
            &format!("    7 the command     claim it: familiar-ai claim test_stack_detected --project_root . --test_stack pytest --test_command '{py} -m pytest -q tests/test_greet.py'", py = familiar_ai::teach::example::python()),
            "    8 touching tests  none declared; none is allowed",
            "    9 the string      claim it: familiar-ai claim criterion_head --criterion_id c1",
            "   10 who holds it    claim it: familiar-ai claim criterion_assigned --criterion_id c1",
            "   11 where it lives  claim it: familiar-ai claim fix_location_category --criterion_id c1 --category '' --where ''",
            ALL_AT_ONCE,
            "  nothing is edited until the plan is confirmed; this edit waits",
        ]
    );
}

#[test]
fn line_7_waits_for_line_4_once_a_command_is_claimed() {
    // definition change (ports 2026-10-24, piece 2): with a command claimed and no anchor yet, line 7
    // waits for line 4 as line 5 does; its claim line sent the bench's agents to claim the command
    // again and again (click-3299, pytest-14931). With no command claimed it is still claimed now, as
    // the two tests above pin: the test is written before the place, under a command.
    let p = project("waits");
    p.claim(&["intent_submitted", "--for", "0", "--text", "greet with hello"]);
    p.claim(&["criterion", "--criterion_id", "c1", "--text", "greet says hello", "--path", "full"]);
    p.cases_kept("c1"); // the cases card passed (plan cases-card, 2026-10-25) [draft names]
    p.claim(&["test_planned", "--criterion_id", "c1", "--file_path", "tests/test_c1.sh", "--will", "create"]);
    p.claim(&["test_stack_detected", "--project_root", ".", "--test_stack", "sh", "--test_command", "sh tests/test_c1.sh"]);
    let reason = denied(&p.edit("src/app.sh"));
    assert!(reason.contains("\n    5 the check       waits for line 4: the binary checks the anchor's lines once they are claimed\n"), "{reason}");
    assert!(reason.contains("\n    7 the command     waits for line 4: a command is claimed; it must cover the anchor's files once they are claimed\n"), "{reason}");
    // once the anchor is claimed, under the command's folder, line 7 holds
    p.claim(&["place_chosen", "--criterion_id", "c1", "--places", r#"[{"file_path":"src/app.sh","symbol":"greet","line_start":1,"line_end":3}]"#]);
    let reason = denied(&p.edit("src/app.sh"));
    assert!(reason.contains("\n  ✓ 7 the command     sh tests/test_c1.sh\n"), "{reason}");
}
