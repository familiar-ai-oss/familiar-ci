//! A test run's life: reserved, started, asked to stop, ended, and what its output says.

use serde::{Deserialize, Serialize};

/// A test run the binary reserves before launching it; at most one live reservation per
/// criterion, rev and commit. Numbers are stored as text.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunReserved {
    pub run_id: String,
    pub criterion_id: String,
    pub criterion_rev: String,
    pub commit: String,
    pub command: String,
    /// What the run holds while it lives, freed only when its process group is confirmed gone.
    pub resources: Vec<String>,
    /// Seconds the command may take.
    pub wait: String,
    pub by: String,
    /// The earliest time, in seconds, a retry may start.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
}

/// The detached child began the reserved run; the fact's `at` is when it began.
/// `pgroup_start` is the group's start time, so a reused group id is never mistaken for it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunStarted {
    pub run_id: String,
    pub pgid: String,
    pub pgroup_start: String,
}

/// A person asked for the run to stop; whatever it ends with after this is not proof.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunCancelRequested {
    pub run_id: String,
}

/// The run ended. `outcome` is passed, failed or did_not_run; `end` is exited, signalled,
/// timed_out, empty_command, no_directory, not_started, cancelled or killed. The rest is the
/// run's evidence, absent on every run ended before it was kept (the owner's plan of
/// 2026-10-14, §3).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct RunEnded {
    pub run_id: String,
    pub outcome: String,
    pub end: String,
    /// The command's exit code, when it exited.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i64>,
    /// The signal that ended it, when one did.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signal: Option<i64>,
    /// The cut of the cleaned standard output.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stdout_tail: Option<String>,
    /// The cut of the cleaned standard error.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stderr_tail: Option<String>,
    /// One row per stream the drain cut.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub truncated: Option<Vec<Truncated>>,
}

/// `truncated[].stream: stdout | stderr | report`; `report` is the text of the runner's report
/// files.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Stream {
    Stdout,
    Stderr,
    Report,
}

/// One row of `run_ended.truncated`: the bytes a stream carried and the bytes the drain dropped.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Truncated {
    pub stream: Stream,
    pub total_bytes: u64,
    pub dropped_bytes: u64,
}

/// What a run's output says happened, read by the binary from the run's evidence; at most one per
/// run. `by` is regex, binary or reporter [draft], `label` one of the outcomes, `classified_on` full (the whole
/// cleaned capture) or cut (the stored cut only); see `CLOSED_FOR_KIND`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TestLogTranslation {
    pub run_id: String,
    pub criterion_id: String,
    pub commit: String,
    pub by: String,
    pub label: String,
    /// One line, verbatim from the cleaned text searched.
    pub evidence: String,
    pub failing_tests: Vec<String>,
    pub classified_on: String,
    /// Where the output points, root-relative.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub location: Option<Location>,
    /// The name the output says is missing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub missing_name: Option<String>,
    /// The counts the output gave [draft name], `{}` when it gave none; absent on every
    /// translation written before it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub counts: Option<Counts>,
    /// The tests the runner said ran, filtered to the plan's test files [draft name]; written
    /// when Familiar's own reporter read the run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ran_tests: Option<Vec<String>>,
    /// The tests the runner said it skipped, filtered the same way [draft name].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skipped_tests: Option<Vec<String>>,
    /// The runner's and its plugins' versions as the reporter saw them [draft name], to spot drift
    /// from the version the reader was chosen under.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub versions: Option<String>,
    /// The deciding error's project frames as the reporter recorded them [draft name], from the
    /// test down to where the error was raised, each function as the runner named it; at most
    /// [`FRAMES_KEPT`] (the first and the last half when there were more). Written only by a reader
    /// that records frames (Familiar's own reporter); the can't-tell card shows it (2026-10-20).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frames: Option<Vec<Location>>,
}

/// The most frames `test_log_translation.frames` keeps: the first half and the last half.
pub const FRAMES_KEPT: usize = 64;

/// `test_log_translation.counts` [draft]: what the runner said about how many tests ran, each a
/// whole number and each left out when the output did not say.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Counts {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub passed: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failed: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skipped: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deselected: Option<u64>,
}

/// `location.kind` [draft]: why the output points at the place: an `assertion` that failed, an
/// `exception` raised, a `compile` error or a `signal`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LocationKind {
    Assertion,
    Exception,
    Compile,
    Signal,
}

/// `test_log_translation.location`: a file and a line in it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Location {
    pub file_path: String,
    pub line: u64,
    /// The function git's hunk header names above the line at the run's commit
    /// (`funcname_above`), written by the binary; the place rule reads it (draft name, the
    /// owner, 2026-10-15).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub function: Option<String>,
    /// Why the output points here [draft]; written by the binary, absent before it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<LocationKind>,
}

/// How a runner is read here [draft, lane a1, the adapter]: the choice a dry run made for one
/// (root, runner) at a commit, from the kinds of reading the binary has. Written by the binary only.
/// `watched` lists the files whose change owes a new dry run; `gaps` what the dry run could not
/// read (empty when nothing is missing); `ran_in` is laptop or pipeline when it says where.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReaderChosen {
    /// The dry run (a reserved run) that made the choice.
    pub run_id: String,
    pub root: String,
    pub runner: String,
    /// The reader's id, the only thing the adapter file names.
    pub reader: String,
    /// The command the choice is for, as claimed.
    pub command: String,
    pub commit: String,
    /// What the setup was, in the binary's words; may be left out.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub setup: Option<String>,
    /// The runner's and its plugins' versions the dry run saw; may be left out.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub versions: Option<String>,
    /// The Familiar version that chose.
    pub familiar: String,
    pub watched: Vec<String>,
    pub gaps: Vec<String>,
    pub why: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ran_in: Option<String>,
}
