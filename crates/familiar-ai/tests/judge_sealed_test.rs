//! The red tests-first commit is stamped and, in the same PostToolUse, the
//! judge one rung above the holder is asked once. Runs the real binary with a
//! fake `claude` first on PATH; the 60 s limit is tested in `door::judge`.

mod common;

use std::process::Command;

use common::Project;
use serde_json::{json, Value};

const SESSION: &str = "84be6339-0cc7-40ec-91b9-cae44365931b";

const YES: &str = r#"{"type":"result","is_error":false,"result":"{\"answers_intent\": true, \"gap\": \"\", \"reason\": \"it checks that greet says hello.\"}","modelUsage":{"claude-opus-4-6":{}}}"#;
const NO: &str = r#"[{"type":"system","subtype":"init"},{"type":"result","is_error":false,"result":"{\"answers_intent\": false, \"gap\": \"greet saying hello when it runs\", \"reason\": \"it greps the file instead of running greet.\"}","modelUsage":{"claude-opus-4-6":{}}}]"#;
const GARBAGE: &str = r#"{"type":"result","is_error":false,"result":"Yes, I think it does."}"#;

/// The argv the fake `claude` is called with for `alias` (the schema).
fn argv(alias: &str) -> String {
    format!(
        "-p --model {alias} --output-format json --json-schema {} --setting-sources  --strict-mcp-config --tools  --max-budget-usd 0.50",
        familiar_ai::door::judge_answer::CONTRACT_SCHEMA
    )
}

/// A set-up project planned to c1 (anchor src/app.sh, test
/// tests/test_c1.sh), its holder's model `model`, the fake `claude`
/// answering `answer`.
fn project(name: &str, model: Option<&str>, answer: &str, judge_off: bool) -> Project {
    said_first(name, model, answer, judge_off, None)
}

/// [`project`], with the prompt `said`, when given, typed before the
/// intent is claimed for it.
fn said_first(name: &str, model: Option<&str>, answer: &str, judge_off: bool, said: Option<&str>) -> Project {
    let b = Project::builder(name).session(SESSION);
    let d = b.dir().join("bin");
    let fake = format!("#!/bin/sh\nprintf '%s\\n' \"$*\" >> '{d}/calls'\ncat >> '{d}/prompts'\ncat '{d}/answer'\n", d = d.display());
    let b = b.bin("claude", &fake);
    std::fs::write(d.join("answer"), answer).expect("answer");
    let p = if judge_off { b.create() } else { b.env_remove("FAMILIAR_AI_JUDGE").create() };
    match said {
        Some(said) => {
            p.hook("UserPromptSubmit", json!({ "prompt": said }));
            p.claim(&["intent_submitted", "--text", "greet with hello"]);
        }
        None => {
            p.claim(&["intent_submitted", "--for", "0", "--text", "greet with hello"]);
        }
    }
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
    if let Some(m) = model {
        p.hook("PostModelSwitch", json!({ "from_model": "claude-opus-4-1", "to_model": m }));
    }
    p
}

impl Project {
    /// Commit the test (`body`) alone and hand the hook its PostToolUse.
    fn tests_commit(&self, body: &str) -> Value {
        self.write("tests/test_c1.sh", body);
        self.git(&["add", "-A"]);
        self.git(&["commit", "-q", "-m", "c1: tests"]);
        self.hook("PostToolUse", json!({ "tool_name": "Bash", "tool_input": { "command": "git commit -m 'c1: tests'" }, "tool_response": {} }))
    }

    /// The fake `claude`'s argv, one line per call.
    fn calls(&self) -> Vec<String> {
        std::fs::read_to_string(self.dir.join("bin/calls")).unwrap_or_default().lines().map(str::to_string).collect()
    }

    /// Every judge_unanswered in the record: (criterion, seal, why).
    fn unanswered(&self) -> Vec<(String, String, String)> {
        let s = |f: &Value, k: &str| f[k].as_str().unwrap_or("").to_string();
        self.facts_of("judge_unanswered").iter().map(|f| (s(f, "criterion_id"), s(f, "seal"), s(f, "why"))).collect()
    }

    /// c1's judge line in `status --json`.
    fn judge(&self) -> Value {
        self.criterion_json("c1")["judge"].clone()
    }

    fn plan(&self) -> String {
        self.read(".familiar/plan")
    }
}

fn context(out: &Value) -> &str {
    out["hookSpecificOutput"]["additionalContext"].as_str().unwrap_or("")
}

#[test]
fn a_yes_is_recorded_and_told_and_the_judge_is_one_rung_up_from_haiku() {
    let p = project("yes", Some("claude-haiku-4-5-20251001"), YES, false);
    let out = p.tests_commit("grep -q hello src/app.sh\n");
    // after the judge's yes, the state shown is sealed (red, covered)
    assert_eq!(context(&out), "familiar-ai: the judge (opus) read your request against the test: it covers it\nfamiliar-ai: ran the tests of \"greet says hello\": they fail, as they should before the fix · sealed (red, covered)\n  the code goes on src/app.sh greet lines 1–3; the test stays as it is");
    assert_eq!(out["systemMessage"], "familiar-ai: the judge (opus) read your request against the test: it covers it", "the verdict is the person's to see too");
    assert_eq!(p.calls(), [argv("opus")]);
    let prompt = std::fs::read_to_string(p.dir.join("bin/prompts")).unwrap_or_default();
    for fact in ["greet with hello", "greet says hello", "src/app.sh · greet · lines 1–3", "greet() { echo hi; }", "THE SEALED TEST tests/test_c1.sh (new in the seal commit)\ngrep -q hello src/app.sh"] {
        assert!(prompt.contains(fact), "{fact} in {prompt}");
    }
    let status = p.status();
    assert!(
        status.contains("familiar-ai: \"greet says hello\"\n  asked for: greet with hello\n  tested by: tests/test_c1.sh\n  judge: it covers the request (opus)"),
        "{status}"
    );
    assert_eq!(p.judge(), "covers");
    assert_eq!(p.state_name("c1"), "sealed");
    assert!(!p.plan().contains("test_change_announced"), "a yes announces nothing");
    // the next hook call does not ask again: one verdict per seal
    p.hook("PostToolUse", json!({ "tool_name": "Bash", "tool_input": { "command": "ls" }, "tool_response": {} }));
    assert_eq!(p.calls().len(), 1);
}

#[test]
fn a_no_announces_the_change_and_the_next_tests_commit_is_a_new_seal() {
    let p = project("no", Some("claude-haiku-4-5-20251001"), NO, false);
    let out = p.tests_commit("grep -q hello src/app.sh\n");
    assert_eq!(
        context(&out),
        "familiar-ai: the judge (opus) read your request against the test: it does not cover greet saying hello when it runs; the test may be changed\n  the judge's reason: it greps the file instead of running greet; the next commit of tests alone is sealed again\nfamiliar-ai: ran the tests of \"greet says hello\": they fail, as they should before the fix · stamped"
    );
    assert!(p.plan().contains(r#""kind":"test_change_announced","criterion_id":"c1","reason":"judge: not exercised: greet saying hello when it runs; it greps the file instead of running greet.""#), "{}", p.plan());
    assert!(p.status().contains("  judge: it does not cover the request (opus)"));
    assert_eq!(p.judge(), "does not cover");
    // announced: the sealed test may be edited without proof_tampered
    let edit = p.hook("PreToolUse", json!({ "tool_name": "Edit", "tool_input": { "file_path": p.path("tests/test_c1.sh").to_string_lossy() } }));
    assert!(!edit.to_string().contains("deny"), "{edit}");
    std::fs::write(p.dir.join("bin/answer"), YES).expect("answer");
    let out = p.tests_commit("sh src/app.sh | grep -qx hello\n");
    assert!(context(&out).starts_with("familiar-ai: the judge (opus) read your request against the test: it covers it"), "{out}");
    assert_eq!(p.calls().len(), 2, "the new seal is judged");
    assert_eq!(p.judge(), "covers");
}

#[test]
fn an_answer_that_is_not_the_one_object_is_unanswered_and_twice_is_not_judged() {
    let p = project("garbage", Some("claude-haiku-4-5-20251001"), GARBAGE, false);
    let out = p.tests_commit("grep -q hello src/app.sh\n");
    assert_eq!(out["systemMessage"], "familiar-ai: the judge did not answer (exit 0, but its answer was not the one JSON object); the sealed test stands, not judged; its call is logged in .familiar/judge.log");
    let log = p.read(".familiar/judge.log");
    let schema = familiar_ai::door::judge_answer::CONTRACT_SCHEMA;
    assert!(log.contains(&format!("· criterion c1 · claude -p --model opus --output-format json --json-schema {schema} --setting-sources '' --strict-mcp-config --tools '' --max-budget-usd 0.50 · exit 0 · ")), "{log}");
    assert!(log.contains("stdout (first 2000 characters):\n{\"type\":\"result\",\"is_error\":false,\"result\":\"Yes, I think it does.\"}"), "{log}");
    let ignored = Command::new("git").args(["check-ignore", "-q", ".familiar/judge.log"]).current_dir(p.repo()).status().expect("git");
    assert!(ignored.success(), "the log stays on the laptop");
    // a red seal with no verdict waits on the judge
    assert_eq!(context(&out), "familiar-ai: ran the tests of \"greet says hello\": they fail, as they should before the fix · stamped · waiting on the judge\n  the code goes on src/app.sh greet lines 1–3; the test stays as it is");
    assert!(p.status().contains("  judge: not judged yet"));
    assert_eq!(p.judge(), "waiting");
    // one judge_unanswered ⇒ the next hook call asks again (one judge per call)
    assert_eq!(p.unanswered(), [("c1".to_string(), p.head(), "unparseable".to_string())]);
    let ls = json!({ "tool_name": "Bash", "tool_input": { "command": "ls" }, "tool_response": {} });
    let again = p.hook("PostToolUse", ls.clone());
    assert_eq!(again["systemMessage"], out["systemMessage"], "{again}");
    assert_eq!(p.calls().len(), 2, "asked again: one unanswered call");
    // two for the same seal ⇒ not judged: no judge asked again, status says why
    assert_eq!(p.unanswered().len(), 2);
    assert!(p.status().contains("  judge: not judged (unparseable, twice)"), "{}", p.status());
    assert_eq!(p.judge(), "not judged");
    p.hook("PostToolUse", ls.clone());
    p.hook("Stop", json!({}));
    assert_eq!(p.calls().len(), 2, "not judged: the judge is not asked again for this seal");
    // a verdict written after them still counts
    let record = familiar_ai_facts::Record::open_waiting(&p.record_path(), std::time::Duration::from_secs(5)).expect("record");
    let yes = json!({ "criterion_id": "c1", "model": "claude-opus-4-6", "answers_intent": "yes", "reason": "r" });
    record.append_raw("judge_verdict", yes.as_object().cloned().unwrap_or_default(), 9.0, familiar_ai::TENANT).expect("verdict");
    drop(record);
    assert_eq!(p.judge(), "covers");
}

#[test]
fn off_never_asks() {
    let p = project("off", Some("claude-haiku-4-5-20251001"), YES, true);
    let out = p.tests_commit("grep -q hello src/app.sh\n");
    assert_eq!(out, json!({ "hookSpecificOutput": { "hookEventName": "PostToolUse", "additionalContext": "familiar-ai: ran the tests of \"greet says hello\": they fail, as they should before the fix · stamped\n  the code goes on src/app.sh greet lines 1–3; the test stays as it is" } }));
    assert!(p.calls().is_empty());
    // the judge off says so; the seal stands on its red run alone
    assert!(p.status().contains("  judge: off: the person chose no judge, so the seal stands on its red run alone"), "{}", p.status());
}

#[test]
fn no_model_seen_and_fable_are_both_asked_of_fable() {
    for (name, model) in [("unknown", None), ("fable", Some("claude-fable-5"))] {
        let p = project(name, model, YES, false);
        p.tests_commit("grep -q hello src/app.sh\n");
        assert_eq!(p.calls(), [argv("fable")], "{name}");
    }
}

#[test]
fn a_judge_that_fails_names_its_exit_and_the_log_keeps_its_stderr() {
    let p = project("fails", Some("claude-haiku-4-5-20251001"), YES, false);
    // the shape of a sandbox's silent "unjudged": the child says why on stderr and exits 1
    let fake = "#!/bin/sh\ncat > /dev/null\necho 'API Error: 403 the proxy refused the CONNECT' >&2\nexit 1\n";
    std::fs::write(p.dir.join("bin/claude"), fake).expect("fake claude");
    let out = p.tests_commit("grep -q hello src/app.sh\n");
    assert_eq!(out["systemMessage"], "familiar-ai: the judge did not answer (exit 1); the sealed test stands, not judged; its call is logged in .familiar/judge.log");
    let log = p.read(".familiar/judge.log");
    assert!(log.contains(" · exit 1 · ") && log.contains("stderr (first 2000 characters):\nAPI Error: 403 the proxy refused the CONNECT\n"), "{log}");
    assert_eq!(p.judge(), "waiting");
    assert_eq!(p.unanswered(), [("c1".to_string(), p.head(), "error".to_string())], "why = error");
}

#[test]
fn a_holder_no_fact_names_is_read_from_the_transcript_before_the_rung() {
    // headless Claude Code (the bench) names no model at SessionStart: the
    // judge would ask fable; the transcript names the holder's model
    let p = project("transcript", None, YES, false);
    let transcript = p.dir.join("transcript.jsonl");
    let lines = [
        r#"{"type":"user","message":{"role":"user","content":"fix it"}}"#,
        r#"{"type":"assistant","message":{"model":"us.anthropic.claude-haiku-4-5-20251001-v1:0","content":[]}}"#,
    ];
    std::fs::write(&transcript, lines.join("\n")).expect("transcript");
    p.write("tests/test_c1.sh", "grep -q hello src/app.sh\n");
    p.git(&["add", "-A"]);
    p.git(&["commit", "-q", "-m", "c1: tests"]);
    let out = p.hook(
        "PostToolUse",
        json!({ "tool_name": "Bash", "tool_input": { "command": "git commit" }, "tool_response": {}, "transcript_path": transcript.to_string_lossy() }),
    );
    assert!(context(&out).starts_with("familiar-ai: the judge (opus) read your request against the test: it covers it"), "{out}");
    assert_eq!(p.calls(), [argv("opus")], "haiku ⇒ opus (never below opus), not fable");
}

#[test]
fn the_persons_prompt_is_the_intent_and_judge_prompt_prints_what_was_sent() {
    // the person types; the agent's own intent claim is its reading
    let said = "Make greet say hello instead of hi, and keep the exit code 0.";
    let p = said_first("person", Some("claude-haiku-4-5-20251001"), YES, false, Some(said));
    let out = p.tests_commit("grep -q hello src/app.sh\n");
    assert!(context(&out).starts_with("familiar-ai: the judge (opus) read your request against the test: it covers it"), "{out}");
    let sent = std::fs::read_to_string(p.dir.join("bin/prompts")).unwrap_or_default();
    assert!(sent.contains(&format!("THE INTENT (what the person asked for, in their own words)\n{said}\n\nTHE AGENT'S READING (what the agent wrote down; not the person's words)\nthe intent, as the agent put it: greet with hello\ncriterion c1: greet says hello\n")), "{sent}");
    assert!(sent.contains("The question: list every path the person's words cover that criterion c1 claims (nested cases, other entry points, each input class)"), "{sent}");
    assert!(sent.contains("1. Name each noun and each named form in the person's words") && sent.contains("2. Does at least one assertion run the changed behaviour"), "{sent}");
    assert!(sent.ends_with("If you cannot see the tests, the answer is:\n{\"answers_intent\": false, \"gap\": \"\", \"reason\": \"the test is not visible\", \"cases\": []}\n"), "{sent}");
    // the standalone builder prints the same text, asks no model, writes nothing
    let before = std::fs::metadata(p.record_path()).map(|m| m.len()).unwrap_or(0);
    let (code, printed, err) = p.run(&["judge-prompt", "c1"], "");
    assert_eq!((code, err.as_str()), (0, ""));
    assert_eq!(printed, sent, "the very prompt the judge was sent");
    assert_eq!(p.calls().len(), 1, "no model asked");
    assert_eq!(std::fs::metadata(p.record_path()).map(|m| m.len()).unwrap_or(0), before);
    let (code, _, err) = p.run(&["judge-prompt", "c9"], "");
    assert_eq!((code, err.as_str()), (1, "familiar-ai: criterion c9 has no seal in this record: the judge is asked at a seal\n"));
    // the status shows the person's prompt, the agent's reading beneath
    assert!(p.status().contains(&format!("familiar-ai: \"greet says hello\"\n  asked for: {said}\n  the agent's reading: greet with hello\n")), "{}", p.status());
}

#[test]
fn a_judge_that_cannot_see_the_test_leaves_the_seal_unjudged_never_a_no() {
    const UNSEEN: &str = r#"{"type":"result","is_error":false,"result":"{\"answers_intent\": false, \"gap\": \"\", \"reason\": \"the test is not visible\"}","modelUsage":{"claude-opus-4-6":{}}}"#;
    let p = project("unseen", Some("claude-haiku-4-5-20251001"), UNSEEN, false);
    let out = p.tests_commit("grep -q hello src/app.sh\n");
    assert_eq!(
        out["systemMessage"].as_str().unwrap_or(""),
        "familiar-ai: the judge (opus) could not see the sealed test (the test is not visible); it stands, not judged; its call is logged in .familiar/judge.log"
    );
    assert!(!p.plan().contains("test_change_announced"), "not a no: {}", p.plan());
    assert_eq!(p.judge(), "waiting");
    assert_eq!(p.unanswered(), [("c1".to_string(), p.head(), "cannot_see".to_string())], "why = cannot_see");
}

#[test]
fn a_modified_test_file_whose_seal_diff_holds_no_test_function_is_shown_as_its_test_functions() {
    // the seal commit changes only an import line of a planned test file
    let p = project("functions", Some("claude-haiku-4-5-20251001"), YES, false);
    let before = "import pytest\nfrom app import greet\n\n\ndef helper():\n    return 'hello'\n\n\ndef test_greet_says_hello():\n    assert greet() == helper()\n\n\nclass TestGreet:\n    def test_twice(self):\n        assert greet() == greet()\n";
    p.write("tests/test_greet.py", before);
    p.git(&["add", "-A"]);
    p.git(&["commit", "-q", "-m", "the tests that were there"]);
    p.hook("PostToolUse", json!({ "tool_name": "Bash", "tool_input": { "command": "ls" }, "tool_response": {} }));
    p.claim(&["test_planned", "--criterion_id", "c1", "--file_path", "tests/test_greet.py", "--will", "modify"]);
    p.write("tests/test_greet.py", &before.replace("from app import greet", "from app import greet, shout"));
    p.tests_commit("grep -q hello src/app.sh\n");
    assert_eq!(p.calls().len(), 1, "the seal was judged");
    let (code, printed, err) = p.run(&["judge-prompt", "c1"], "");
    assert_eq!((code, err.as_str()), (0, ""));
    let section = "THE SEALED TEST tests/test_greet.py (changed by the seal commit: its test functions as at the seal, the ones the seal changed first)\n\
                   changed in the seal: no test function (only line 2, outside every test function)\n\
                   import pytest\nfrom app import greet, shout\n\n\
                   def test_greet_says_hello():\n    assert greet() == helper()\n\n\
                   class TestGreet:\n    def test_twice(self):\n        assert greet() == greet()\n";
    assert!(printed.contains(section), "{printed}");
    assert!(!printed.contains("@@ lines") && !printed.contains("+ from app"), "never a bare diff: {printed}");
    let sent = std::fs::read_to_string(p.dir.join("bin/prompts")).unwrap_or_default();
    assert!(sent.contains(section), "the judge was sent the same: {sent}");
}
