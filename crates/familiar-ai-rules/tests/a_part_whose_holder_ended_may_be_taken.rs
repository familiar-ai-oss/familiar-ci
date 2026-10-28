//! A part's holder has ended when, since the part was last assigned to it, the latest of its
//! conversation's `subagent_ended` and `holder_resumed` is `subagent_ended` (a session's end is
//! `subagent_ended` with agent_type `session`, the owner's answer of 2026-10-27); the next
//! session then takes the part at its first claim or edit. A holder still running, resumed, or the
//! part assigned again since has not ended; a part nobody holds has no holder to end.

mod traced;

use familiar_ai_facts::Log;
use familiar_ai_rules::{plan, View};
use serde_json::json;
use traced::*;

fn ended(log: &Log, id: &str) -> Log {
    raw(log, "subagent_ended", json!({ "agent_id": id, "agent_type": "session" }))
}

fn ended_now(log: &Log) -> bool {
    plan::holder_ended(&View::new(log), "c1")
}

#[test]
fn a_part_is_free_to_take_once_its_holders_session_ended() {
    // c1 is held by a1
    let log = planned();
    assert!(!ended_now(&log), "a1 runs");
    assert!(!ended_now(&ended(&log, "a2")), "another session's end");
    let log = ended(&log, "a1");
    assert!(ended_now(&log), "a1 ended");
    assert!(!ended_now(&raw(&log, "holder_resumed", json!({ "agent_id": "a1" }))), "a1 resumed");
    assert!(!ended_now(&raw(&log, "criterion_assigned", json!({ "criterion_id": "c1", "agent_conversation_id": "a2" }))), "taken by a2, which runs");
}

#[test]
fn an_end_before_the_part_was_assigned_is_not_its_holders_end() {
    let mut log: Log = ended(&Vec::new(), "a1");
    for f in planned() {
        log = raw(&log, f.kind, serde_json::Value::Object(f.fields));
    }
    assert!(!ended_now(&log));
}

#[test]
fn a_part_nobody_holds_has_no_holder_to_end() {
    let log: Log = planned().into_iter().filter(|f| f.kind != "criterion_assigned").fold(Vec::new(), |l, f| raw(&l, f.kind, serde_json::Value::Object(f.fields)));
    assert!(!ended_now(&ended(&log, "a1")));
}
