//! The judge is asked only about the criterion's own part of the prompt, so on
//! a plan with several criteria a sibling's path is not reported as a gap.

mod common;

use common::Project;
use serde_json::{json, Value};

const SESSION: &str = "84be6339-0cc7-40ec-91b9-cae44365931b";

const YES: &str = r#"{"type":"result","is_error":false,"result":"{\"answers_intent\": true, \"gap\": \"\", \"reason\": \"it checks that greet says hello.\"}","modelUsage":{"claude-opus-4-6":{}}}"#;

/// A set-up project planned to c1 (anchor src/app.sh, test
/// tests/test_c1.sh), its holder on haiku, the fake `claude` answering
/// `answer`, c2 strung after it (anchor src/bye.sh).
fn project(name: &str, answer: &str) -> Project {
    let b = Project::builder(name).session(SESSION).file("src/bye.sh", "#!/bin/sh\nbye() { echo bye; }\nbye\n").env_remove("FAMILIAR_AI_JUDGE");
    let d = b.dir().join("bin");
    let fake = format!("#!/bin/sh\nprintf '%s\\n' \"$*\" >> '{d}/calls'\nn=$(ls '{d}'/prompt_* 2>/dev/null | wc -l); cat > \"{d}/prompt_$n\"\ncat '{d}/answer'\n", d = d.display());
    let b = b.bin("claude", &fake);
    std::fs::write(d.join("answer"), answer).expect("answer");
    let p = b.create();
    p.claim(&["intent_submitted", "--for", "0", "--text", "greet with hello"]);
    p.claim(&["behavior", "--behavior_id", "b1", "--text", "the greeting"]);
    p.claim(&["promise", "--promise_id", "p1", "--behavior_id", "b1", "--accepts", "a name", "--gives_back", "a greeting", "--refuses", "nothing", "--cases", r#"[{"case_id":"n1","kind":"normal","given":"a name","expect":"a greeting"},{"case_id":"o1","kind":"opposite","given":"no name","expect":"no greeting"}]"#]);
    p.claim(&["criterion", "--criterion_id", "c1", "--text", "greet says hello", "--path", "full", "--behavior_id", "b1", "--promise_id", "p1"]);
    p.cases_kept("c1"); // the cases card passed (plan cases-card, 2026-10-25) [draft names]
    p.claim(&["place_chosen", "--criterion_id", "c1", "--places", r#"[{"file_path":"src/app.sh","symbol":"greet","line_start":1,"line_end":3}]"#]);
    p.claim(&["test_planned", "--criterion_id", "c1", "--file_path", "tests/test_c1.sh", "--will", "create"]);
    {
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
    {
        p.claim(&["criterion_strung_on_head", "--criterion_id", "c2", "--head", "c1"]);
        p.claim(&["criterion_assigned", "--criterion_id", "c2"]);
        p.claim(&["fix_location_category", "--criterion_id", "c2", "--category", "this_repository", "--where", ""]);
    }
    p.hook("PostModelSwitch", json!({ "from_model": "claude-opus-4-1", "to_model": "claude-haiku-4-5-20251001" }));
    p
}

impl Project {
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

    /// The prompts the fake judge received on stdin, in call order.
    fn prompts(&self) -> Vec<String> {
        (0..).map_while(|n| std::fs::read_to_string(self.dir.join(format!("bin/prompt_{n}"))).ok()).collect()
    }
}

/// What the question says when it is about criterion `c`'s own part.
fn asked_about(prompt: &str, c: &str) -> Result<(), String> {
    let lines = [
        format!("Criterion {c} is the part of the person's words this test answers; its sibling criteria answer the rest."),
        format!("list every path the person's words cover that criterion {c} claims"),
        "A path of the person's words that belongs to a sibling criterion is not a gap.".to_string(),
        format!("in the person's words that criterion {c} claims"),
    ];
    match lines.iter().find(|l| !prompt.contains(l.as_str())) {
        Some(missing) => Err(format!("the question lacks {missing:?}\n{prompt}")),
        None => Ok(()),
    }
}

/// Two criteria, both sealed (each judged yes), the fake judge keeping every prompt.
fn sealed_pair(name: &str) -> Project {
    let p = project(name, YES);
    p.tests_commit("c1", "grep -q hello src/app.sh\n");
    p.code_commit();
    p.tests_commit("c2", "grep -q farewell src/bye.sh\n");
    p
}

#[test]
fn the_prompt_for_a_criterion_names_its_own_part_and_says_a_siblings_path_is_not_a_gap() {
    let p = sealed_pair("prompt");
    let (code, c2, err) = p.run(&["judge-prompt", "c2"], "");
    assert_eq!((code, err.as_str()), (0, ""));
    asked_about(&c2, "c2").unwrap_or_else(|e| panic!("{e}"));
    assert!(!c2.contains("criterion c1 claims") && !c2.contains("Criterion c1 is the part"), "{c2}");
    let (code, c1, err) = p.run(&["judge-prompt", "c1"], "");
    assert_eq!((code, err.as_str()), (0, ""));
    asked_about(&c1, "c1").unwrap_or_else(|e| panic!("{e}"));
    assert!(!c1.contains("criterion c2 claims") && !c1.contains("Criterion c2 is the part"), "{c1}");
}

#[test]
fn the_judge_receives_the_question_about_the_criterion_it_is_asked_of() {
    let p = sealed_pair("stdin");
    let sent = p.prompts();
    assert_eq!(sent.len(), 2, "one call per seal");
    asked_about(&sent[0], "c1").unwrap_or_else(|e| panic!("{e}"));
    asked_about(&sent[1], "c2").unwrap_or_else(|e| panic!("{e}"));
    assert!(!sent[1].contains("criterion c1 claims"), "{}", sent[1]);
    let (_, printed, _) = p.run(&["judge-prompt", "c2"], "");
    assert_eq!(printed, sent[1], "judge-prompt prints the very prompt the judge was sent");
}
