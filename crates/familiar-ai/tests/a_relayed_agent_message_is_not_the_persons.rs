//! A message Claude Code relays from an agent arrives through UserPromptSubmit
//! like a typed prompt, framed by the harness: `Another Claude session sent a
//! message:`, `<agent-message from="…">`, `[Subagent hand-back]`. It is never
//! the person's words (docs/decisions.md §"Holds are the person's": a
//! subagent's or a launched session's question is never the person's), on or off.

mod common;

use common::Project;
use serde_json::json;

/// The frames seen on 2026-10-12, each with a hand-back's text after it.
const RELAYED: [&str; 4] = [
    "Another Claude session sent a message:\nlane done: c40 locally green at b578658",
    "<agent-message from=\"governance-edges\">\nthe three rules are in; referee green\n</agent-message>",
    "[Subagent hand-back]\nbranch lane-x, four commits, referee last lines below",
    "  \n[Subagent hand-back] finished: nothing left undone",
];

fn said(p: &Project) -> Vec<String> {
    p.facts_of("person_said").iter().filter_map(|f| f["text"].as_str().map(str::to_string)).collect()
}

#[test]
fn a_relayed_agent_message_is_never_person_said_and_a_typed_prompt_still_is() {
    let p = Project::new("relayed-on");
    p.hook("UserPromptSubmit", json!({ "prompt": "make greet say hello" }));
    for message in RELAYED {
        let out = p.hook("UserPromptSubmit", json!({ "prompt": message }));
        assert_eq!(out, json!({}), "a relayed message is not told as the person's: {out}");
    }
    p.hook("UserPromptSubmit", json!({ "prompt": "and keep hi for the old callers" }));
    assert_eq!(said(&p), ["make greet say hello", "and keep hi for the old callers"]);
    // a typed prompt that only mentions a hand-back further on is still the person's
    p.hook("UserPromptSubmit", json!({ "prompt": "read the [Subagent hand-back] above and land it" }));
    assert_eq!(said(&p).last().map(String::as_str), Some("read the [Subagent hand-back] above and land it"));
}

#[test]
fn switched_off_a_relayed_agent_message_is_not_recorded_either() {
    let p = Project::new("relayed-off");
    assert_eq!(p.run_as(None, &["off"], "").0, 0);
    for message in RELAYED {
        p.hook_raw("UserPromptSubmit", &p.payload("UserPromptSubmit", json!({ "prompt": message })));
    }
    assert!(said(&p).is_empty(), "{:?}", said(&p));
    // so status shows none of them as the person's words
    let status = p.status_json().to_string();
    assert!(!status.contains("hand-back") && !status.contains("lane done"), "{status}");
}
