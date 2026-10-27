//! Where a criterion's work lives in the code, and whether those lines still match.

use serde::{Deserialize, Serialize};

/// One row of `place_chosen.places`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Place {
    pub file_path: String,
    pub symbol: String,
    pub line_start: u64,
    pub line_end: u64,
}

/// The fact that chose the anchor: existing lines for a criterion.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlaceChosen {
    pub criterion_id: String,
    pub places: Vec<Place>,
    /// The person's answer this place records, written only by the binary right after
    /// `hold_answered {hold: anchor_untouched, choice: keep_going}`: the place where the
    /// change landed, appended to the plan with the stamp that the person overrode it
    /// (the owner, 2026-10-13). Absent on every place an agent chose.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hold: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub choice: Option<String>,
}

/// The fact that chose the anchor: a spot for new work beside an existing symbol.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NewWorkSpotChosen {
    pub criterion_id: String,
    pub file_path: String,
    pub symbol: String,
    pub beside: String,
}

/// The fact that chose the anchor for greenfield work, announced by the agent.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NewWorkNoSpot {
    pub criterion_id: String,
    /// `<file>[:<symbol>]`: the caller the new work is reached from, or where it is stored when nothing calls it yet.
    /// See docs/decisions.md#new-work-names-its-integration-point
    pub integration_point: String,
}

/// `checked_at: planning | start`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CheckedAt {
    Planning,
    Start,
}

/// The result of checking an anchor's lines against the code.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnchorShaMatched {
    pub criterion_id: String,
    pub file_path: String,
    pub symbol: String,
    pub line_start: u64,
    pub line_end: u64,
    pub sha: String,
    pub lines_hash: String,
    pub matched: bool,
    pub checked_at: CheckedAt,
    /// The start check only (the owner, 2026-10-11): the working tree's
    /// lines at the mapped range equal the lines at the record's tip commit.
    /// Absent on the planning check.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tip_matched: Option<bool>,
}

/// The binary's check that a light criterion's anchor is documentation (the
/// owner, 2026-10-11): one spot of the anchor read at `sha`. A place names
/// its file, symbol and lines; a new-work anchor its file alone. Mirrors
/// `anchor_sha_matched` without `lines_hash` and `checked_at`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnchorIsDocumentation {
    pub criterion_id: String,
    pub file_path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub symbol: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line_start: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line_end: Option<u64>,
    /// The sha the lines were read at.
    pub sha: String,
    /// Markdown, or the lines are one docstring.
    pub matched: bool,
}

/// One function a try's failing tests ran, as the binary lists it (the owner's plan of
/// 2026-10-23, the shared interface) [draft names]: its rank by match to the person's words, its
/// file relative to the root, its name as the recorder saw it (`Class.method` or a bare name) and
/// its lines.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TracedFunction {
    pub rank: u64,
    pub file_path: String,
    pub symbol: String,
    pub line_start: u64,
    pub line_end: u64,
}

/// The binary's listing after a try of a part's test (`familiar-ai try`) [draft name]: the try's
/// run and sha, whether its tests failed (`red`), how many functions its failing tests ran
/// (`path_size`), how they were ranked (`words` or `embedder+words`), the query (the person's
/// words, with their answer to the one question) and at most 25 of them, best first. `top` is
/// empty when nothing failed or nothing was recorded. Written by the binary only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TraceListed {
    pub criterion_id: String,
    pub run_id: String,
    pub sha: String,
    pub red: bool,
    pub path_size: u64,
    pub ranker: String,
    pub query: String,
    pub top: Vec<TracedFunction>,
}

/// The agent's one-line reason its place is not on the try's list [draft name], read with the
/// latest `place_chosen` it follows; the seal's judge reads it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OffListReason {
    pub criterion_id: String,
    pub reason: String,
}

/// One case the case-finder found (the owner's plan of 2026-10-25, the cases card) [draft names]:
/// its id (`a1`, `a2`, …), a short line the person reads on the card, what is given, what a
/// correct change gives, and why the person's words or the code make it necessary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FoundCase {
    pub case_id: String,
    pub short: String,
    pub given: String,
    pub expect: String,
    pub why: String,
}

/// The case-finder's finding on a part [draft name], recorded by the binary from the subagent's
/// answer: `found` with its cases, `none` with why the planned cases cover it, or `failed` with why
/// (cut off, no valid answer); the model that ran and the tokens it used. Written by the binary only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CasesFound {
    pub criterion_id: String,
    pub outcome: String,
    pub cases: Vec<FoundCase>,
    pub why: String,
    pub model: String,
    pub tokens: u64,
    /// The lens panel's questions for the person (the owner's plan of 2026-10-26, approved): at
    /// most three, written by the synthesis; absent when it asks none, and on every finding the
    /// case-finder recorded before the panel.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub questions: Vec<FoundQuestion>,
}

/// One question the lens panel's synthesis asks the person (the owner's plan of 2026-10-26,
/// approved): its id (`q1`..`q3`), its text, two to four options and why it decides the cases.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FoundQuestion {
    pub question_id: String,
    pub text: String,
    pub options: Vec<String>,
    pub why: String,
}

/// Which Agent call got which lens of the panel (the owner's plan of 2026-10-26, approved): the
/// part, one of the five lenses or the synthesis, the call's tool_use_id. Written by the binary only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LensReleased {
    pub criterion_id: String,
    pub lens: String,
    pub tool_use_id: String,
}

/// One finding of a lens: its kind, a note, where in the code (both may be empty) and the question
/// it would put to the person (may be empty).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LensFinding {
    pub kind: String,
    pub note: String,
    pub file_path: String,
    pub target_symbol: String,
    pub candidate_question: String,
}

/// A lens's answer on a part (the owner's plan of 2026-10-26, approved), recorded by the binary:
/// the lens, its outcome (found, none or failed), its findings, the model that ran and the tokens
/// it used. The synthesis is not a lens's answer: it is recorded as `cases_found`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LensFound {
    pub criterion_id: String,
    pub lens: String,
    pub outcome: String,
    pub findings: Vec<LensFinding>,
    pub model: String,
    pub tokens: u64,
}

/// The person's pick on one of the synthesis's questions (the owner's plan of 2026-10-26,
/// approved), recorded by the picker: the question's id, the option's text or `typed`, and the
/// words typed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuestionAnswered {
    pub criterion_id: String,
    pub question_id: String,
    pub choice: String,
    pub typed: String,
}
