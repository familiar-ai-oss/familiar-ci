//! What a judge or an investigator said about a criterion, and where its fix lives.

use serde::{Deserialize, Serialize};

/// The judge's answer: does the sealed test answer the intent?
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AnswersIntent {
    Yes,
    No,
}

/// What the judge, a model one rung above the holder, answered at a seal.
/// Written by the binary only, after the seal's stamp; the claim command refuses it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JudgeVerdict {
    pub criterion_id: String,
    /// The model that answered, as Claude Code reports it (else the alias asked).
    pub model: String,
    pub answers_intent: AnswersIntent,
    /// The judge's one sentence.
    pub reason: String,
    /// The behaviour the intent describes that the sealed tests do not
    /// exercise, as the judge named it. Absent when none was named.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gap: Option<String>,
    /// For each case of the promise's contract, the test function the judge named as exercising
    /// it [draft names]; empty when the criterion has no contract or the answer named none.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cases: Vec<CaseTest>,
    /// For each place the agent judged not affected, the judge's word on its reason [draft names];
    /// empty when none was shown.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub places: Vec<PlaceOk>,
}

/// The judge's word on a not_affected place's reason [draft names]: `ok` is false when the code
/// shown says the behavior reaches it after all.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlaceOk {
    pub file: String,
    pub line: u64,
    pub ok: bool,
}

/// One case of a promise's contract and the test function the judge named for it, `none` when
/// none exercises it; `ok` is the judge's word that the test checks the case's expected result
/// [draft names].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CaseTest {
    pub case_id: String,
    pub test: String,
    pub ok: bool,
}

/// The reviewer's word on a green fix [draft] (the owner's plan of 2026-10-20, step 3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FixCovers {
    Yes,
    /// Every quote was read again at the green commit.
    No,
    /// No verdict that counts: the call gave none, or a "no" whose quotes did not check out.
    NotJudged,
}

/// Why a review counts as not judged: a closed set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FixNotJudgedWhy {
    /// The call ended with an error or could not start.
    Error,
    /// The call outlived its wait and was stopped.
    Timeout,
    /// It exited 0, but no answer in the expected form could be read.
    Unparseable,
    /// A "no" with no quote, or a quote the binary did not find where it said.
    Unverified,
}

/// One quote of a review: a line of the repository at the reviewed commit, and its text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FixQuote {
    pub file_path: String,
    pub line: u64,
    pub text: String,
}

/// What the reviewer, the judge's model, answered about a green fix at `commit` [draft name].
/// Written by the binary only; the claim command refuses it. A `no` is written only after the
/// binary read every quote again at `commit`, each as the repository holds it there.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FixVerdict {
    pub criterion_id: String,
    /// The green commit the reviewer read.
    pub commit: String,
    /// The model that answered, as Claude Code reports it (else the alias asked).
    pub model: String,
    pub covers: FixCovers,
    /// The reviewer's reason; empty when the call gave no answer.
    pub reason: String,
    /// What the fix does not do, as the reviewer named it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gap: Option<String>,
    /// The quotes: as read again on a `no`, as given on an unverified one.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub quotes: Vec<FixQuote>,
    /// Why it is not judged.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub why: Option<FixNotJudgedWhy>,
    /// What happened, in a few words, when not judged.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

/// The judge's word on an announced change to a pre-existing test [draft name]: does the person's
/// ask call for that test's old expectation to change? Written by the binary only; `covers`
/// `not_judged` with `why` when the call gave no answer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExistingTestVerdict {
    pub file_path: String,
    /// The head the judge read the change at.
    pub commit: String,
    pub model: String,
    pub covers: FixCovers,
    pub reason: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gap: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub why: Option<FixNotJudgedWhy>,
}

/// Why a judge call yielded no verdict: a closed set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JudgeUnansweredWhy {
    /// The call ended with an error: a non-zero exit, a signal, a child that
    /// could not start, or Claude Code's own error result.
    Error,
    /// The call outlived the judge's wait and was stopped.
    Timeout,
    /// It exited 0, but no answer in the expected form could be read.
    Unparseable,
    /// It answered that it cannot see the sealed test, which points to a bug in what the binary showed it.
    CannotSee,
}

/// A judge call that yielded no verdict, written by the binary only; the claim command refuses it.
/// `seal` is the commit the judge read the sealed tests at.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JudgeUnanswered {
    pub criterion_id: String,
    pub seal: String,
    pub why: JudgeUnansweredWhy,
}

/// Why an investigator ended without the hand-back it was sent for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InvestigatorUnfinishedWhy {
    /// It used up its budget.
    Budget,
    /// It outlived its wait and was stopped.
    Timeout,
    /// It ended with an error, or could not start.
    Error,
    /// Sent for `two_failures`, it ended with the criterion not green.
    NoGreen,
    /// Sent for `two_tries_not_red`, it ended with no laptop seal standing.
    NoSeal,
    /// A diagnosis (S7b) exited 0 with no finding in the expected form.
    Unparseable,
    /// A diagnosis (S7b) quoted lines or words the binary did not find where it said.
    Unverified,
}

/// The investigator's diagnosis of a test that never went red [draft words, the owner,
/// 2026-10-16, S7b]. Written by the binary only, from the investigator's structured output,
/// after it re-read every quote. It clears no hold, raises none, narrows no card and relabels no run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InvestigatorFinding {
    pub criterion_id: String,
    /// The category and its own fields.
    #[serde(flatten)]
    pub category: FindingCategory,
    /// The model that answered, as Claude Code reports it.
    pub model: String,
    /// The runs the finding read, and for `behavior_exists` the binary's own run at planning.
    pub runs: Vec<FindingRun>,
    pub reason: String,
    /// The proof, each quote as the binary read it.
    pub quotes: Vec<Quote>,
}

/// What a test that never went red came from [draft words].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "category", rename_all = "snake_case")]
pub enum FindingCategory {
    /// The test checks something other than what was asked; `guidance` is what it should check.
    WrongTest { guidance: String },
    /// The code already does what was asked.
    BehaviorExists {},
    /// The part, as planned, left out something the person asked for.
    RequirementMissed { missed: String, scope: String },
}

impl FindingCategory {
    /// The category's word.
    pub fn word(&self) -> &'static str {
        match self {
            FindingCategory::WrongTest { .. } => "wrong_test",
            FindingCategory::BehaviorExists {} => "behavior_exists",
            FindingCategory::RequirementMissed { .. } => "requirement_missed",
        }
    }

    /// The fields this category carries besides `category`.
    pub fn own_fields(word: &str) -> &'static [&'static str] {
        match word {
            "wrong_test" => &["guidance"],
            "requirement_missed" => &["missed", "scope"],
            _ => &[],
        }
    }
}

/// The category words, in the order the investigator is asked to check them.
pub const FINDING_CATEGORIES: [&str; 3] = ["wrong_test", "behavior_exists", "requirement_missed"];

/// A run a finding read: its id, commit and label.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FindingRun {
    pub run_id: String,
    pub commit: String,
    pub label: String,
}

/// One quote of a finding's proof.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "role", rename_all = "snake_case")]
pub enum Quote {
    /// Lines of a test file at a commit.
    Test(QuotedLines),
    /// Lines of the code at a commit.
    Code(QuotedLines),
    /// The person's words: `said` is the `t` of their `person_said`.
    Person { said: u64, text: String },
}

/// Lines `line_start`..=`line_end` of `file_path` at `commit`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QuotedLines {
    pub file_path: String,
    pub line_start: u64,
    pub line_end: u64,
    pub commit: String,
    pub text: String,
}

impl Quote {
    /// The quoted text.
    pub fn text(&self) -> &str {
        match self {
            Quote::Test(q) | Quote::Code(q) => &q.text,
            Quote::Person { text, .. } => text,
        }
    }
}

/// An investigator that ended without the hand-back it was sent for, written by the binary only.
/// `conversation` is the `to_conversation` of the transfer that sent it; the fact ends that
/// investigator's chain, so the hold shows again.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InvestigatorUnfinished {
    pub criterion_id: String,
    pub conversation: String,
    pub why: InvestigatorUnfinishedWhy,
}

/// Where a criterion's fix lives.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FixLocation {
    ThisRepository,
    Dependency,
    Environment,
    /// The fix is a request to someone else.
    Request,
}

/// The holder's claim of where a criterion's fix lives, written by the agent or an investigator, never the person.
/// See docs/decisions.md#a-fix-outside-the-repository-is-a-card
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FixLocationCategory {
    pub criterion_id: String,
    pub category: FixLocation,
    /// Where the fix lives: empty for `this_repository`.
    #[serde(rename = "where")]
    pub where_: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub needs: Option<String>,
}
