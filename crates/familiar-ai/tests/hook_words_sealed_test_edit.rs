//! An edit of a sealed test is refused with the person's words beside it: the
//! PreToolUse of an edit of a sealed test file says the hold and what comes next,
//! then what the person asked for, the agent's reading of it and the judge's word
//! on the seal, then the claim that announces the change, so a test is updated
//! to the definition and never loosened in the dark.

mod common;

use common::Project;
use serde_json::{json, Value};

const SESSION: &str = "5c4b3a29-1807-4f6e-8d5c-4b3a29180716";

const YES: &str = r#"{"type":"result","is_error":false,"result":"{\"answers_intent\": true, \"gap\": \"\", \"reason\": \"it runs greet and checks it says hello.\"}","modelUsage":{"claude-opus-4-6":{}}}"#;

const HOLD: &str = "familiar-ai: \"greet says hello\" · the sealed test was changed; fix the code without editing the test, or commit the changed test alone and see it fail · next: announce the test change first";
const ASKED: &str = "  asked for: make greet say hello instead of hi";
const READING: &str = "  the agent's reading: greet with hello";
const CLAIM: &str = "  claim it: familiar-ai claim test_change_announced --criterion_id c1 --reason 'the assertion named the wrong file'";

/// c1 planned and its test sealed red; with `judge`, a fake `claude` answers the judge `judge`.
fn sealed(name: &str, judge: Option<&str>) -> Project {
    let b = Project::builder(name).session(SESSION);
    let p = match judge {
        Some(answer) => {
            let d = b.dir().join("bin");
            let fake = format!("#!/bin/sh\ncat > /dev/null\ncat '{}/answer'\n", d.display());
            let b = b.bin("claude", &fake);
            std::fs::write(d.join("answer"), answer).expect("answer");
            b.env_remove("FAMILIAR_AI_JUDGE").create()
        }
        None => b.create(),
    };
    p.hook("UserPromptSubmit", json!({ "prompt": "make greet say hello instead of hi" }));
    let t = p.status_json()["said"][0]["for"].as_u64().expect("the prompt's t");
    p.claim(&["intent_submitted", "--for", &t.to_string(), "--text", "greet with hello"]);
    p.claim(&["one_reading", "--why", "the ask names what must change and how"]);
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
    p.write("tests/test_c1.sh", "sh src/app.sh | grep -qx hello\n");
    p.commit("c1: tests first");
    assert_eq!(p.state_name("c1"), "sealed");
    p
}

/// The refusal's words for the PreToolUse of `out`.
fn reason(out: &Value) -> &str {
    assert_eq!(out["hookSpecificOutput"]["permissionDecision"], "deny", "{out}");
    out["hookSpecificOutput"]["permissionDecisionReason"].as_str().unwrap_or("")
}

#[test]
fn an_edit_of_the_sealed_test_is_refused_with_the_persons_words_the_reading_and_the_judges_word_before_the_claim() {
    let p = sealed("off", None);
    let out = p.tool("PreToolUse", "Edit", json!({ "file_path": p.path("tests/test_c1.sh").to_string_lossy() }));
    let judge = "  judge: off: the person chose no judge, so the seal stands on its red run alone";
    assert_eq!(reason(&out), [HOLD, ASKED, READING, judge, CLAIM].join("\n"));
}

#[test]
fn the_judges_yes_on_the_seal_is_beside_the_persons_words() {
    let p = sealed("yes", Some(YES));
    assert_eq!(p.criterion_json("c1")["judge"], "covers");
    let out = p.tool("PreToolUse", "Write", json!({ "file_path": p.path("tests/test_c1.sh").to_string_lossy(), "content": "true\n" }));
    let judge = "  judge: it covers the request (opus)";
    assert_eq!(reason(&out), [HOLD, ASKED, READING, judge, CLAIM].join("\n"));
}

#[test]
fn a_shell_write_of_the_sealed_test_is_refused_with_the_same_words() {
    let p = sealed("shell", None);
    let out = p.bash("PreToolUse", "sed -i 's/hello/hi/' tests/test_c1.sh");
    let words = reason(&out);
    let judge = "  judge: off: the person chose no judge, so the seal stands on its red run alone";
    assert!(words.starts_with(&[HOLD, ASKED, READING, judge, CLAIM].join("\n")), "{words}");
}

#[test]
fn the_edit_of_another_criterions_sealed_test_names_that_criterion_its_judge_and_its_claim() {
    // c1 green, so the conversation works on c2; the edit is of c1's sealed test
    let p = Project::builder("other").session(SESSION).create();
    p.hook("UserPromptSubmit", json!({ "prompt": "make greet say hello instead of hi, and bye say goodbye" }));
    let t = p.status_json()["said"][0]["for"].as_u64().expect("the prompt's t");
    p.claim(&["intent_submitted", "--for", &t.to_string(), "--text", "greet with hello"]);
    p.claim(&["one_reading", "--why", "the ask names what must change and how"]);
    p.claim(&["behavior", "--behavior_id", "b1", "--text", "the greeting"]);
    p.claim(&["promise", "--promise_id", "p1", "--behavior_id", "b1", "--accepts", "a name", "--gives_back", "a greeting", "--refuses", "nothing", "--cases", r#"[{"case_id":"n1","kind":"normal","given":"a name","expect":"a greeting"},{"case_id":"o1","kind":"opposite","given":"no name","expect":"no greeting"}]"#]);
    p.claim(&["criterion", "--criterion_id", "c1", "--text", "greet says hello", "--path", "full", "--behavior_id", "b1", "--promise_id", "p1"]);
    p.cases_kept("c1"); // the cases card passed (plan cases-card, 2026-10-25) [draft names]
    p.claim(&["criterion", "--criterion_id", "c2", "--text", "bye says goodbye", "--path", "full", "--behavior_id", "b1", "--promise_id", "p1"]);
    p.cases_kept("c2"); // the cases card passed (plan cases-card, 2026-10-25) [draft names]
    for c in ["c1", "c2"] {
        p.claim(&["place_chosen", "--criterion_id", c, "--places", r#"[{"file_path":"src/app.sh","symbol":"greet","line_start":1,"line_end":3}]"#]);
        p.claim(&["test_planned", "--criterion_id", c, "--file_path", &format!("tests/test_{c}.sh"), "--will", "create"]);
        p.claim(&["criterion_assigned", "--criterion_id", c]);
        p.claim(&["fix_location_category", "--criterion_id", c, "--category", "this_repository", "--where", ""]);
    }
    p.claim(&["test_stack_detected", "--project_root", ".", "--test_stack", "sh", "--test_command", "sh tests/test_c1.sh"]);
    p.claim(&["criterion_head", "--criterion_id", "c1"]);
    let out = p.claim(&["criterion_strung_on_head", "--criterion_id", "c2", "--head", "c1"]);
    assert!(out.contains("familiar-ai: the plan is confirmed"), "{out}");
    p.write("tests/test_c1.sh", "sh src/app.sh | grep -qx hello\n");
    p.commit("c1: tests first");
    p.write("src/app.sh", "#!/bin/sh\ngreet() { echo hello; }\ngreet\n");
    p.commit("c1: greet says hello");
    assert_eq!(p.state_name("c1"), "green");
    let out = p.tool("PreToolUse", "Edit", json!({ "file_path": p.path("tests/test_c1.sh").to_string_lossy() }));
    let judge = "  judge: off: the person chose no judge, so the seal stands on its red run alone";
    assert_eq!(reason(&out), [HOLD, "  asked for: make greet say hello instead of hi, and bye say goodbye", READING, judge, CLAIM].join("\n"));
}
