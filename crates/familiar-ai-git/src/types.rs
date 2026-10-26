//! The types the public functions take and return.

/// A hunk of a line diff: old lines `old_start..old_start+old_count` were
/// replaced by new lines `new_start..new_start+new_count` (1-based starts).
/// A pure insertion (`old_count` 0) lands between old lines `old_start - 1`
/// and `old_start`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Hunk {
    pub old_start: u64,
    /// Number of old lines the hunk replaces (0 on a pure insertion).
    pub old_count: u64,
    pub new_start: u64,
    /// Number of new lines the hunk puts in their place.
    pub new_count: u64,
}

/// One file of a commit's diff against its first parent, with line hunks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangedFile {
    /// Relative to the repository root.
    pub path: String,
    /// The line hunks between the parent's version and this commit's.
    pub hunks: Vec<Hunk>,
}

/// An anchor as passed to [`crate::commit_seen`], with line numbers as the
/// file stood at commit `sha`. Lines 0..0 mean the whole file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Anchor {
    pub criterion_id: String,
    /// Relative to the repository root.
    pub file_path: String,
    /// First anchored line (1-based), as numbered at `sha`.
    pub line_start: u64,
    /// Last anchored line (inclusive), as numbered at `sha`.
    pub line_end: u64,
    /// The commit the anchor was taken at (planning time).
    pub sha: String,
}

/// How [`crate::map_range`] came out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MappedRange {
    /// The range as numbered in the new version (grew or shrank if the hunks
    /// touched its lines).
    Range { line_start: u64, line_end: u64 },
    /// Every line of the range was deleted.
    Deleted,
}
