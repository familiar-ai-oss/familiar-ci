//! `familiar-ai status --json` carries each part's journey, so the pane draws
//! the seven steps without deriving any of them.
//!
//! ```text
//!   plan ── place ── test ── red ── fix ── green ── CI
//!   done: true ●   false ○ (not done; every step of a part in play)
//!         null · (not reached: past the next step of a part not in play)
//!         "half" ◐ (CI: locally verified, not hosted)
//!   at:   the short sha the step was done at (red, fix, green)
//! ```

mod common;

use common::Project;
use serde_json::{json, Value};

const SESSION: &str = "5a7e1c93-0b2d-4e6f-8a14-9c3d2b1e0f57";

fn project(name: &str) -> Project {
    Project::builder(name)
        .session(SESSION)
        .no_app()
        .file("src/a.sh", "#!/bin/sh\ngreet() { echo hi; }\ngreet\n")
        .file("src/b.sh", "#!/bin/sh\nbye() { echo bye; }\nbye\n")
        .create()
}

impl Project {
    /// The intent; c1 heads the string on src/a.sh greet, c2 is strung on it
    /// on src/b.sh bye.
    fn planned(&self) {
        self.hook("UserPromptSubmit", json!({ "prompt": "a says hello" }));
        let path = self.path("src/a.sh").to_string_lossy().into_owned();
        let out = self.hook("PreToolUse", json!({ "tool_name": "Edit", "tool_input": { "file_path": path, "old_string": "echo hi", "new_string": "echo hello" } }));
        let reason = out["hookSpecificOutput"]["permissionDecisionReason"].as_str().unwrap_or("").to_string();
        let t = reason.split("intent_submitted --for ").nth(1).and_then(|r| r.split_whitespace().next()).unwrap_or_else(|| panic!("a claim line: {reason}")).to_string();
        self.claim(&["intent_submitted", "--for", &t, "--text", "a says hello"]);
        // definition change (the owner's plan of 2026-10-21, step 2) [draft]: a short ask with no example
        // fires a trigger; the agent says it reads one way before the tests
        self.claim(&["one_reading", "--why", "the ask names what must change and how"]);
        self.claim(&["behavior", "--behavior_id", "b1", "--text", "the greeting"]);
        self.claim(&["promise", "--promise_id", "p1", "--behavior_id", "b1", "--accepts", "a name", "--gives_back", "a greeting", "--refuses", "nothing", "--cases", r#"[{"case_id":"n1","kind":"normal","given":"a name","expect":"a greeting"},{"case_id":"o1","kind":"opposite","given":"no name","expect":"no greeting"}]"#]);
        for (c, text, file, symbol) in [("c1", "a says hello", "src/a.sh", "greet"), ("c2", "b says goodbye", "src/b.sh", "bye")] {
            let test = format!("tests/test_{c}.sh");
            self.claim(&["criterion", "--criterion_id", c, "--text", text, "--path", "full", "--behavior_id", "b1", "--promise_id", "p1"]);
            self.cases_kept(c); // the cases card passed (plan cases-card, 2026-10-25) [draft names]
            self.claim(&["place_chosen", "--criterion_id", c, "--places", &format!(r#"[{{"file_path":"{file}","symbol":"{symbol}","line_start":1,"line_end":3}}]"#)]);
            self.claim(&["test_planned", "--criterion_id", c, "--file_path", &test, "--will", "create", "--test_command", &format!("sh {test}")]);
            if c == "c1" {
                self.claim(&["test_stack_detected", "--project_root", ".", "--test_stack", "sh", "--test_command", "sh tests/test_c1.sh"]);
                self.claim(&["criterion_head", "--criterion_id", c]);
            } else {
                self.claim(&["criterion_strung_on_head", "--criterion_id", c, "--head", "c1"]);
            }
            self.claim(&["criterion_assigned", "--criterion_id", c]);
            self.claim(&["fix_location_category", "--criterion_id", c, "--category", "this_repository", "--where", ""]);
        }
    }
}

fn criterion<'a>(j: &'a Value, c: &str) -> &'a Value {
    j["criteria"].as_array().and_then(|a| a.iter().find(|x| x["id"] == c)).unwrap_or_else(|| panic!("{c} in {j}"))
}

/// The seven steps in order: each `done`, and `at` where given.
fn steps(done: [Value; 7], at: [Option<&str>; 7]) -> Value {
    let names = ["plan", "place", "test", "red", "fix", "green", "CI"];
    Value::from(
        names
            .iter()
            .zip(done)
            .zip(at)
            .map(|((n, d), a)| {
                let mut s = json!({ "name": n, "done": d });
                if let Some(a) = a {
                    s["at"] = Value::from(a);
                }
                s
            })
            .collect::<Vec<_>>(),
    )
}

const NO_AT: [Option<&str>; 7] = [None; 7];

/// c1 walks plan → red → green while c2, strung on it, waits; then c2's turn
/// comes. Every other field of a criterion stays as it was.
#[test]
fn each_part_carries_its_journey_from_plan_to_green() {
    let p = project("journey");
    p.planned();

    // planned: c1 in play, every step after place not done; c2 waiting, past
    // its next step not reached
    let j = p.status_json();
    let (c1, c2) = (criterion(&j, "c1"), criterion(&j, "c2"));
    assert_eq!(c1["state_name"], "testing", "{j}");
    assert_eq!(c2["state_name"], "waiting", "{j}");
    let (t, f, n) = (json!(true), json!(false), Value::Null);
    assert_eq!(
        c1["journey"],
        json!({
            "steps": steps([t.clone(), t.clone(), f.clone(), f.clone(), f.clone(), f.clone(), f.clone()], NO_AT),
            "tests": ["tests/test_c1.sh"],
            "places": [{ "file": "src/a.sh", "symbol": "greet" }],
            "opened": true,
        }),
        "{j}"
    );
    assert_eq!(
        c2["journey"],
        json!({
            "steps": steps([t.clone(), t.clone(), f.clone(), n.clone(), n.clone(), n.clone(), n.clone()], NO_AT),
            "tests": ["tests/test_c2.sh"],
            "places": [{ "file": "src/b.sh", "symbol": "bye" }],
            "opened": false,
        }),
        "{j}"
    );
    // the fields beside it stay
    for k in ["id", "name", "part", "ask", "text", "state", "state_name", "holds", "holder", "held_by", "path", "judge", "after"] {
        assert!(c1.get(k).is_some(), "{k} stays: {j}");
    }

    // red: the test written and sealed failing at its commit
    p.write("tests/test_c1.sh", "sh src/a.sh | grep -qx hello\n");
    p.commit("c1: tests first");
    let seal7 = p.git(&["rev-parse", "--short=7", "HEAD"]);
    let j = p.status_json();
    let c1 = criterion(&j, "c1");
    assert_eq!(c1["state_name"], "sealed", "{j}");
    assert_eq!(c1["journey"]["steps"], steps([t.clone(), t.clone(), t.clone(), t.clone(), f.clone(), f.clone(), f.clone()], [None, None, None, Some(&seal7), None, None, None]), "{j}");
    assert_eq!(c1["journey"]["opened"], true, "{j}");
    assert_eq!(criterion(&j, "c2")["journey"]["steps"][3]["done"], Value::Null, "{j}");

    // green: the fix landed on the place and the test passes there
    p.write("src/a.sh", "#!/bin/sh\ngreet() { echo hello; }\ngreet\n");
    p.commit("c1: the fix");
    let head7 = p.git(&["rev-parse", "--short=7", "HEAD"]);
    let j = p.status_json();
    let (c1, c2) = (criterion(&j, "c1"), criterion(&j, "c2"));
    assert_eq!(c1["state_name"], "green", "{j}");
    assert_eq!(
        c1["journey"],
        json!({
            "steps": steps([t.clone(), t.clone(), t.clone(), t.clone(), t.clone(), t.clone(), f.clone()], [None, None, None, Some(&seal7), Some(&head7), Some(&head7), None]),
            "tests": ["tests/test_c1.sh"],
            "places": [{ "file": "src/a.sh", "symbol": "greet" }],
            "opened": true,
        }),
        "{j}"
    );
    // c2's turn has come: in play, so nothing of it is "not reached"
    assert_eq!(c2["state_name"], "testing", "{j}");
    assert_eq!(c2["journey"]["steps"], steps([t.clone(), t, f.clone(), f.clone(), f.clone(), f.clone(), f], NO_AT), "{j}");
    assert_eq!(c2["journey"]["opened"], true, "{j}");
}
