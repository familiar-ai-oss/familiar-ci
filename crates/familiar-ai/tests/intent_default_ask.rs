//! An intent claimed without `--for` is for the person's latest prompt before
//! the claim, so the PR body shows the ask typed just before; with no prompt
//! yet the claim is refused and nothing is appended.

mod common;

use common::Project;
use serde_json::{json, Value};

const SAID: [&str; 3] = ["what does greet print", "is there a test folder", "make greet say hello"];

const SESSION: &str = "84be6339-0cc7-40ec-91b9-cae44365931b";

/// A set-up project with one commit; the person has said nothing yet.
fn project(name: &str) -> Project {
    Project::builder(name).session(SESSION).create()
}

impl Project {
    /// The project with the person's three prompts said, and their turns.
    fn said(name: &str) -> (Project, Vec<u64>) {
        let p = project(name);
        for said in SAID {
            p.hook("UserPromptSubmit", json!({ "prompt": said }));
        }
        let ts = p.facts().iter().filter(|f| f.kind == "person_said").map(|f| f.t).collect::<Vec<_>>();
        assert_eq!(ts.len(), 3, "three person_said: {ts:?}");
        (p, ts)
    }

    /// The intent, then a criterion under it, so the status has an ask.
    fn plan(&self, intent: &[&str]) {
        let (code, out, err) = self.try_claim(intent);
        assert_eq!(code, 0, "claim {intent:?}: {out}{err}");
        let (code, out, err) = self.try_claim(&["criterion", "--criterion_id", "c1", "--text", "greet says hello", "--path", "light"]);
        assert_eq!(code, 0, "criterion: {out}{err}");
    }

    /// The intent's `for` as recorded.
    fn intent_for(&self) -> Option<u64> {
        self.facts().iter().find(|f| f.kind == "intent_submitted").and_then(|f| f.fields.get("for")).and_then(Value::as_u64)
    }

    /// `status --json`'s ask of c1: its turn and its words.
    fn ask(&self) -> (Option<u64>, String) {
        let (code, out, err) = self.run(&["status", "--json"], "");
        assert_eq!(code, 0, "{err}");
        let v: Value = serde_json::from_str(&out).expect("one JSON object");
        let c1 = v["criteria"].as_array().and_then(|cs| cs.iter().find(|c| c["id"] == "c1" || c["criterion_id"] == "c1")).cloned().unwrap_or_else(|| panic!("c1 in {out}"));
        (c1["ask"]["for"].as_u64(), c1["ask"]["text"].as_str().unwrap_or("").to_string())
    }

    /// `status --said`'s line for each prompt, in order.
    fn said_lines(&self) -> Vec<String> {
        let (code, out, err) = self.run(&["status", "--said"], "");
        assert_eq!(code, 0, "{err}");
        SAID.iter().map(|s| out.lines().find(|l| l.contains(s)).unwrap_or_else(|| panic!("{s} in {out}")).to_string()).collect()
    }
}

#[test]
fn an_intent_without_for_is_for_the_latest_prompt() {
    let (p, ts) = Project::said("latest");
    p.plan(&["intent_submitted", "--text", "greet with hello"]);
    assert_eq!(p.intent_for(), Some(ts[2]), "the intent is for the third prompt: {:?}", p.facts());
    assert_eq!(p.ask(), (Some(ts[2]), SAID[2].to_string()));
    let lines = p.said_lines();
    assert!(lines[2].ends_with("· ask 1"), "{lines:?}");
    assert!(lines[0].ends_with("· note") && lines[1].ends_with("· note"), "{lines:?}");
}

#[test]
fn an_intent_with_for_is_for_the_prompt_it_names() {
    let (p, ts) = Project::said("named");
    let first = ts[0].to_string();
    p.plan(&["intent_submitted", "--for", &first, "--text", "greet with hello"]);
    assert_eq!(p.intent_for(), Some(ts[0]));
    assert_eq!(p.ask(), (Some(ts[0]), SAID[0].to_string()));
    let lines = p.said_lines();
    assert!(lines[0].ends_with("· ask 1"), "{lines:?}");
}

#[test]
fn an_intent_without_for_and_no_prompt_is_refused() {
    let p = project("none");
    let (code, out, err) = p.try_claim(&["intent_submitted", "--text", "greet with hello"]);
    assert_eq!(code, 1, "{out}{err}");
    assert!(err.contains("no prompt to tie the intent to; the person has said nothing yet"), "{err}");
    assert!(err.contains("familiar-ai claim intent_submitted"), "the kind's form follows: {err}");
    assert!(!p.facts().iter().any(|f| f.kind == "intent_submitted"), "nothing appended: {:?}", p.facts());
}
