//! Two refusals teach the way out: a full-path criterion naming no behavior or
//! promise is taught the re-claim with the ids, and `tests_not_red` names a
//! failing test to write and the command run after each commit.

mod common;

use common::Project;

const SESSION: &str = "84be6339-0cc7-40ec-91b9-cae44365931b";

fn project(name: &str) -> Project {
    Project::builder(name).session(SESSION).create()
}

#[test]
fn a_full_path_criterion_with_nothing_linked_is_told_the_gap_and_the_reclaim() {
    let p = project("full-path");
    p.claim(&["intent_submitted", "--for", "0", "--text", "greet with hello"]);
    // the full path for a bug fix, no ids
    let out = p.claim(&["criterion", "--criterion_id", "c1", "--text", "greet says hello", "--path", "full"]);
    assert!(
        out.contains("familiar-ai: the plan comes first · next: \"greet says hello\" is on the full path but names no behavior and promise: claim behavior b1 and promise p1, then re-claim the criterion with --behavior_id b1 --promise_id p1\n"),
        "{out}"
    );
    assert!(out.contains("  claim it: familiar-ai claim behavior --behavior_id b1 --text 'greeting by name'\n  then:     familiar-ai claim promise --promise_id p1 --behavior_id b1 "), "{out}");
    assert!(out.contains("  then:     familiar-ai claim criterion --criterion_id c1 --text 'greet says hello' --path full --behavior_id b1 --promise_id p1\n"), "{out}");
    assert!(!out.contains("write the behavior (behavior)"), "{out}");
    // then behavior and promise claimed twice: still nothing links, and the words say so
    for _ in 0..2 {
        p.claim(&["behavior", "--behavior_id", "b1", "--text", "greeting by name"]);
        p.claim(&["promise", "--promise_id", "p1", "--behavior_id", "b1", "--accepts", "a name", "--gives_back", "a greeting", "--refuses", "an empty name", "--cases", r#"[{"case_id":"n1","kind":"normal","given":"a name","expect":"a greeting"},{"case_id":"o1","kind":"opposite","given":"no name","expect":"no greeting"}]"#]);
    }
    let out = p.hook("PreToolUse", serde_json::json!({ "tool_name": "Edit", "tool_input": { "file_path": p.path("src/app.sh").to_string_lossy() } }));
    let reason = out["hookSpecificOutput"]["permissionDecisionReason"].as_str().expect("deny");
    // the gap's words and its claim come first, then the plan's eleven lines
    let lines: Vec<&str> = reason.lines().collect();
    assert_eq!(
        lines[..3],
        [
            "familiar-ai: the plan comes first · next: \"greet says hello\" is on the full path but names no behavior and promise: behavior b1 and promise p1 are claimed; re-claim the criterion with --behavior_id b1 --promise_id p1",
            "  claim it: familiar-ai claim criterion --criterion_id c1 --text 'greet says hello' --path full --behavior_id b1 --promise_id p1",
            "  the plan's eleven lines, 3 of 11 hold:",
        ],
        "{reason}"
    );
    assert_eq!(lines[5], "    3 full path only  claim it: familiar-ai claim criterion --criterion_id c1 --text 'greet says hello' --path full --behavior_id b1 --promise_id p1", "{reason}");
    // the re-claim links: line 3 holds, line 4 is next (the case-finder's none and the person's
    // keep_planned stand already, read from the intent on: plan cases-card, 2026-10-25 [draft names])
    p.cases_kept("c1");
    let out = p.claim(&["criterion", "--criterion_id", "c1", "--text", "greet says hello", "--path", "full", "--behavior_id", "b1", "--promise_id", "p1"]);
    assert!(out.contains("claim it: familiar-ai claim place_chosen --criterion_id c1"), "{out}");
    // definition change (the owner, 2026-10-13): the form's example for a criterion is a code anchor's, the full path with its behavior and promise (was: light)
    let (_, form, _) = p.run(&["claim", "criterion"], "");
    assert!(form.contains("--path full --behavior_id b1 --promise_id p1\n"), "{form}");
    // definition change (the owner, 2026-10-11): the path's note is the rulebook's approved line 2 note
    assert!(form.contains("--path light only when the anchor is documentation: a markdown file or a docstring; the binary checks it; everything else is --path full and also needs line 3"), "{form}");
    assert!(!form.contains("a bug fix or a small change"), "{form}");
}

#[test]
fn tests_not_red_is_followed_by_what_to_write_and_the_command_the_binary_runs() {
    let p = project("not-red");
    p.claim(&["intent_submitted", "--for", "0", "--text", "greet with hello"]);
    p.claim(&["behavior", "--behavior_id", "b1", "--text", "the greeting"]);
    p.claim(&["promise", "--promise_id", "p1", "--behavior_id", "b1", "--accepts", "a name", "--gives_back", "a greeting", "--refuses", "nothing", "--cases", r#"[{"case_id":"n1","kind":"normal","given":"a name","expect":"a greeting"},{"case_id":"o1","kind":"opposite","given":"no name","expect":"no greeting"}]"#]);
    p.claim(&["criterion", "--criterion_id", "c1", "--text", "greet says hello", "--path", "full", "--behavior_id", "b1", "--promise_id", "p1"]);
    p.cases_kept("c1"); // the cases card passed (plan cases-card, 2026-10-25) [draft names]
    p.claim(&["place_chosen", "--criterion_id", "c1", "--places", r#"[{"file_path":"src/app.sh","symbol":"greet","line_start":1,"line_end":3}]"#]);
    p.claim(&["test_planned", "--criterion_id", "c1", "--file_path", "tests/test_c1.sh", "--will", "create"]);
    p.claim(&["test_stack_detected", "--project_root", ".", "--test_stack", "sh", "--test_command", "sh tests/test_c1.sh"]);
    p.claim(&["criterion_head", "--criterion_id", "c1"]);
    p.claim(&["criterion_assigned", "--criterion_id", "c1"]);
    let out = p.claim(&["fix_location_category", "--criterion_id", "c1", "--category", "this_repository", "--where", ""]);
    assert!(out.contains("familiar-ai: the plan is confirmed"), "{out}");
    // a test that passes on the code as it is now
    std::fs::create_dir_all(p.path("tests")).expect("tests");
    std::fs::write(p.path("tests/test_c1.sh"), "grep -q hi src/app.sh\n").expect("write");
    p.git(&["add", "-A"]);
    p.git(&["commit", "-q", "-m", "c1: tests first"]);
    let out = p.hook("PostToolUse", serde_json::json!({ "tool_name": "Bash", "tool_input": { "command": "git add -A && git commit -m 'c1: tests first'" }, "tool_response": {} }));
    let context = out["hookSpecificOutput"]["additionalContext"].as_str().expect("state line");
    assert!(context.starts_with("familiar-ai: \"greet says hello\" · the test must fail on the code as it is, committed alone before any fix; reverting a fix does not count"), "{context}");
    let out = p.hook("Stop", serde_json::json!({}));
    assert_eq!(out["decision"], "block");
    let reason = out["reason"].as_str().expect("reason");
    assert_eq!(
        reason,
        "familiar-ai: \"greet says hello\" · the test must fail on the code as it is, committed alone before any fix; reverting a fix does not count · next: commit its tests alone, with no code, and see them fail\n  write a test that fails on the code as it is now; familiar-ai runs sh tests/test_c1.sh after each commit\n  a second run that passes counts as a failed try; two in a row and the person decides"
    );
    // the test was never sealed: rewriting it, by the shell too, is allowed, and told what the test is for
    let out = p.hook("PreToolUse", serde_json::json!({ "tool_name": "Bash", "tool_input": { "command": "cat > tests/test_c1.sh << 'EOF'\ngrep -q hello src/app.sh\nEOF" } }));
    assert_eq!(
        out,
        serde_json::json!({ "hookSpecificOutput": { "hookEventName": "PreToolUse", "additionalContext": "familiar-ai: tests/test_c1.sh is the planned test of \"greet says hello\"\n  asked for: greet with hello\n  it must fail on the code as it is, committed alone, before any fix; familiar-ai runs sh tests/test_c1.sh after each commit" } }),
        "the planned test, not yet sealed"
    );
}

/// `anchor_untouched`: after the rules' words, one line names what the code
/// commits since the seal changed, against the anchor, then the clear line.
#[test]
fn anchor_untouched_is_followed_by_what_the_commits_did_change_against_the_anchor() {
    let p = project("untouched");
    // the anchor is the greet function alone (line 2 of 3); the fix will land outside it
    std::fs::write(p.path("src/app.sh"), "#!/bin/sh\ngreet() { echo hi; }\ngreet\n").expect("write");
    std::fs::write(p.path("src/other.sh"), "echo other\n").expect("write");
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
    std::fs::create_dir_all(p.path("tests")).expect("tests");
    std::fs::write(p.path("tests/test_c1.sh"), "grep -q hello src/app.sh\n").expect("write");
    p.git(&["add", "-A"]);
    p.git(&["commit", "-q", "-m", "c1: tests first"]);
    let out = p.hook("PostToolUse", serde_json::json!({ "tool_name": "Bash", "tool_input": { "command": "git add -A && git commit -m 'c1: tests first'" }, "tool_response": {} }));
    assert_eq!(out["hookSpecificOutput"]["additionalContext"], "familiar-ai: ran the tests of \"greet says hello\": they fail, as they should before the fix · stamped\n  the code goes on src/app.sh greet lines 2–2; the test stays as it is");
    // the fix lands OUTSIDE the anchor: line 3 of the anchor's file, and another file
    std::fs::write(p.path("src/app.sh"), "#!/bin/sh\ngreet() { echo hi; }\ngreet; echo hello\n").expect("write");
    std::fs::write(p.path("src/other.sh"), "echo other, too\n").expect("write");
    p.git(&["add", "-A"]);
    p.git(&["commit", "-q", "-m", "c1: the fix, beside the anchor"]);
    let out = p.hook("PostToolUse", serde_json::json!({ "tool_name": "Bash", "tool_input": { "command": "git commit -am 'c1: the fix'" }, "tool_response": {} }));
    let context = out["hookSpecificOutput"]["additionalContext"].as_str().expect("state line");
    assert!(context.starts_with("familiar-ai: \"greet says hello\" · the change landed outside the place the plan named"), "{context}");
    let out = p.hook("Stop", serde_json::json!({}));
    assert_eq!(out["decision"], "block");
    let reason = out["reason"].as_str().expect("reason");
    let lines: Vec<&str> = reason.lines().collect();
    // anchor_untouched is a card the person answers: the stop says the question is open first
    assert_eq!(lines[0], "familiar-ai: a question for the person is open: ask them now with AskUserQuestion; the hook turns that call into the question card");
    assert_eq!(lines[1], "familiar-ai: \"greet says hello\" · the change landed outside the place the plan named · next: the person answers its card: plan it again, keep going as planned or release it; or choose its place in the code again and have the lines checked");
    assert_eq!(lines[2], "  the commits since the seal changed src/app.sh (and 1 more); the place the plan named is src/app.sh greet 2–2");
    assert!(lines[3].starts_with("  claim it: familiar-ai claim place_chosen --criterion_id c1 --places "), "{reason}");
    assert!(reason.contains("The record's card: \"greet says hello\": its tests pass, but no commit changed the code at src/app.sh greet. What now?"), "{reason}");
    // the refused stop shows the intent next to the sealed test
    assert_eq!(&lines[lines.len() - 4..], ["familiar-ai: \"greet says hello\"", "  asked for: greet with hello", "  tested by: tests/test_c1.sh", "  judge: off: the person chose no judge, so the seal stands on its red run alone"], "{reason}");
}
