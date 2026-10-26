//! What the person asked for: the intent, its behaviors, promises and criteria, the person's
//! prompts and how each was answered, and the person's off switch.

use serde::{Deserialize, Serialize};

/// `path: full | light`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Path {
    Full,
    Light,
}

/// The intent the run was started from, with main's sha at submit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IntentSubmitted {
    pub text: String,
    pub sha: String,
    /// The `t` of the `person_said` this intent answers, if any.
    #[serde(rename = "for", default, skip_serializing_if = "Option::is_none")]
    pub for_t: Option<u64>,
}

/// The readings the agent sees in the person's ask when it reads more than one way (the owner's
/// plan of 2026-10-21, step 2) [draft name]: two or three, each in plain words; the person picks
/// one on a card.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReadingAlternatives {
    pub readings: Vec<String>,
}

/// The agent's word that the person's ask reads one way, and why [draft name]: it answers a
/// trigger with no card.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OneReading {
    pub why: String,
}

/// A behavior the product keeps, in the person's words (full path only).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Behavior {
    pub behavior_id: String,
    pub text: String,
}

/// A behavior's promise: accepts, gives back, refuses (full path only).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Promise {
    pub promise_id: String,
    pub behavior_id: String,
    pub accepts: String,
    pub gives_back: String,
    pub refuses: String,
}

/// A case's kind in a promise's contract [draft names]: `normal` shows the behavior asked for;
/// `opposite` is the same call with the trigger absent; `edge` is a boundary the person's words
/// name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CaseKind {
    Normal,
    Opposite,
    Edge,
}

/// One case of a promise's contract [draft names]: what is given and what is expected. It is a
/// field of the `promise` fact, not of the typed form the sealed tests build.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PromiseCase {
    pub case_id: String,
    pub kind: CaseKind,
    /// One line the person reads on the cases card [draft name] (the owner's plan of 2026-10-25,
    /// §3); a case without one is read by its `given`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub short: Option<String>,
    pub given: String,
    pub expect: String,
}

/// A criterion: one checkable line of the intent, marked full or light.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Criterion {
    pub criterion_id: String,
    pub text: String,
    pub path: Path,
    /// Set on the full path only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub behavior_id: Option<String>,
    /// Set on the full path only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub promise_id: Option<String>,
    /// The `t` of the `person_said` this criterion answers.
    #[serde(rename = "for", default, skip_serializing_if = "Option::is_none")]
    pub for_t: Option<u64>,
}

/// A prompt the person typed into their agent, never a task notification.
/// Written by the binary only; the claim command refuses it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PersonSaid {
    pub text: String,
    /// The session the person typed it in, so a lane carries only the words
    /// of the session that launches it; a prompt recorded before it was kept
    /// has none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_conversation_id: Option<String>,
}

/// How a person's prompt was answered without a criterion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum How {
    /// The prompt asked for no work.
    NoWork,
    /// The prompt was a question, and it was answered.
    Question,
}

/// The agent's claim that a `person_said` needs no criterion.
/// `for` holds the `t` of that `person_said`, since `t` is a stamped key.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PersonSaidAnswered {
    #[serde(rename = "for")]
    pub for_t: u64,
    pub how: How,
}

/// The person's off switch for a project: while the latest says `off`, every hook allows everything silently.
/// Written by `familiar-ai off` and `familiar-ai on` from the person's terminal, never from an agent's shell.
/// See docs/decisions.md#the-off-switch
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FamiliarSwitch {
    /// One of `SWITCH_TO`.
    pub to: String,
}
