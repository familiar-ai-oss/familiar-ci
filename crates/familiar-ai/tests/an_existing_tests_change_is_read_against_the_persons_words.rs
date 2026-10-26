//! An announced change to an existing test is read by the judge against the person's words (the
//! owner, 2026-10-20; the bench's astroid-3152 and requests-7315) [draft names]. The question is
//! not coverage: does the person's ask call for this test's old expectation to change? Its word is
//! `existing_test_verdict`; a "no" holds the part again, naming the judge's gap with what the
//! person asked for, and the agent clears it by putting the lines back or by announcing again;
//! never a card. Runs the real binary with a fake `claude` first on PATH; no model is called.

mod common;

use common::Project;
use serde_json::{json, Value};

const SESSION: &str = "84be6339-0cc7-40ec-91b9-cae44365931b";
const OLD: &str = "tests/test_old.py";
const OLD_AT_BASE: &str = "import os\n\n\ndef test_init_defaults():\n    assert os.sep == \"/\"\n";

fn said(yes: bool, gap: &str, reason: &str) -> String {
    let answer = json!({ "answers_intent": yes, "gap": gap, "reason": reason }).to_string();
    json!({ "type": "result", "is_error": false, "result": answer, "modelUsage": { "claude-opus-4-6": {} } }).to_string()
}

fn project(name: &str) -> Project {
    let b = Project::builder(name).session(SESSION).file(OLD, OLD_AT_BASE);
    let d = b.dir().join("bin");
    let fake = format!("#!/bin/sh\nd='{d}'\nprintf '%s\\n' \"$*\" >> \"$d/calls\"\ncat > \"$d/prompt\"\ncat \"$d/answer\"\n", d = d.display());
    let b = b.bin("claude", &fake);
    std::fs::write(d.join("answer"), said(true, "", "it runs greet and checks it says hello.")).expect("answer");
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
    fn judge_says(&self, answer: &str) {
        std::fs::write(self.dir.join("bin/answer"), answer).expect("answer");
    }

    fn judged(&self) -> String {
        std::fs::read_to_string(self.dir.join("bin/prompt")).unwrap_or_default()
    }

    /// Sealed red and covered, then green, then the old test's expectation changed: held.
    fn changed(&self) {
        self.write("tests/test_c1.sh", "sh src/app.sh | grep -qx hello\n");
        self.commit("c1: tests");
        self.write("src/app.sh", "#!/bin/sh\ngreet() { echo hello; }\ngreet\n");
        self.commit("c1: code");
        assert_eq!(self.state_name("c1"), "green", "{}", self.status());
        self.write(OLD, &OLD_AT_BASE.replace("\"/\"", "os.sep"));
        self.commit("the old test agrees with the code");
        assert_eq!(self.state_name("c1"), "held", "{}", self.status());
        self.claim(&["existing_test_announced", "--file_path", OLD, "--reason", "the test now matches what the code does"]);
    }

    fn verdicts(&self) -> Vec<Value> {
        self.facts_of("existing_test_verdict")
    }
}

#[test]
fn the_judge_asks_whether_the_persons_ask_calls_for_the_old_expectation_to_change() {
    let p = project("existing-judge-yes");
    p.changed();
    let stop = p.hook("Stop", json!({}));
    let v = p.verdicts();
    assert_eq!(v.len(), 1, "{stop}");
    assert_eq!((v[0]["file_path"].as_str(), v[0]["covers"].as_str()), (Some(OLD), Some("yes")));
    let prompt = p.judged();
    for part in [
        "THE INTENT (what the person asked for, in their own words)\ngreet with hello",
        "THE AGENT'S READING (what the agent wrote down; not the person's words)",
        "criterion c1: greet says hello",
        "THE EXISTING TEST tests/test_old.py (changed: test_init_defaults, line 5)\nAs it stood before the agent's work:\ndef test_init_defaults():\n    assert os.sep == \"/\"\n\nAs it stands now:\ndef test_init_defaults():\n    assert os.sep == os.sep",
        "THE AGENT'S REASON (the agent's words, not the person's)\nthe test now matches what the code does",
        familiar_ai::teach::existing::QUESTION,
    ] {
        assert!(prompt.contains(part), "{part:?} in {prompt}");
    }
    assert_eq!(p.state_name("c1"), "green", "{}", p.status());
}

#[test]
fn a_no_holds_again_with_the_persons_words_and_the_judges_gap_and_never_is_a_card() {
    let p = project("existing-judge-no");
    p.changed();
    p.judge_says(&said(false, "the separator the old test expected", "the ask is about the greeting; nothing in it asks for the separator to change."));
    let stop = p.hook("Stop", json!({}));
    assert_eq!(p.verdicts().last().and_then(|v| v["covers"].as_str()).map(str::to_string), Some("no".to_string()), "{stop}");
    assert_eq!(p.state_name("c1"), "held");
    let stop = p.hook("Stop", json!({}));
    assert_eq!(stop["decision"], "block", "{stop}");
    let r = stop["reason"].as_str().unwrap_or("");
    assert!(r.contains("the separator the old test expected"), "{r}");
    assert!(r.contains("asked for: greet with hello"), "{r}");
    assert!(!r.contains("AskUserQuestion"), "the agent's to clear, not the person's: {r}");
    // the lines put back clear it
    p.write(OLD, OLD_AT_BASE);
    p.commit("put the old test back");
    assert_eq!(p.state_name("c1"), "green", "{}", p.status());
}
