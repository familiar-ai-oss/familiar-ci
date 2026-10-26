//! Once every criterion is green, an allowed stop and each prompt tell the
//! agent the facts to ask the person about, and the status line follows the
//! branch and pull request; a fake `gh` and a local bare repository stand in.

mod common;

use std::process::Command;

use common::Project;
use serde_json::json;

const SESSION: &str = "84be6339-0cc7-40ec-91b9-cae44365931b";
const PUSH: &str = "familiar-ai: every criterion is locally green. CI proves it on the pull request. The branch is not pushed. The person has not been asked whether to push and open the pull request. The push begins with familiar-ai tidy, which keeps one seal commit and one fix commit per criterion and drops the failed attempts.";
const OPEN_PR: &str = "familiar-ai: every criterion is locally green. CI proves it on the pull request. The branch is pushed and has no pull request. The person has not been asked whether to open the pull request.";

/// A fake `gh` that logs every call and answers `pr view` and `pr checks` from
/// files; a missing `view.exit` means no pull request.
const FAKE_GH: &str = r#"#!/bin/sh
here="$(cd "$(dirname "$0")" && pwd)"
echo "$*" >> "$here/calls"
case "$1 $2" in
  "pr view") code=$(cat "$here/view.exit" 2>/dev/null || echo 1); [ "$code" = 0 ] && cat "$here/view.json" 2>/dev/null; exit "$code" ;;
  "pr checks") cat "$here/checks.json" 2>/dev/null; exit 0 ;;
esac
exit 1
"#;

fn project(name: &str) -> Project {
    Project::builder(name).session(SESSION).bin("gh", FAKE_GH).create()
}

impl Project {
    fn statusline(&self) -> String {
        let input = json!({ "session_id": SESSION, "cwd": self.repo().to_string_lossy(), "workspace": { "current_dir": self.repo().to_string_lossy(), "project_dir": self.repo().to_string_lossy() } });
        let (code, out, err) = self.run(&["statusline"], &input.to_string());
        assert_eq!((code, err.as_str()), (0, ""));
        // CI and the next step belong to other surfaces, not the status line
        assert!(!out.contains("CI") && !out.contains("next:"), "{out}");
        out
    }

    /// Claims the plan, seals the test and makes the code green.
    fn green(name: &str) -> Project {
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
        p.claim(&["criterion_assigned", "--criterion_id", "c1"]);
        p.claim(&["fix_location_category", "--criterion_id", "c1", "--category", "this_repository", "--where", ""]);
        assert_eq!(p.statusline(), "repo | Familiar | ON | your ask: greet with hello · 1 testing\n");
        std::fs::create_dir_all(p.repo().join("tests")).expect("tests");
        std::fs::write(p.repo().join("tests/test_c1.sh"), "sh src/app.sh | grep -qx hello\n").expect("test");
        let sealed = p.commit("c1: tests first");
        assert_eq!(sealed["hookSpecificOutput"]["additionalContext"], "familiar-ai: ran the tests of \"greet says hello\": they fail, as they should before the fix · stamped\n  the code goes on src/app.sh greet lines 1–3; the test stays as it is");
        std::fs::write(p.repo().join("src/app.sh"), "#!/bin/sh\ngreet() { echo hello; }\ngreet\n").expect("fix");
        let green = p.commit("c1: the fix");
        assert_eq!(green["hookSpecificOutput"]["additionalContext"], "familiar-ai: ran the tests of \"greet says hello\": they pass · stamped");
        p
    }
}

#[test]
fn every_criterion_green_the_agent_is_told_the_facts_and_the_status_line_follows_the_branch() {
    let p = Project::green("walk");
    assert_eq!(p.statusline(), "repo | Familiar | ON | your ask: greet with hello · 1 green\n");

    // not pushed: the stop is allowed, the agent told the facts
    let stop = p.hook("Stop", json!({ "stop_hook_active": false }));
    assert_eq!(stop, json!({ "hookSpecificOutput": { "hookEventName": "Stop", "additionalContext": PUSH } }));
    // the stop the hook already continued once ends; the person sees the line
    let again = p.hook("Stop", json!({ "stop_hook_active": true }));
    assert_eq!(again, json!({ "systemMessage": "familiar-ai: locally green · next: push and open the pull request, and CI proves it" }));
    // the person's next prompt: the same facts, so it cannot be missed
    let prompt = p.hook("UserPromptSubmit", json!({ "prompt": "anything else?" }));
    assert_eq!(prompt["hookSpecificOutput"]["additionalContext"], PUSH, "{prompt}");
    assert!(prompt.get("decision").is_none(), "{prompt}");
    // every criterion is done, so the run is over and a prompt needs no answer
    assert!(p.hook("Stop", json!({ "stop_hook_active": true })).get("decision").is_none());
    let (_, status, _) = p.run(&["status"], "");
    assert!(status.contains("\nfamiliar-ai: locally green · next: push and open the pull request, and CI proves it\n"), "{status}");
    assert!(!std::fs::read_to_string(p.dir.join("bin/calls")).unwrap_or_default().contains("pr"), "not pushed: gh is not asked");

    // pushed to a local bare upstream, no pull request yet
    let bare = p.dir.join("upstream.git");
    let out = Command::new("git").args(["init", "-q", "--bare"]).arg(&bare).output().expect("bare");
    assert!(out.status.success());
    p.git(&["remote", "add", "origin", &bare.to_string_lossy()]);
    p.git(&["push", "-q", "-u", "origin", "main"]);
    assert_eq!(p.statusline(), "repo | Familiar | ON | your ask: greet with hello · 1 green\n");
    let stop = p.hook("Stop", json!({}));
    assert_eq!(stop, json!({ "hookSpecificOutput": { "hookEventName": "Stop", "additionalContext": OPEN_PR } }));
    // the stop the hook already continued once: the person reads locally green, never proven on this laptop
    let again = p.hook("Stop", json!({ "stop_hook_active": true }));
    assert_eq!(again, json!({ "systemMessage": "familiar-ai: locally green · next: open the pull request" }));

    // the pull request is open: CI is proving it
    std::fs::write(p.dir.join("bin/view.exit"), "0").expect("a PR");
    let head = p.head();
    std::fs::write(p.dir.join("bin/view.json"), format!(r#"{{"number":7,"headRefOid":"{head}"}}"#)).expect("the PR's head");
    assert_eq!(p.statusline(), "repo | Familiar | ON | your ask: greet with hello · 1 green\n");
    let stop = p.hook("Stop", json!({}));
    assert_eq!(stop, json!({ "systemMessage": "familiar-ai: CI is proving it" }));
    let prompt = p.hook("UserPromptSubmit", json!({ "prompt": "and now?" }));
    assert_eq!(prompt["hookSpecificOutput"]["additionalContext"], "familiar-ai: CI is proving it");

    // its checks: all passed with no familiar-verify (proving), familiar-verify passed, then one failed
    std::fs::write(p.dir.join("bin/checks.json"), r#"[{"name":"build","bucket":"pass"},{"name":"lint","bucket":"skipping"}]"#).expect("checks");
    assert_eq!(p.statusline(), "repo | Familiar | ON | your ask: greet with hello · 1 green\n");
    std::fs::write(p.dir.join("bin/checks.json"), r#"[{"name":"build","bucket":"pass"},{"name":"familiar-verify","bucket":"pass","state":"SUCCESS"}]"#).expect("checks");
    assert_eq!(p.statusline(), "repo | Familiar | ON | your ask: greet with hello · 1 green\n");
    std::fs::write(p.dir.join("bin/checks.json"), r#"[{"name":"build","bucket":"pass"},{"name":"test (ubuntu)","bucket":"fail"}]"#).expect("checks");
    assert_eq!(p.statusline(), "repo | Familiar | ON | your ask: greet with hello · 1 green\n");
    let (_, status, _) = p.run(&["status"], "");
    assert!(status.contains("\nfamiliar-ai: CI found: test (ubuntu) (see the PR)\n") || status.ends_with("familiar-ai: CI found: test (ubuntu) (see the PR)\n"), "{status}");

    // a commit after the push: ahead of the upstream again
    std::fs::write(p.repo().join("README"), "x\n").expect("readme");
    p.commit("docs");
    assert_eq!(p.statusline(), "repo | Familiar | ON | your ask: greet with hello · 1 green\n");
}

/// The agent's push question goes through the picker as written; the answer
/// is told to the agent and nothing is recorded.
#[test]
fn the_push_question_is_asked_in_the_picker_and_its_answer_is_told() {
    let p = Project::green("push");
    let ask = json!({ "questions": [{ "question": "Continue?", "header": "x", "options": [{ "label": "yes", "description": "Continue" }], "multiSelect": false }] });
    let pre = p.hook("PreToolUse", json!({ "tool_name": "AskUserQuestion", "tool_input": ask }));
    let hso = &pre["hookSpecificOutput"];
    assert_eq!(hso["permissionDecision"], "ask", "{pre}");
    let push = hso["updatedInput"].clone();
    assert_eq!(
        push.to_string(),
        r#"{"questions":[{"question":"Every criterion is locally green. Push and open the pull request so CI proves it?","header":"familiar-ai","options":[{"label":"yes, push and open it","description":"the agent pushes the branch and opens the pull request"},{"label":"not now","description":"nothing is pushed now"}],"multiSelect":false}]}"#
    );
    let record = || std::fs::metadata(p.repo().join(".familiar/record")).map(|m| m.len()).unwrap_or(0);
    let before = record();
    let q = push["questions"][0]["question"].as_str().unwrap_or("").to_string();
    for (answer, told) in [("not now", "familiar-ai: the person chose: not now"), ("yes, push and open it", "familiar-ai: the person chose: push and open the pull request")] {
        let post = p.hook("PostToolUse", json!({ "tool_name": "AskUserQuestion", "tool_input": push, "tool_response": { "questions": push["questions"], "answers": { q.clone(): answer } } }));
        assert_eq!(post["hookSpecificOutput"]["additionalContext"], told, "{post}");
    }
    assert_eq!(record(), before, "no fact for the push question");
}

#[test]
fn the_persons_first_words_are_not_an_intent_and_the_status_line_says_idle() {
    let p = project("intent");
    assert_eq!(p.statusline(), "repo | Familiar | ON | idle\n");
    let out = p.hook("UserPromptSubmit", json!({ "prompt": "make greet say hello" }));
    assert!(out.get("systemMessage").is_none(), "a prompt is not the intent: {out}");
    assert_eq!(p.statusline(), "repo | Familiar | ON | idle\n");
    // not set up: a repository with no .familiar folder; nothing outside one
    let bare = project("bare");
    std::fs::remove_dir_all(bare.repo().join(".familiar")).expect("rm");
    assert_eq!(bare.statusline(), "repo | Familiar | not set up\n");
    let input = json!({ "cwd": std::env::temp_dir().to_string_lossy() });
    let (code, out, _) = p.run(&["statusline"], &input.to_string());
    assert_eq!((code, out.as_str()), (0, ""), "outside any repository: nothing");
}
