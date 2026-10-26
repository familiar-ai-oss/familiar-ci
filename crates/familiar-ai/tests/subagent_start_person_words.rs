//! A subagent the agent starts through the Agent tool is told, at its start,
//! the person's words for the run that is open, in the investigator's form,
//! and the part it holds when it holds one. With no run open it is told nothing.

mod common;

use common::project::DEFAULT_SESSION;
use common::Project;
use serde_json::{json, Value};

const ASKED: &str = "make greet say hello, not hi";
const HEAD: &str = "familiar-ai: this subagent works for the person, in a run that is open.\n\nTHE PERSON ASKED (their words):\nmake greet say hello, not hi";

fn started(p: &Project, agent: &str) -> Value {
    p.hook("SubagentStart", json!({ "agent_id": agent, "agent_type": "general-purpose" }))
}

#[test]
fn a_subagent_is_told_the_persons_words_and_its_part() {
    let p = Project::new("told");
    assert_eq!(started(&p, "a6"), json!({}), "no run open: nothing is told");
    p.hook("UserPromptSubmit", json!({ "prompt": ASKED }));
    let t = p.facts().iter().find(|f| f.kind == "person_said").map(|f| f.t).expect("the person's prompt is recorded");
    p.claim(&["intent_submitted", "--for", &t.to_string(), "--text", "the agent's reading: greet says hello"]);

    let v = started(&p, "a7");
    assert_eq!(v["hookSpecificOutput"]["hookEventName"], "SubagentStart", "{v}");
    assert_eq!(v["hookSpecificOutput"]["additionalContext"], HEAD, "{v}");

    p.claim(&["criterion", "--criterion_id", "c1", "--text", "greet says hello", "--path", "light", "--for", &t.to_string()]);
    p.claim(&["criterion_assigned", "--criterion_id", "c1", "--agent_conversation_id", &format!("{DEFAULT_SESSION}/a7")]);
    let v = started(&p, "a7");
    assert_eq!(v["hookSpecificOutput"]["additionalContext"], format!("{HEAD}\n\nTHE PART THIS SUBAGENT HOLDS: \"greet says hello\""), "{v}");
    let other = started(&p, "a8");
    assert_eq!(other["hookSpecificOutput"]["additionalContext"], HEAD, "a subagent that holds no part: {other}");
    assert_eq!(p.facts_of("person_said").len(), 1, "a subagent's start records nothing of the person's");
}
