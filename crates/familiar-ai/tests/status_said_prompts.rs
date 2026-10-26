//! Each prompt the person typed is marked an ask, an answer or a note, derived
//! from facts that already exist; nothing is stored. See
//! docs/decisions.md#asks-answers-and-notes

mod common;

use common::Project;
use serde_json::json;

const SESSION: &str = "c4b2a3f1-7e6d-4a80-9b2a-3d4e5f6a7b8c";
const ASKED: &str = "fix the greeting so it says hello\nand keep the rest as it is";
const QUESTION: &str = "what does greet print right now?";
const STEER: &str = "use the shorter word if you can";

fn project(name: &str) -> Project {
    Project::builder(name).session(SESSION).env("PATH", "/usr/bin:/bin").env("TZ", "UTC").create()
}

impl Project {
    /// The person types `prompt`; its turn.
    fn says(&self, prompt: &str) -> u64 {
        let (code, _, err) = self.hook_raw("UserPromptSubmit", &self.fixture("user_prompt_submit", json!({ "prompt": prompt })));
        assert_eq!(code, 0, "{err}");
        self.facts().iter().rev().find(|f| f.kind == "person_said").map(|f| f.t).expect("the prompt is recorded")
    }

    /// An ask (claimed as an intent, split into one part), a question the
    /// agent answered, and a steer no one claimed anything for.
    fn three(name: &str) -> (Project, [u64; 3]) {
        let p = project(name);
        let ask = p.says(ASKED);
        p.claim(&["intent_submitted", "--for", &ask.to_string(), "--text", "greet says hello"]);
        p.claim(&["criterion", "--criterion_id", "c1", "--text", "greet says hello", "--path", "light"]);
        let question = p.says(QUESTION);
        p.claim(&["person_said_answered", "--for", &question.to_string(), "--how", "question"]);
        let steer = p.says(STEER);
        (p, [ask, question, steer])
    }
}

/// `h:mm`: the hour without a leading zero, two digits of minutes.
fn is_clock(s: &str) -> bool {
    let Some((h, m)) = s.split_once(':') else { return false };
    !h.is_empty() && h.len() <= 2 && !(h.len() == 2 && h.starts_with('0')) && h.chars().all(|c| c.is_ascii_digit()) && m.len() == 2 && m.chars().all(|c| c.is_ascii_digit())
}

/// `status --json` carries `said`: every prompt, oldest first, whole, with
/// its time, its turn and what it became; an ask carries its number, the
/// same ask the criteria name.
#[test]
fn status_json_says_each_prompt_and_what_it_became() {
    let (p, [ask, question, steer]) = Project::three("json");
    let j = p.status_json();
    let said = j["said"].as_array().unwrap_or_else(|| panic!("said is a list: {j}"));
    assert_eq!(said.len(), 3, "{j}");
    for row in said {
        assert!(row["at"].as_str().is_some_and(is_clock), "{row}");
    }
    let at = |i: usize| said[i]["at"].clone();
    assert_eq!(said[0], json!({ "for": ask, "at": at(0), "text": ASKED, "became": "ask", "ask": 1 }), "{j}");
    assert_eq!(said[1], json!({ "for": question, "at": at(1), "text": QUESTION, "became": "answer", "ask": null }), "{j}");
    assert_eq!(said[2], json!({ "for": steer, "at": at(2), "text": STEER, "became": "note", "ask": null }), "{j}");
    // the ask's row is the ask the criterion names, by its turn and its time
    let c1 = &j["criteria"][0]["ask"];
    assert_eq!((c1["for"].clone(), c1["at"].clone()), (json!(ask), at(0)), "{j}");
}

/// Before anything is said, `said` is an empty list.
#[test]
fn nothing_said_is_an_empty_list() {
    let p = project("none");
    assert_eq!(p.status_json()["said"], json!([]));
}

/// `status --said` prints the same list after the picture, one line per
/// prompt; the plain `status` does not.
#[test]
fn status_said_prints_one_line_per_prompt() {
    let (p, _) = Project::three("text");
    let j = p.status_json();
    let at = |i: usize| j["said"][i]["at"].as_str().unwrap_or_default().to_string();
    let (code, out, err) = p.run(&["status", "--said"], "");
    assert_eq!(code, 0, "{err}");
    let (plain_code, plain, _) = p.run(&["status"], "");
    assert_eq!(plain_code, 0);
    assert!(out.starts_with(plain.trim_end()), "the picture first:\n{out}\n---\n{plain}");
    let lines: Vec<&str> = out.lines().rev().take(3).collect::<Vec<_>>().into_iter().rev().collect();
    assert_eq!(
        lines,
        [
            format!("  {}  fix the greeting so it says hello and keep the rest as it is  · ask 1", at(0)),
            format!("  {}  {QUESTION}  · answer", at(1)),
            format!("  {}  {STEER}  · note", at(2)),
        ],
        "{out}"
    );
    assert!(!plain.contains("· note") && !plain.contains("· answer"), "the plain status stays as it is: {plain}");
}
