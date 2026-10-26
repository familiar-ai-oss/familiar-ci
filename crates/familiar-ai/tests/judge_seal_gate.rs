//! The judge gates code on the seal: code is refused while the verdict is
//! pending or no and goes ahead after yes. Runs the real binary with a fake
//! `claude` first on PATH; the real model is never called.

mod common;

use std::path::Path;
use std::process::Command;

use common::Project;
use serde_json::{json, Value};

const SESSION: &str = "84be6339-0cc7-40ec-91b9-cae44365931b";

const YES: &str = r#"{"type":"result","is_error":false,"result":"{\"answers_intent\": true, \"gap\": \"\", \"reason\": \"it checks that greet says hello.\"}","modelUsage":{"claude-opus-4-6":{}}}"#;
const NO: &str = r#"{"type":"result","is_error":false,"result":"{\"answers_intent\": false, \"gap\": \"greet saying hello when it runs\", \"reason\": \"it greps the file instead of running greet.\"}","modelUsage":{"claude-opus-4-6":{}}}"#;
const GARBAGE: &str = r#"{"type":"result","is_error":false,"result":"Yes, I think it does."}"#;

const WAITING: &str = "familiar-ai: \"greet says hello\" · the judge reads the sealed test against your request first · next: wait for the judge";
const GATE_NO: &str = "familiar-ai: \"greet says hello\" · the judge found it does not cover greet saying hello when it runs · next: change the test, commit it alone, it must fail first";
const FOUND: &str = "the judge (opus) found the test does not cover greet saying hello when it runs; the test may be changed";

/// A set-up project planned to c1 (anchor src/app.sh, test
/// tests/test_c1.sh), its holder on haiku, the fake `claude` answering
/// `answer`; with `two`, c2 strung after it (anchor src/bye.sh).
fn project(name: &str, answer: &str, judge_on: bool, two: bool) -> Project {
    let b = Project::builder(name).session(SESSION).file("src/bye.sh", "#!/bin/sh\nbye() { echo bye; }\nbye\n");
    let d = b.dir().join("bin");
    let fake = format!("#!/bin/sh\nprintf '%s\\n' \"$*\" >> '{d}/calls'\ncat > /dev/null\ncat '{d}/answer'\n", d = d.display());
    let b = b.bin("claude", &fake);
    std::fs::write(d.join("answer"), answer).expect("answer");
    let p = if judge_on { b.env_remove("FAMILIAR_AI_JUDGE").create() } else { b.create() };
    p.claim(&["intent_submitted", "--for", "0", "--text", "greet with hello"]);
    p.claim(&["behavior", "--behavior_id", "b1", "--text", "the greeting"]);
    p.claim(&["promise", "--promise_id", "p1", "--behavior_id", "b1", "--accepts", "a name", "--gives_back", "a greeting", "--refuses", "nothing", "--cases", r#"[{"case_id":"n1","kind":"normal","given":"a name","expect":"a greeting"},{"case_id":"o1","kind":"opposite","given":"no name","expect":"no greeting"}]"#]);
    p.claim(&["criterion", "--criterion_id", "c1", "--text", "greet says hello", "--path", "full", "--behavior_id", "b1", "--promise_id", "p1"]);
    p.cases_kept("c1"); // the cases card passed (plan cases-card, 2026-10-25) [draft names]
    p.claim(&["place_chosen", "--criterion_id", "c1", "--places", r#"[{"file_path":"src/app.sh","symbol":"greet","line_start":1,"line_end":3}]"#]);
    p.claim(&["test_planned", "--criterion_id", "c1", "--file_path", "tests/test_c1.sh", "--will", "create"]);
    if two {
        p.claim(&["behavior", "--behavior_id", "b1", "--text", "the greeting"]);
        p.claim(&["promise", "--promise_id", "p1", "--behavior_id", "b1", "--accepts", "a name", "--gives_back", "a greeting", "--refuses", "nothing", "--cases", r#"[{"case_id":"n1","kind":"normal","given":"a name","expect":"a greeting"},{"case_id":"o1","kind":"opposite","given":"no name","expect":"no greeting"}]"#]);
        p.claim(&["criterion", "--criterion_id", "c2", "--text", "bye says farewell", "--path", "full", "--behavior_id", "b1", "--promise_id", "p1"]);
        p.cases_kept("c2"); // the cases card passed (plan cases-card, 2026-10-25) [draft names]
        p.claim(&["place_chosen", "--criterion_id", "c2", "--places", r#"[{"file_path":"src/bye.sh","symbol":"bye","line_start":1,"line_end":3}]"#]);
        p.claim(&["test_planned", "--criterion_id", "c2", "--file_path", "tests/test_c2.sh", "--will", "create", "--test_command", "sh tests/test_c2.sh"]);
        // each criterion claims its own command, so both are scoped and the unscoped gate holds neither
        p.claim(&["test_planned", "--criterion_id", "c1", "--file_path", "tests/test_c1.sh", "--will", "create", "--test_command", "sh tests/test_c1.sh"]);
    }
    p.claim(&["test_stack_detected", "--project_root", ".", "--test_stack", "sh", "--test_command", "for f in tests/test_*.sh; do sh \"$f\" || exit 1; done"]);
    p.claim(&["criterion_head", "--criterion_id", "c1"]);
    p.claim(&["criterion_assigned", "--criterion_id", "c1"]);
    p.claim(&["fix_location_category", "--criterion_id", "c1", "--category", "this_repository", "--where", ""]);
    if two {
        p.claim(&["criterion_strung_on_head", "--criterion_id", "c2", "--head", "c1"]);
        p.claim(&["criterion_assigned", "--criterion_id", "c2"]);
        p.claim(&["fix_location_category", "--criterion_id", "c2", "--category", "this_repository", "--where", ""]);
    }
    p.hook("PostModelSwitch", json!({ "from_model": "claude-opus-4-1", "to_model": "claude-haiku-4-5-20251001" }));
    p
}

impl Project {
    /// Turns the judge on or off for the calls from now on.
    fn judge(&self, on: bool) {
        self.set_env("FAMILIAR_AI_JUDGE", if on { None } else { Some("off") });
    }

    fn answer(&self, answer: &str) {
        std::fs::write(self.dir.join("bin/answer"), answer).expect("answer");
    }

    /// Commit `body` as the test of `c` alone and hand the hook its PostToolUse.
    fn tests_commit(&self, c: &str, body: &str) -> Value {
        self.write(&format!("tests/test_{c}.sh"), body);
        self.git(&["add", "-A"]);
        self.git(&["commit", "-q", "-m", &format!("{c}: tests")]);
        self.hook("PostToolUse", json!({ "tool_name": "Bash", "tool_input": { "command": "git commit -m tests" }, "tool_response": {} }))
    }

    /// Commit the code that makes c1's test pass and hand the hook its PostToolUse.
    fn code_commit(&self) -> Value {
        self.write("src/app.sh", "#!/bin/sh\ngreet() { echo hello; }\ngreet\n");
        self.git(&["add", "-A"]);
        self.git(&["commit", "-q", "-m", "c1: code"]);
        self.hook("PostToolUse", json!({ "tool_name": "Bash", "tool_input": { "command": "git commit -am code" }, "tool_response": {} }))
    }

    fn edit(&self, file: &str) -> Value {
        self.hook("PreToolUse", json!({ "tool_name": "Edit", "tool_input": { "file_path": self.path(file).to_string_lossy() } }))
    }

    fn calls(&self) -> usize {
        std::fs::read_to_string(self.dir.join("bin/calls")).unwrap_or_default().lines().count()
    }

    fn statusline(&self) -> String {
        let input = json!({ "workspace": { "project_dir": self.repo().to_string_lossy() } }).to_string();
        let mut cmd = self.command();
        cmd.arg("statusline");
        Project::output(cmd, &input).1.trim_end().to_string()
    }
}

fn denied(out: &Value) -> Option<&str> {
    let h = &out["hookSpecificOutput"];
    (h["permissionDecision"] == "deny").then(|| h["permissionDecisionReason"].as_str().unwrap_or(""))
}

/// 1 when status shows the judge's answer for the criterion named `text`, else 0.
fn verdicts(p: &Project, text: &str) -> usize {
    let status = p.status();
    let Some(at) = status.find(&format!("familiar-ai: \"{text}\"\n  asked for: ")) else { return 0 };
    let judge = status[at..].lines().find(|l| l.starts_with("  judge: ")).unwrap_or("");
    usize::from(judge.starts_with("  judge: it covers") || judge.starts_with("  judge: it does not cover"))
}

#[test]
fn a_seal_stamped_with_the_judge_off_is_judged_by_any_later_hook_call_once() {
    let p = project("later", YES, false, false);
    p.tests_commit("c1", "grep -q hello src/app.sh\n");
    assert_eq!(p.calls(), 0, "the judge was off at the seal");
    p.judge(true);
    // a hook call that stamps nothing: the backstop asks the judge
    p.hook("UserPromptSubmit", json!({ "prompt": "carry on" }));
    assert_eq!((p.calls(), verdicts(&p, "greet says hello")), (1, 1));
    p.bash("PreToolUse", "ls");
    p.hook("Stop", json!({}));
    assert_eq!(p.calls(), 1, "once per seal: a verdict answers it");
    assert!(p.status().contains("familiar-ai: \"greet says hello\" · sealed (red, covered) · the code comes next"), "{}", p.status());
    assert!(p.statusline().ends_with(" · 1 sealed"), "{}", p.statusline());
}

#[test]
fn two_unjudged_seals_take_one_judge_per_hook_call_the_oldest_first() {
    let p = project("oldest", YES, false, true);
    p.tests_commit("c1", "grep -q hello src/app.sh\n");
    p.code_commit();
    p.tests_commit("c2", "grep -q farewell src/bye.sh\n");
    assert!(p.status().contains("\"bye says farewell\" · the test fails, as it should before the fix"), "{}", p.status());
    p.judge(true);
    p.bash("PostToolUse", "ls");
    assert_eq!((p.calls(), verdicts(&p, "greet says hello"), verdicts(&p, "bye says farewell")), (1, 1, 0), "the oldest seal first, one judge per call");
    p.bash("PostToolUse", "ls");
    assert_eq!((p.calls(), verdicts(&p, "greet says hello"), verdicts(&p, "bye says farewell")), (2, 1, 1));
}

#[test]
fn the_investigators_child_keeps_the_judge_and_its_own_hooks_judge_its_seal() {
    // the launch: only FAMILIAR_AI_INVESTIGATOR=off and its conversation are set
    let dir = common::temp_dir("launch");
    std::fs::create_dir_all(&dir).expect("dir");
    let fake = dir.join("claude");
    std::fs::write(&fake, "#!/bin/sh\ncat > /dev/null\necho \"${FAMILIAR_AI_JUDGE-unset}|$FAMILIAR_AI_INVESTIGATOR|$FAMILIAR_AI_CONVERSATION_ID\"\n").expect("fake");
    assert!(Command::new("chmod").arg("+x").arg(&fake).status().expect("chmod").success());
    let go = familiar_ai::door::investigator_run::Launch {
        criterion_id: "c1".into(),
        conversation: "s1/u1".into(),
        session_id: "u1".into(),
        rung: "sonnet",
        prompt: String::new(),
        permission_mode: None,
    };
    let i = familiar_ai::input::Investigating { program: fake, plugin_dir: dir.clone(), budget: "1.00".into(), wait: std::time::Duration::from_secs(10), claude_dir: None };
    let ran = familiar_ai::door::investigator_run::launch(&go, Path::new(&dir), &i);
    // whatever this test process carries is handed down; nothing switches the judge off
    let inherited = std::env::var("FAMILIAR_AI_JUDGE").unwrap_or_else(|_| "unset".into());
    assert_eq!(ran.stdout, format!("{inherited}|off|s1/u1\n"));
    let _ = std::fs::remove_dir_all(&dir);

    // a hook call in the investigator's session (its conversation set by the binary) judges the seal
    let p = project("investigator", NO, true, false);
    p.set_env("FAMILIAR_AI_CONVERSATION_ID", Some(&format!("{SESSION}/u1")));
    p.write("tests/test_c1.sh", "grep -q hello src/app.sh\n");
    p.git(&["add", "-A"]);
    p.git(&["commit", "-q", "-m", "c1: tests"]);
    let out = p.hook_with("PostToolUse", &p.payload_as("u1", "PostToolUse", json!({ "tool_name": "Bash", "tool_input": { "command": "git commit" }, "tool_response": {} })));
    assert_eq!((p.calls(), verdicts(&p, "greet says hello")), (1, 1), "{out}");
    assert!(out.to_string().contains("it does not cover greet saying hello when it runs"), "{out}");
}

#[test]
fn code_waits_for_the_judge_then_its_no_sends_the_agent_back_to_the_test() {
    // the seal stamped with the judge off; then on, answering garbage: the
    // edit's own call asks it (one judge_unanswered) and still waits
    let p = project("gate", GARBAGE, false, false);
    p.tests_commit("c1", "grep -q hello src/app.sh\n");
    p.judge(true);
    assert!(p.status().contains("familiar-ai: \"greet says hello\" · waiting on the judge"), "{}", p.status());
    assert!(p.statusline().ends_with(" · 1 waits"), "{}", p.statusline());
    assert_eq!(denied(&p.edit("src/app.sh")), Some(WAITING));
    assert_eq!(p.calls(), 1, "one unanswered call: still waiting");
    // its "no", read in the very call that refuses: the words name the gap,
    // then the judge's own line
    p.answer(NO);
    let refused = p.edit("src/app.sh");
    assert!(denied(&refused).is_some_and(|r| r.starts_with(&format!("{GATE_NO}\nfamiliar-ai: the judge (opus) read your request"))), "{refused}");
    // held: an Edit of the code, a shell write of it, a commit carrying it
    assert_eq!(denied(&p.edit("src/app.sh")), Some(GATE_NO));
    assert_eq!(denied(&p.edit("src/elsewhere.sh")), Some(GATE_NO), "any code, for the active criterion");
    assert_eq!(denied(&p.bash("PreToolUse", "echo 'greet() { echo hello; }' > src/app.sh")), Some(GATE_NO));
    p.write("src/app.sh", "#!/bin/sh\ngreet() { echo hello; }\ngreet\n");
    assert_eq!(denied(&p.bash("PreToolUse", "git commit -am code")), Some(GATE_NO));
    p.write("src/app.sh", "#!/bin/sh\ngreet() { echo hi; }\ngreet\n");
    assert_eq!(denied(&p.bash("PreToolUse", "git commit -m nothing-new")), None, "the anchor as HEAD has it: no code to carry");
    assert_eq!(p.calls(), 2, "once its verdict is written");
    assert!(p.status().contains(&format!("familiar-ai: \"greet says hello\" · {FOUND}")), "{}", p.status());
    // the test is the way out: announced by the judge's reason, it may be edited
    assert_eq!(denied(&p.edit("tests/test_c1.sh")), None);
    let stop = p.hook("Stop", json!({}));
    assert!(stop["reason"].as_str().unwrap_or("").contains(&format!("· {FOUND}\n  next: change the test, commit it alone, it must fail first")), "{stop}");
}

#[test]
fn code_goes_ahead_after_the_judges_yes() {
    let p = project("yes", YES, true, false);
    let out = p.tests_commit("c1", "grep -q hello src/app.sh\n");
    assert!(out.to_string().contains("they fail, as they should before the fix · sealed (red, covered)"), "{out}");
    assert_eq!(denied(&p.edit("src/app.sh")), None);
    p.write("src/app.sh", "#!/bin/sh\ngreet() { echo hello; }\ngreet\n");
    assert_eq!(denied(&p.bash("PreToolUse", "git commit -am code")), None);
    let out = p.code_commit();
    assert!(out.to_string().contains("they pass · stamped"), "{out}");
    assert!(p.statusline().ends_with(" · 1 green"), "{}", p.statusline());
    assert!(p.hook("Stop", json!({})).get("decision").is_none());
}

#[test]
fn a_no_after_green_refuses_the_stop_and_the_state_is_not_green() {
    let p = project("green", NO, false, false);
    p.tests_commit("c1", "grep -q hello src/app.sh\n");
    p.code_commit();
    assert!(p.status().contains("familiar-ai: \"greet says hello\" · locally green"), "{}", p.status());
    p.judge(true);
    let stop = p.hook("Stop", json!({}));
    assert_eq!(p.calls(), 1, "the stop's own call asked the judge");
    assert_eq!(stop["decision"], "block", "{stop}");
    assert!(
        stop["reason"].as_str().unwrap_or("").starts_with(&format!("familiar-ai: not done yet · \"greet says hello\" · {FOUND}\n  next: change the test, commit it alone, it must fail first")),
        "{stop}"
    );
    let status = p.status();
    assert!(status.contains(&format!("familiar-ai: \"greet says hello\" · {FOUND}")) && !status.contains("locally green"), "{status}");
    let line = p.statusline();
    assert!(line.ends_with(" · 1 testing") && !line.contains("green"), "{line}");
    // a fact, not the switch: with the judge off now, the "no" still stops the stop
    p.judge(false);
    assert_eq!(p.hook("Stop", json!({}))["decision"], "block");
}

#[test]
fn with_the_judge_off_there_is_no_gate_and_status_says_so() {
    let p = project("off", YES, false, false);
    p.tests_commit("c1", "grep -q hello src/app.sh\n");
    assert_eq!(denied(&p.edit("src/app.sh")), None);
    p.write("src/app.sh", "#!/bin/sh\ngreet() { echo hello; }\ngreet\n");
    assert_eq!(denied(&p.bash("PreToolUse", "git commit -am code")), None);
    let status = p.status();
    assert!(status.contains("familiar-ai: \"greet says hello\" · the test fails, as it should before the fix · the code comes next"), "{status}");
    assert!(status.contains("  judge: off: the person chose no judge, so the seal stands on its red run alone"), "{status}");
    assert_eq!(p.calls(), 0);
}

#[test]
fn two_calls_with_no_verdict_for_one_seal_lift_the_gate_and_stop_the_asking() {
    // unparseable, then an error: two judge_unanswered for one seal
    let p = project("twice", GARBAGE, true, false);
    p.tests_commit("c1", "grep -q hello src/app.sh\n");
    assert_eq!(p.calls(), 1);
    assert!(p.status().contains("  judge: not judged yet"), "{}", p.status());
    std::fs::write(p.dir.join("bin/claude"), "#!/bin/sh\nprintf '%s\\n' \"$*\" >> \"$(dirname \"$0\")/calls\"\ncat > /dev/null\nexit 1\n").expect("fake");
    // the second call (an Edit of the code) is not answered either: not judged, the edit goes ahead
    assert_eq!(denied(&p.edit("src/app.sh")), None, "not judged: the gate lifts");
    assert_eq!(p.calls(), 2);
    let status = p.status();
    assert!(status.contains("  judge: not judged (error, twice)"), "{status}");
    assert!(status.contains("familiar-ai: \"greet says hello\" · the test fails, as it should before the fix · the code comes next"), "{status}");
    assert!(!p.statusline().contains("waiting on the judge"), "{}", p.statusline());
    // no further calls for this seal, on any hook
    p.bash("PreToolUse", "ls");
    p.write("src/app.sh", "#!/bin/sh\ngreet() { echo hello; }\ngreet\n");
    assert_eq!(denied(&p.bash("PreToolUse", "git commit -am code")), None);
    let out = p.code_commit();
    assert!(out.to_string().contains("they pass · stamped"), "{out}");
    p.hook("Stop", json!({}));
    assert_eq!(p.calls(), 2, "the judge is not asked again for a seal not judged");
}
