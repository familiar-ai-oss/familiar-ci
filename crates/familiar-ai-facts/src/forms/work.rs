//! Who holds each criterion and in what order: the string of criteria, assignments and transfers
//! between conversations, the model each ran, the person's answers to holds, the dependency a
//! person approved, what was told to whom, and the subagents that ended.

use serde::{Deserialize, Serialize};

use super::anchor::Place;

/// The criterion at the head of the string.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CriterionHead {
    pub criterion_id: String,
}

/// A criterion strung behind another on the agent's task list.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CriterionStrungOnHead {
    pub criterion_id: String,
    pub head: String,
}

/// The criterion given to one agent conversation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CriterionAssigned {
    pub criterion_id: String,
    pub agent_conversation_id: String,
}

/// The criterion handed from one conversation to another.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CriterionTransfer {
    pub criterion_id: String,
    pub from_conversation: String,
    pub to_conversation: String,
}

/// Which model a conversation ran, written by the binary only from Claude Code's events.
/// The model's rung is derived from its family name and never stored.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelSeen {
    /// The session id, or the session id and agent id joined by `/`.
    pub agent_conversation_id: String,
    /// The model as Claude Code reports it.
    pub model: String,
}

/// The person's answer to a held criterion, given in the terminal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HoldAnswered {
    pub criterion_id: String,
    /// The hold's reason, as free text.
    pub hold: String,
    pub choice: String,
    /// The free-text answer when no offered choice fits.
    pub typed: String,
}

/// The dependency drafted for an approved `a_dependency`, written by the binary only.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DependencyDrafted {
    /// The `t` of the `hold_answered` this draft answers.
    pub approval: u64,
    /// The dependency's own criterion id.
    pub criterion_id: String,
    pub text: String,
    pub test_file: String,
    /// Where its code sits, as a `place_chosen` place, when the draft names one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub place: Option<Place>,
}

/// An approval the binary could not draft, and why; written by the binary only.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DependencyUnanswered {
    /// The `t` of the `hold_answered` it was drafting for.
    pub approval: u64,
    pub why: String,
}

/// Something owed to an agent was told, once: a run's result or a person's pick.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Told {
    /// A `run_id`, or the `t` of a `hold_answered` written as text.
    pub about: String,
}

/// A criterion closed as covered by another, its dependency.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CriterionCovered {
    pub criterion_id: String,
    pub by_criterion: String,
}

/// A subagent ended at an allowed `SubagentStop`, or a lane's headless session at its allowed
/// stop (`agent_type: lane`, its session id as `agent_id`); written by the binary only.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SubagentEnded {
    pub agent_id: String,
    pub agent_type: String,
}

/// A lane's headless session started again (`launch_lane --resume`) with the id of a holder
/// that ended (the owner's draft name, 2026-10-16): the latest of this and `subagent_ended`
/// says whether the holder runs. Written by the binary only.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HolderResumed {
    pub agent_id: String,
}
