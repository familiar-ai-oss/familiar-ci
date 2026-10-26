//! What proves a criterion: its test stack, planned and written tests, test results, fingerprints,
//! the commits that landed, what hosted CI said, and the mark of a record seeded from them.

use serde::{Deserialize, Serialize};

/// A test run's outcome; `did_not_run` means the tests never ran and counts as no result.
/// See docs/decisions.md#exit-codes-not-output
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Outcome {
    #[serde(rename = "passed")]
    Passed,
    #[serde(rename = "failed")]
    Failed,
    #[serde(rename = "did_not_run")]
    DidNotRun,
}

/// `ran_in: laptop | pipeline`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RanIn {
    Laptop,
    Pipeline,
}

/// `will: create | modify`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Will {
    Create,
    Modify,
}

/// A project root's test stack, announced by the agent and confirmed by the binary.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TestStackDetected {
    pub codebase_id: String,
    pub project_root: String,
    pub test_stack: String,
    /// The command that runs a criterion's tests.
    pub test_command: String,
}

/// A test the plan says will exist: create or modify.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TestPlanned {
    pub criterion_id: String,
    pub file_path: String,
    pub will: Will,
    /// The command that runs this criterion's tests, when it has its own; absent means the stack's.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub test_command: Option<String>,
}

/// A test added at the tests-first commit; it must be a planned one.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TestWritten {
    pub criterion_id: String,
    pub file_path: String,
    pub test_name: String,
}

/// An existing test that touches the criterion's anchor.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TouchingTestDeclared {
    pub criterion_id: String,
    pub file_path: String,
    pub test_name: String,
}

/// A run of a criterion's tests at a commit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PassPleasePass {
    pub criterion_id: String,
    pub file_path: String,
    /// Optional because results are read per planned test file, not per test.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub test_name: Option<String>,
    pub commit: String,
    pub outcome: Outcome,
    pub ran_in: RanIn,
    /// True marks the teeth run; absent means false.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub with_anchor_reverted: Option<bool>,
}

/// The tests' hash stamped at a commit, pinning the proof.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FingerprintStamped {
    pub criterion_id: String,
    pub commit: String,
    /// The tests' hash.
    pub proof_not_tampered: String,
    pub ran_in: RanIn,
}

/// The SHA-256 of one criterion's own test files at a landed commit, from the helper shared by the
/// laptop's commit reader and CI's verify. Not written when none of its files exists there.
/// See docs/decisions.md#one-stamp-per-criterion-per-run
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TestsObserved {
    pub criterion_id: String,
    /// The criterion's rev when observed, as opaque text.
    pub criterion_rev: String,
    pub commit: String,
    pub tests_hash: String,
}

/// Written when the agent announces it must change a sealed test.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TestChangeAnnounced {
    pub criterion_id: String,
    pub reason: String,
}

/// What the binary reads from git when a commit lands.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CommitSeen {
    pub commit: String,
    pub files: Vec<String>,
    pub touches_tests: bool,
    pub touches_code: bool,
    /// The criterion ids whose anchors the commit touches.
    pub anchor_overlap: Vec<String>,
    /// No longer written; old records carry it, and it is never read as proof.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tests_hash: Option<String>,
    /// `agent` when the commit landed during the agent's own Bash call; absent for the person's
    /// commits and unknown ones.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub by: Option<String>,
}

/// One entry of `commit_seen.existing_tests_changed` [draft name], a field of the fact and not of
/// the typed form, which the sealed tests build: the lines of test definitions that stood at the
/// plan's base and that the commit's tree no longer holds as they were, the standing change since
/// the base and not the commit's own diff. Absent when there is none.
///
/// One pre-existing test file's standing change [draft name]: the test definitions it is in and
/// the base's lines changed or removed, 1-based and inclusive.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExistingTestChange {
    pub file_path: String,
    pub tests: Vec<String>,
    pub lines: Vec<[u64; 2]>,
}

/// The agent's announcement that a pre-existing test file's lines change, with its reason
/// [draft name]; the judge reads the reason against the person's words.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExistingTestAnnounced {
    pub file_path: String,
    pub reason: String,
}

/// An existing test file the binary found referencing a file the fix changed [draft name]; run at
/// the head as a touching test. Written by the binary only.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TouchingTestDerived {
    pub criterion_id: String,
    pub file_path: String,
    /// The changed file it references.
    pub imports: String,
}

/// One site the place lister found [draft names]: the file, the line shown (1-based), the line
/// defining what it is in (empty when none) and the names of the place found there.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListedSite {
    pub file: String,
    pub line: u64,
    pub def: String,
    pub names: Vec<String>,
}

/// The sites that name what a part's planned place names, found by search at the planning commit
/// before any fix (the owner's plan of 2026-10-21, step 3) [draft name]. Written by the binary only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlacesListed {
    pub criterion_id: String,
    pub sites: Vec<ListedSite>,
}

/// The agent's word on a listed site [draft names]: the behavior asked for reaches it, or not.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlaceVerdict {
    Applies,
    NotAffected,
}

/// The agent's classification of one listed site [draft name]: `applies` adds a contract case of
/// kind `place`; `not_affected` needs its one-line reason, which the judge reads at the seal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlaceClassified {
    pub criterion_id: String,
    pub file: String,
    pub line: u64,
    pub verdict: PlaceVerdict,
    pub reason: String,
}

/// The action started on the pull request's head.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PipelineRunSeen {
    pub head_commit: String,
}

/// What a hosted CI run of `familiar-verify` said at a commit, as `familiar-ai ci fetch` read it.
/// Written by the binary alone, so `status` never touches the network.
/// See docs/decisions.md#ci-proves-the-laptop-guides
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CiReportSeen {
    /// The commit the run checked.
    pub sha: String,
    /// The workflow run's databaseId, as text.
    pub run_id: String,
    /// The run's finishing time, gh's `updatedAt`; not `at`, which append stamps.
    pub finished_at: String,
    /// One of `CI_OUTCOMES`.
    pub outcome: String,
    /// The report's per-criterion lines, in order; may be empty.
    pub report: Vec<String>,
}

/// The mark setup's seed writes last (the owner's name, 2026-10-15): every fact before it the
/// seed wrote, from the committed plan and verify's recheck. Written by the binary only.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecordSeeded {
    /// The HEAD the seed walked to.
    pub commit: String,
    /// How many plan facts the seed copied first.
    pub plan_lines: u64,
}
