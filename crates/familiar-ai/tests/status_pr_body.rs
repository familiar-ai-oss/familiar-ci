//! `status --pr` prints the whole pull request body: the logo, one sentence,
//! each part with its ask, its tests and the judge, the CI line and the
//! closing line that says where the proof comes from. Parts are named by
//! their own text; ids never reach the page.

mod common;

use common::Project;
use serde_json::{json, Value};

const SESSION: &str = "5d1e7a3c-2b4f-4c69-9e18-7a6b5c4d3e2f";
const ASKED: &str = "fix the greeting so it says hello, and make bye say goodbye";

const YES: &str = r#"{"type":"result","is_error":false,"result":"{\"answers_intent\": true, \"gap\": \"\", \"reason\": \"it checks that greet says hello.\"}","modelUsage":{"claude-opus-4-6":{}}}"#;

const LOGO: &str = r#"<p align="center"><img src="https://raw.githubusercontent.com/familiar-ai-oss/familiar-ci/main/docs/logo-on-dark.png" width="180" alt="Familiar"></p>"#;
const CI: &str = "CI (familiar-verify) proves each part: red at its tests-first commit, green at the head.";
const CLOSING: &str = "Proven by [Familiar](https://github.com/familiar-ai-oss/familiar-ci#the-proof), tests that failed first, then passed, verified by CI in a fresh clone.";

/// A set-up project whose fake `claude` answers yes to every judge call; the
/// person asks `ASKED` and the agent claims its intent for that prompt.
fn project(name: &str) -> Project {
    let b = Project::builder(name).session(SESSION).file("src/bye.sh", "#!/bin/sh\nbye() { echo bye; }\nbye\n").env_remove("FAMILIAR_AI_JUDGE");
    let d = b.dir().join("bin");
    let b = b.bin("claude", &format!("#!/bin/sh\ncat > /dev/null\ncat '{}/answer'\n", d.display()));
    std::fs::write(d.join("answer"), YES).expect("answer");
    let p = b.create();
    p.hook_with("UserPromptSubmit", &p.fixture("user_prompt_submit", json!({ "prompt": ASKED })));
    let said = p.facts().iter().rev().find(|f| f.kind == "person_said").map(|f| f.t).expect("the prompt is recorded");
    p.claim(&["intent_submitted", "--for", &said.to_string(), "--text", "greet says hello; bye says goodbye"]);
    p
}

impl Project {
    /// Plan `c` with its text, anchor file and test file, run by its own command.
    fn part(&self, c: &str, text: &str, file: &str, symbol: &str, test: &str) {
        self.claim(&["behavior", "--behavior_id", "b1", "--text", "the greeting"]);
        self.claim(&["promise", "--promise_id", "p1", "--behavior_id", "b1", "--accepts", "a name", "--gives_back", "a greeting", "--refuses", "nothing", "--cases", r#"[{"case_id":"n1","kind":"normal","given":"a name","expect":"a greeting"},{"case_id":"o1","kind":"opposite","given":"no name","expect":"no greeting"}]"#]);
        self.claim(&["criterion", "--criterion_id", c, "--text", text, "--path", "full", "--behavior_id", "b1", "--promise_id", "p1"]);
        self.cases_kept(c); // the cases card passed (plan cases-card, 2026-10-25) [draft names]
        let places = format!(r#"[{{"file_path":"{file}","symbol":"{symbol}","line_start":1,"line_end":3}}]"#);
        self.claim(&["place_chosen", "--criterion_id", c, "--places", &places]);
        self.claim(&["test_planned", "--criterion_id", c, "--file_path", test, "--will", "create", "--test_command", &format!("sh {test}")]);
    }

    /// Commit `files`, then hand the hook the commit's PostToolUse.
    fn commit_files(&self, message: &str, files: &[(&str, &str)]) -> Value {
        for (path, text) in files {
            self.write(path, text);
        }
        self.git(&["add", "-A"]);
        self.git(&["commit", "-q", "-m", message]);
        self.hook("PostToolUse", json!({ "tool_name": "Bash", "tool_input": { "command": "git commit -m work" }, "tool_response": {} }))
    }

    /// What `status --pr` prints, which must succeed with nothing on stderr.
    fn pr_body(&self) -> String {
        let (code, out, err) = self.run(&["status", "--pr"], "");
        assert_eq!((code, err.as_str()), (0, ""), "{out}");
        out
    }
}

/// The two parts of `ASKED`, both green; the first is sealed with the judge
/// on and answers yes, the second is sealed with the judge off.
fn two_parts(name: &str) -> Project {
    let p = project(name);
    p.part("c1", "greet says hello", "src/app.sh", "greet", "tests/test_greet.sh");
    p.part("c2", "bye says goodbye", "src/bye.sh", "bye", "tests/test_bye.sh");
    p.claim(&["test_stack_detected", "--project_root", ".", "--test_stack", "sh", "--test_command", "for f in tests/test_*.sh; do sh \"$f\" || exit 1; done"]);
    p.claim(&["criterion_head", "--criterion_id", "c1"]);
    p.claim(&["criterion_strung_on_head", "--criterion_id", "c2", "--head", "c1"]);
    for c in ["c1", "c2"] {
        p.claim(&["criterion_assigned", "--criterion_id", c]);
        p.claim(&["fix_location_category", "--criterion_id", c, "--category", "this_repository", "--where", ""]);
    }
    p.hook("PostModelSwitch", json!({ "from_model": "claude-opus-4-1", "to_model": "claude-haiku-4-5-20251001" }));
    p.commit_files("greet: tests", &[("tests/test_greet.sh", "grep -q hello src/app.sh\n")]);
    p.commit_files("greet: code", &[("src/app.sh", "#!/bin/sh\ngreet() { echo hello; }\ngreet\n")]);
    p.set_env("FAMILIAR_AI_JUDGE", Some("off"));
    p.commit_files("bye: tests", &[("tests/test_bye.sh", "grep -q goodbye src/bye.sh\n")]);
    p.commit_files("bye: code", &[("src/bye.sh", "#!/bin/sh\nbye() { echo goodbye; }\nbye\n")]);
    assert_eq!((p.state_name("c1"), p.state_name("c2")), ("green".to_string(), "green".to_string()), "{}", p.status());
    p
}

#[test]
fn two_parts_one_judged_the_body_is_the_page_whole() {
    let p = two_parts("two");
    let body = p.pr_body();
    let expected = format!(
        "{LOGO}

2 parts, each tested first and fixed on its anchor.

**greet says hello**

- asked for: {ASKED}
- tested by: tests/test_greet.sh
- judge: it covers the request (opus)

**bye says goodbye**

- asked for: {ASKED}
- tested by: tests/test_bye.sh
- judge: not asked

{CI}

{CLOSING}
"
    );
    assert_eq!(body, expected);
    assert!(!body.contains("c1") && !body.contains("c2"), "{body}");
}

#[test]
fn one_part_the_sentence_says_part() {
    let p = project("one");
    p.part("c1", "greet says hello", "src/app.sh", "greet", "tests/test_greet.sh");
    p.claim(&["test_stack_detected", "--project_root", ".", "--test_stack", "sh", "--test_command", "sh tests/test_greet.sh"]);
    p.claim(&["criterion_head", "--criterion_id", "c1"]);
    p.claim(&["criterion_assigned", "--criterion_id", "c1"]);
    p.claim(&["fix_location_category", "--criterion_id", "c1", "--category", "this_repository", "--where", ""]);
    p.set_env("FAMILIAR_AI_JUDGE", Some("off"));
    p.commit_files("greet: tests", &[("tests/test_greet.sh", "grep -q hello src/app.sh\n")]);
    p.commit_files("greet: code", &[("src/app.sh", "#!/bin/sh\ngreet() { echo hello; }\ngreet\n")]);
    assert_eq!(p.state_name("c1"), "green", "{}", p.status());
    let expected = format!(
        "{LOGO}

1 part, each tested first and fixed on its anchor.

**greet says hello**

- asked for: {ASKED}
- tested by: tests/test_greet.sh
- judge: not asked

{CI}

{CLOSING}
"
    );
    assert_eq!(p.pr_body(), expected);
}

#[test]
fn outside_a_project_it_refuses_as_status_does() {
    let p = Project::builder("outside").no_setup().create();
    let (code, out, err) = p.run(&["status", "--pr"], "");
    let (s_code, s_out, s_err) = p.run(&["status"], "");
    assert_eq!((code, out, err), (s_code, s_out, s_err));
    assert_ne!(code, 0);
}
