//! Every checkout shows every other: `status --json` lists each other
//! checkout of the repository with its folder, branch and switch, what the
//! person said there, each ask with its parts, their states and who holds
//! each part in words; the status line carries an entry for each checkout
//! with a run open; a busy record is listed as busy. It reads the same
//! whatever the reading checkout's own switch says. The pane draws this same
//! `status --json`; its drawing is tested in plugin/tests.

mod common;

use std::path::{Path, PathBuf};

use common::Project;
use familiar_ai_facts::Record;
use serde_json::{json, Value};

const HOLDER: &str = "6f1d2e3a-4b5c-4d6e-8f70-81a2b3c4d5e6";
const AGENT: &str = "a7f3c2e1b9d04";
const APP: &str = "#!/bin/sh\ngreet() { echo hi; }\nbye() { echo bye; }\n";
const ASKED: &str = "greet with hello and wave goodbye";

fn project(name: &str) -> Project {
    Project::builder(name).no_session().no_app().file("src/app.sh", APP).create()
}

impl Project {
    /// A new worktree beside the repository on branch `name`, set up there.
    fn worktree(&self, name: &str) -> PathBuf {
        let at = self.dir.join(name);
        self.git(&["worktree", "add", "-q", "-b", name, &at.to_string_lossy()]);
        let (code, out, err) = self.run_in(&at, &["setup"], "");
        assert_eq!(code, 0, "setup in the worktree: {out}{err}");
        at
    }

    fn hook_at(&self, at: &Path, event: &str, fields: Value) -> Value {
        let mut payload = json!({ "hook_event_name": event, "session_id": HOLDER, "cwd": at.to_string_lossy() });
        for (k, v) in fields.as_object().cloned().unwrap_or_default() {
            payload[k] = v;
        }
        let (code, out, err) = self.run_in(at, &["hook", event], &payload.to_string());
        assert_eq!(code, 0, "{event}: {err}");
        serde_json::from_str(out.trim()).unwrap_or(Value::Null)
    }

    fn claim_at(&self, at: &Path, args: &[&str]) -> String {
        let mut argv = vec!["claim"];
        argv.extend_from_slice(args);
        let (code, out, err) = self.run_in(at, &argv, "");
        assert_eq!(code, 0, "claim {args:?}: {out}{err}");
        out
    }

    fn json_at(&self, at: &Path) -> Value {
        let (code, out, err) = self.run_in(at, &["status", "--json"], "");
        assert_eq!(code, 0, "{err}");
        serde_json::from_str(&out).expect("status json")
    }

    /// The worktree switched on by the person, its holder's session started
    /// on opus, a sonnet subagent spawned, and a confirmed plan of two parts:
    /// part 1 held by the session, part 2 by its subagent.
    fn lane_with_a_plan(&self) -> PathBuf {
        let wt = self.worktree("lane");
        assert_eq!(self.run_in(&wt, &["on"], "").0, 0);
        self.hook_at(&wt, "SessionStart", json!({ "source": "startup", "model": "claude-opus-4-1-20250805" }));
        self.hook_at(&wt, "UserPromptSubmit", json!({ "prompt": ASKED }));
        self.hook_at(
            &wt,
            "PostToolUse",
            json!({ "tool_name": "Agent", "tool_input": { "description": "part 2", "prompt": "wave", "subagent_type": "general-purpose" },
                    "tool_response": { "status": "completed", "agentId": AGENT, "resolvedModel": "claude-sonnet-4-5-20250929", "content": [] } }),
        );
        let t = self.json_at(&wt)["said"][0]["for"].to_string();
        let sub = format!("{HOLDER}/{AGENT}");
        for args in [
            vec!["intent_submitted", "--for", &t, "--text", ASKED],
            vec!["one_reading", "--why", "the ask names what must change and how"],
            vec!["behavior", "--behavior_id", "b1", "--text", "the greeting"],
            vec!["promise", "--promise_id", "p1", "--behavior_id", "b1", "--accepts", "a name", "--gives_back", "a greeting", "--refuses", "nothing", "--cases", r#"[{"case_id":"n1","kind":"normal","given":"a name","expect":"a greeting"},{"case_id":"o1","kind":"opposite","given":"no name","expect":"no greeting"}]"#],
            vec!["criterion", "--criterion_id", "c1", "--text", "greet says hello", "--path", "full", "--behavior_id", "b1", "--promise_id", "p1", "--for", &t],
            vec!["criterion", "--criterion_id", "c2", "--text", "bye waves goodbye", "--path", "full", "--behavior_id", "b1", "--promise_id", "p1", "--for", &t],
            vec!["place_chosen", "--criterion_id", "c1", "--places", r#"[{"file_path":"src/app.sh","symbol":"greet","line_start":2,"line_end":2}]"#],
            vec!["place_chosen", "--criterion_id", "c2", "--places", r#"[{"file_path":"src/app.sh","symbol":"bye","line_start":3,"line_end":3}]"#],
            vec!["test_planned", "--criterion_id", "c1", "--file_path", "tests/test_c1.sh", "--will", "create"],
            vec!["test_planned", "--criterion_id", "c2", "--file_path", "tests/test_c2.sh", "--will", "create"],
            vec!["test_stack_detected", "--project_root", ".", "--test_stack", "sh", "--test_command", "sh tests/test_c1.sh"],
            vec!["criterion_head", "--criterion_id", "c1"],
            vec!["criterion_strung_on_head", "--criterion_id", "c2", "--head", "c1"],
            vec!["criterion_assigned", "--criterion_id", "c1", "--agent_conversation_id", HOLDER],
            vec!["criterion_assigned", "--criterion_id", "c2", "--agent_conversation_id", &sub],
            vec!["fix_location_category", "--criterion_id", "c1", "--category", "this_repository", "--where", ""],
        ] {
            self.claim_at(&wt, &args);
        }
        for c in ["c1", "c2"] {
            common::cases_kept_at(&wt, c); // the cases card passed (plan cases-card, 2026-10-25) [draft names]
        }
        let out = self.claim_at(&wt, &["fix_location_category", "--criterion_id", "c2", "--category", "this_repository", "--where", ""]);
        assert!(out.contains("familiar-ai: the plan is confirmed"), "{out}");
        wt
    }
}

/// An ask's parts as the checkout entry lists them, with only what this test reads.
fn parts(ask: &Value) -> Vec<Value> {
    ask["parts"]
        .as_array()
        .expect("parts")
        .iter()
        .map(|p| json!({ "part": p["part"], "text": p["text"], "state_name": p["state_name"], "judge": p["judge"], "held_by": p["held_by"] }))
        .collect()
}

#[test]
fn main_switched_off_lists_the_worktree_with_its_words_asks_parts_and_who_holds_each() {
    let p = project("seen");
    assert_eq!(p.run(&["off"], "").0, 0);
    let wt = p.lane_with_a_plan();

    let main = p.json_at(&p.repo());
    assert_eq!(main["off"], json!(true));
    let list = main["checkouts"].as_array().expect("checkouts");
    assert_eq!(list.len(), 1, "{main}");
    let lane = &list[0];
    assert_eq!(lane["folder"], "lane");
    assert_eq!(lane["branch"], "lane");
    assert_eq!(lane["busy"], json!(false));
    assert_eq!(json!({ "off": lane["off"], "switch_from": lane["switch_from"], "main_off": lane["main_off"] }), json!({ "off": false, "switch_from": "here", "main_off": true }));
    assert_eq!(lane["said"][0]["text"], ASKED);
    assert_eq!(lane["said"][0]["became"], "ask");
    let asks = lane["asks"].as_array().expect("asks");
    assert_eq!(asks.len(), 1);
    assert_eq!(asks[0]["text"], ASKED);
    assert_eq!(
        parts(&asks[0]),
        vec![
            json!({ "part": 1, "text": "greet says hello", "state_name": "testing", "judge": null, "held_by": "held by: opus, conversation 6f1d2e3a" }),
            json!({ "part": 2, "text": "bye waves goodbye", "state_name": "waiting", "judge": null, "held_by": "held by: a subagent (sonnet)" }),
        ]
    );
    assert_eq!(lane["cards"], json!([]));
    assert!(lane["runs"].is_array(), "{lane}");

    // this checkout's own parts carry the same words
    let own = p.json_at(&wt);
    let held: Vec<Value> = own["criteria"].as_array().expect("criteria").iter().map(|c| c["held_by"].clone()).collect();
    assert_eq!(held, [json!("held by: opus, conversation 6f1d2e3a"), json!("held by: a subagent (sonnet)")]);
    // and the worktree shows main, idle and off
    let mains = own["checkouts"].as_array().expect("checkouts");
    assert_eq!(mains.len(), 1);
    assert_eq!(mains[0]["folder"], "repo");
    assert_eq!(mains[0]["branch"], "main");
    assert_eq!(mains[0]["off"], json!(true));
    assert_eq!(mains[0]["mode"], "idle");

    // main's status line carries the worktree's entry after its own switch
    assert_eq!(p.run(&["statusline"], "").1, "repo | Familiar | OFF · lane: 1 testing 1 waits\n");
    // the worktree's carries none for main, which has no run open
    assert_eq!(p.run_in(&wt, &["statusline"], "").1.matches(" · repo").count(), 0);
}

#[test]
fn a_busy_worktree_record_is_listed_as_busy_and_never_dropped() {
    let p = project("busy");
    let wt = p.lane_with_a_plan();
    let held = Record::open(&wt.join(".familiar/record")).expect("hold the worktree's record");
    let main = p.json_at(&p.repo());
    drop(held);
    let list = main["checkouts"].as_array().expect("checkouts");
    assert_eq!(list.len(), 1, "{main}");
    assert_eq!(list[0]["folder"], "lane");
    assert_eq!(list[0]["busy"], json!(true));
    assert_eq!(p.json_at(&p.repo())["checkouts"][0]["busy"], json!(false), "let go: read again");
    // main switched on reads the same entry after its own idle line
    assert_eq!(p.run(&["statusline"], "").1, "repo | Familiar | ON | idle · lane: 1 testing 1 waits\n");
}
