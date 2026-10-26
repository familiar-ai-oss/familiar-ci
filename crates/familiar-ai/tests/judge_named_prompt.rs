//! The judge reads the prompt the intent names with `--for` and those the
//! criteria name, or with no `--for` the latest prompt before the intent. See
//! docs/decisions.md#the-intent-names-the-prompt

mod common;

use common::Project;
use serde_json::{json, Value};

const SAID: [&str; 3] = ["make the greeter polite", "greet says hello", "bye says farewell"];

const SESSION: &str = "84be6339-0cc7-40ec-91b9-cae44365931b";

const YES: &str = r#"{"type":"result","is_error":false,"result":"{\"answers_intent\": true, \"gap\": \"\", \"reason\": \"it checks that greet says hello.\"}","modelUsage":{"claude-opus-4-6":{}}}"#;

/// A set-up project planned to c1 (anchor src/app.sh, test
/// tests/test_c1.sh), its holder on haiku, the fake `claude` answering
/// `answer`, c2 strung after it (anchor src/bye.sh).
fn project(name: &str, answer: &str, named: bool) -> Project {
    let b = Project::builder(name).session(SESSION).file("src/bye.sh", "#!/bin/sh\nbye() { echo bye; }\nbye\n").env_remove("FAMILIAR_AI_JUDGE");
    let d = b.dir().join("bin");
    let fake = format!("#!/bin/sh\nprintf '%s\\n' \"$*\" >> '{d}/calls'\nn=$(ls '{d}'/prompt_* 2>/dev/null | wc -l); cat > \"{d}/prompt_$n\"\ncat '{d}/answer'\n", d = d.display());
    let b = b.bin("claude", &fake);
    std::fs::write(d.join("answer"), answer).expect("answer");
    let p = b.create();
    for said in SAID {
        p.hook("UserPromptSubmit", json!({ "prompt": said }));
    }
    let ts = p.said_ts();
    assert_eq!(ts.len(), 3, "three person_said: {ts:?}");
    let (second, third) = (ts[1].to_string(), ts[2].to_string());
    if named {
        p.claim(&["intent_submitted", "--for", &second, "--text", "greet with hello"]);
        p.claim(&["behavior", "--behavior_id", "b1", "--text", "the greeting"]);
        p.claim(&["promise", "--promise_id", "p1", "--behavior_id", "b1", "--accepts", "a name", "--gives_back", "a greeting", "--refuses", "nothing", "--cases", r#"[{"case_id":"n1","kind":"normal","given":"a name","expect":"a greeting"},{"case_id":"o1","kind":"opposite","given":"no name","expect":"no greeting"}]"#]);
        p.claim(&["criterion", "--criterion_id", "c1", "--text", "greet says hello", "--path", "full", "--behavior_id", "b1", "--promise_id", "p1", "--for", &third]);
        p.cases_kept("c1"); // the cases card passed (plan cases-card, 2026-10-25) [draft names]
    } else {
        p.claim(&["intent_submitted", "--text", "greet with hello"]);
        p.claim(&["behavior", "--behavior_id", "b1", "--text", "the greeting"]);
        p.claim(&["promise", "--promise_id", "p1", "--behavior_id", "b1", "--accepts", "a name", "--gives_back", "a greeting", "--refuses", "nothing", "--cases", r#"[{"case_id":"n1","kind":"normal","given":"a name","expect":"a greeting"},{"case_id":"o1","kind":"opposite","given":"no name","expect":"no greeting"}]"#]);
        p.claim(&["criterion", "--criterion_id", "c1", "--text", "greet says hello", "--path", "full", "--behavior_id", "b1", "--promise_id", "p1"]);
        p.cases_kept("c1"); // the cases card passed (plan cases-card, 2026-10-25) [draft names]
    }
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
    /// The `t` of each `person_said`, in order.
    fn said_ts(&self) -> Vec<u64> {
        self.facts().iter().filter(|f| f.kind == "person_said").map(|f| f.t).collect()
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
}

/// Two criteria, both sealed (each judged yes).
fn sealed_pair(name: &str, named: bool) -> Project {
    let p = project(name, YES, named);
    p.tests_commit("c1", "grep -q hello src/app.sh\n");
    p.code_commit();
    p.tests_commit("c2", "grep -q farewell src/bye.sh\n");
    p
}

fn prompt_of(p: &Project, c: &str) -> String {
    let (code, out, err) = p.run(&["judge-prompt", c], "");
    assert_eq!((code, err.as_str()), (0, ""));
    out
}

#[test]
fn an_intent_with_for_gives_the_judge_that_prompt_and_the_ones_the_criteria_name_not_every_prompt() {
    let p = sealed_pair("named", true);
    for c in ["c1", "c2"] {
        let prompt = prompt_of(&p, c);
        assert!(prompt.contains(SAID[1]) && prompt.contains(SAID[2]), "{c}: {prompt}");
        assert!(!prompt.contains(SAID[0]), "{c}: the first prompt is named by nothing: {prompt}");
        assert!(prompt.contains(&format!("{}\n\nLater, the person also said:\n{}", SAID[1], SAID[2])), "{c}: in order: {prompt}");
    }
}

#[test]
fn an_intent_with_no_for_gives_the_judge_the_latest_prompt_only() {
    let p = sealed_pair("unnamed", false);
    for c in ["c1", "c2"] {
        let prompt = prompt_of(&p, c);
        // the second prompt's words are also c1's criterion: the intent's own section is read
        assert!(prompt.contains(&format!("in their own words)\n{}\n\nTHE AGENT'S READING", SAID[2])), "{c}: the third alone is the intent: {prompt}");
        assert!(!prompt.contains(SAID[0]), "{c}: the first prompt is not the ask: {prompt}");
    }
}
