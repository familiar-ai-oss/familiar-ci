//! The judge remembers what it asked for (the owner's plan of 2026-10-20, step 2; the bench's
//! 13456): each earlier "no" gap of the criterion is in the prompt of every later seal, so a
//! rewrite that drops a test the judge asked for is read against it. Runs the real binary with a
//! fake `claude` first on PATH; no model is called.

mod common;

use common::Project;
use serde_json::json;

const SESSION: &str = "84be6339-0cc7-40ec-91b9-cae44365931b";

const YES: &str = r#"{"type":"result","is_error":false,"result":"{\"answers_intent\": true, \"gap\": \"\", \"reason\": \"it runs greet and checks it says hello.\"}","modelUsage":{"claude-opus-4-6":{}}}"#;
const NO: &str = r#"{"type":"result","is_error":false,"result":"{\"answers_intent\": false, \"gap\": \"greet saying hello when it runs\", \"reason\": \"it greps the file instead of running greet.\"}","modelUsage":{"claude-opus-4-6":{}}}"#;

/// A project planned to c1 (anchor src/app.sh, test tests/test_c1.sh), its holder on haiku, the
/// judge on, the fake `claude` answering what `bin/answer` holds and keeping each prompt.
fn project(name: &str) -> Project {
    let b = Project::builder(name).session(SESSION);
    let d = b.dir().join("bin");
    let fake = format!("#!/bin/sh\nprintf '%s\\n' \"$*\" >> '{d}/calls'\ncat > '{d}/prompt'\ncat '{d}/answer'\n", d = d.display());
    let b = b.bin("claude", &fake);
    let p = b.env_remove("FAMILIAR_AI_JUDGE").create();
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
    p.hook("PostModelSwitch", json!({ "from_model": "claude-opus-4-1", "to_model": "claude-haiku-4-5-20251001" }));
    p
}

impl Project {
    fn answer(&self, answer: &str) {
        std::fs::write(self.dir.join("bin/answer"), answer).expect("answer");
    }

    /// Commit the test (`body`) alone and hand the hook its PostToolUse.
    fn tests_commit(&self, body: &str) {
        self.write("tests/test_c1.sh", body);
        self.git(&["add", "-A"]);
        self.git(&["commit", "-q", "-m", "c1: tests"]);
        self.hook("PostToolUse", json!({ "tool_name": "Bash", "tool_input": { "command": "git commit -m 'c1: tests'" }, "tool_response": {} }));
    }

    fn calls(&self) -> usize {
        std::fs::read_to_string(self.dir.join("bin/calls")).unwrap_or_default().lines().count()
    }

    fn last_prompt(&self) -> String {
        std::fs::read_to_string(self.dir.join("bin/prompt")).unwrap_or_default()
    }
}

const ASKED: &str = "An earlier verdict asked for: greet saying hello when it runs. Is that still exercised? If not, answers_intent is false and gap names it.";

#[test]
fn a_rewrite_after_a_covered_seal_is_read_against_what_an_earlier_no_asked_for() {
    let p = project("remembers");
    // judged no: the test greps the file
    p.answer(NO);
    p.tests_commit("grep -q hello src/app.sh\n");
    assert_eq!(p.calls(), 1);
    assert!(!p.last_prompt().contains("An earlier verdict asked for"), "nothing asked before the first verdict: {}", p.last_prompt());
    // covered: the test runs greet
    p.answer(YES);
    p.tests_commit("sh src/app.sh | grep -qx hello\n");
    assert_eq!(p.calls(), 2);
    assert!(p.last_prompt().contains(ASKED), "the second seal is read against the first no: {}", p.last_prompt());
    // rewritten, announced, without running greet: the third seal is still read against it
    p.claim(&["test_change_announced", "--criterion_id", "c1", "--reason", "a shorter test"]);
    p.tests_commit("grep -q 'echo hello' src/app.sh\n");
    assert_eq!(p.calls(), 3, "the rewrite is a new seal, judged");
    let sent = p.last_prompt();
    assert!(sent.contains(ASKED), "{sent}");
    assert_eq!(sent.matches("An earlier verdict asked for:").count(), 1, "each gap once: {sent}");
    // the standalone builder prints the same
    let (code, printed, err) = p.run(&["judge-prompt", "c1"], "");
    assert_eq!((code, err.as_str()), (0, ""));
    assert_eq!(printed, sent);
}
