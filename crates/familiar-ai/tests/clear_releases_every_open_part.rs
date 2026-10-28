//! The person clears a project's parts with `familiar-ai clear`, from the band's button or a
//! terminal of their own (the owner, 2026-10-27): every part not yet released or proven is released
//! by the person (`hold_answered`, choice `release`), its record kept, so the next ask is planned
//! fresh. From an agent's session, or an agent's command that runs it, it is refused, as the switch is.
//! Each release it writes says so (`from: clear`, the owner, 2026-10-28): only the person can write
//! one, so a cleared part reads released by you, never "answered, not verified", and leaves
//! `familiar-ai status` and `status --json` (the pane's), the record keeping it. A part released on
//! a card stays, with its answer's flag, as before.

mod common;

use common::Project;
use familiar_ai_facts::Record;
use serde_json::{json, Value};

const REFUSAL: &str = "familiar-ai: not cleared · familiar-ai clear is the person's, not the agent's: the person clears the parts with the button or by running familiar-ai clear in a terminal of their own";

/// Two parts planned by the agent's session, c2 already released by the person.
fn project(name: &str) -> Project {
    let p = Project::builder(name).create();
    for args in [
        vec!["intent_submitted", "--for", "0", "--text", "greet and part"],
        vec!["behavior", "--behavior_id", "b1", "--text", "the greeting"],
        vec!["criterion", "--criterion_id", "c1", "--text", "greet says hello", "--path", "full", "--behavior_id", "b1"],
        vec!["criterion", "--criterion_id", "c2", "--text", "part says goodbye", "--path", "full", "--behavior_id", "b1"],
        vec!["criterion_assigned", "--criterion_id", "c1"],
        vec!["criterion_assigned", "--criterion_id", "c2"],
    ] {
        p.claim(&args);
    }
    p.persons_release("c2");
    p
}

impl Project {
    fn persons_release(&self, c: &str) {
        let record = Record::open_waiting(&self.record_path(), std::time::Duration::from_secs(5)).expect("the record");
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0) + 1.0;
        let fields = json!({ "criterion_id": c, "hold": "anchor_untouched", "choice": "release", "typed": "" });
        record.append_raw("hold_answered", fields.as_object().cloned().unwrap_or_default(), now, familiar_ai::TENANT).expect("the release");
    }

    /// Each release's `from`, in record order.
    fn froms(&self) -> Vec<Option<String>> {
        self.facts()
            .into_iter()
            .filter(|f| f.kind == "hold_answered" && f.fields.get("choice").and_then(Value::as_str) == Some("release"))
            .map(|f| f.fields.get("from").and_then(Value::as_str).map(str::to_string))
            .collect()
    }

    /// The parts each `release` names, in record order.
    fn releases(&self) -> Vec<String> {
        self.facts()
            .into_iter()
            .filter(|f| f.kind == "hold_answered" && f.fields.get("choice").and_then(Value::as_str) == Some("release"))
            .filter_map(|f| f.fields.get("criterion_id").and_then(Value::as_str).map(str::to_string))
            .collect()
    }
}

#[test]
fn clear_releases_every_open_part_once_from_the_persons_terminal() {
    let p = project("clear");
    p.set_env("CLAUDE_CODE_SESSION_ID", None);
    let (code, out, err) = p.run(&["clear"], "");
    assert_eq!(code, 0, "{out}{err}");
    assert_eq!(out, "Familiar cleared 1 part of this project: \"greet says hello\". It is released, its record kept.\n");
    assert_eq!(p.releases(), ["c2", "c1"], "c1 released, c2 not again");
    assert_eq!(p.froms(), [None, Some("clear".to_string())], "clear's release says so; the card's does not");
    // c1 cleared leaves the status; c2, released on a card, stays with its flag
    let ids: Vec<Value> = p.status_json()["criteria"].as_array().cloned().unwrap_or_default().iter().map(|c| c["id"].clone()).collect();
    assert_eq!(ids, [json!("c2")]);
    assert!(p.criterion_json("c2")["holds"].to_string().contains("\"released\""), "{}", p.criterion_json("c2"));
    let (code, status, _) = p.run(&["status"], "");
    assert_eq!(code, 0);
    assert!(!status.contains("\"greet says hello\" · "), "{status}");
    assert!(!status.contains("part 1 ·"), "no answer flag for a cleared part: {status}");
    assert!(status.contains("familiar-ai: \"part says goodbye\" · "), "{status}");
    assert!(status.contains("part 2 · anchor_untouched release: answered, not verified"), "the card's release keeps its flag: {status}");
    // nothing left to clear
    let (code, out, _) = p.run(&["clear"], "");
    assert_eq!((code, out.as_str()), (0, "Familiar has no open part to clear in this project.\n"));
    assert_eq!(p.releases().len(), 2);
}

#[test]
fn clear_is_refused_from_an_agents_session() {
    let p = project("agent");
    let (code, out, err) = p.run(&["clear"], "");
    assert_eq!(code, 1, "{out}");
    assert_eq!(err, format!("{REFUSAL}\n"));
    assert_eq!(p.releases(), ["c2"], "nothing released");
}

#[test]
fn an_agents_command_that_runs_clear_is_refused_whatever_its_environment() {
    let p = project("shell");
    for cmd in ["familiar-ai clear", "env -u CLAUDE_CODE_SESSION_ID familiar-ai clear", "cd /tmp && env -i PATH=$PATH ./target/debug/familiar-ai clear"] {
        let v = p.bash("PreToolUse", cmd);
        assert_eq!(v["hookSpecificOutput"]["permissionDecision"], "deny", "{cmd}: {v}");
        assert_eq!(v["hookSpecificOutput"]["permissionDecisionReason"], json!(REFUSAL), "{cmd}");
    }
    let v = p.bash("PreToolUse", "grep -n 'familiar-ai clear' docs/reference.md");
    assert_ne!(v["hookSpecificOutput"]["permissionDecision"], "deny", "{v}");
}
