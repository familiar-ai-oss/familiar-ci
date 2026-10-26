//! Compute the field values of a `commit_seen` fact and the per-criterion
//! test hashes when a commit lands. The fact types live in `familiar_ai_facts`.

use std::collections::BTreeSet;
use std::path::Path;

use crate::range::{map_range, overlap};
use crate::{Anchor, ChangedFile, GitError, MappedRange};

/// The fields of a `commit_seen` fact for `commit`, given all the plan's test
/// files and the anchors as numbered at their planning `sha`. The test hashes
/// are not among them; [`observe_tests`] computes those, see
/// docs/decisions.md#one-stamp-per-criterion-per-run.
pub fn commit_seen(
    repo_path: &Path,
    commit: &str,
    plan_test_files: &[String],
    anchors: &[Anchor],
) -> Result<CommitSeenFields, GitError> {
    let repo = crate::repo::open(repo_path)?;
    let commit_id = crate::repo::resolve_commit(&repo, commit)?;
    let changes = crate::changed::changes_at(&repo, commit_id)?;

    let files: Vec<String> = changes.iter().map(|c| c.path.clone()).collect();
    let plan_set: BTreeSet<&str> = plan_test_files.iter().map(String::as_str).collect();
    let touches_tests = files.iter().any(|f| plan_set.contains(f.as_str()));
    let touches_code = files.iter().any(|f| !plan_set.contains(f.as_str()));

    let anchor_overlap = anchor_overlap(&repo, commit_id, &changes, anchors)?;

    Ok(CommitSeenFields {
        commit: commit_id.to_string(),
        files,
        touches_tests,
        touches_code,
        anchor_overlap,
    })
}

/// `tests_hash` over one criterion's files as they stand at this commit;
/// `None` when none of them exists there.
fn tests_hash_at(
    repo: &gix::Repository,
    commit: gix::ObjectId,
    active_test_files: &[String],
) -> Result<Option<String>, GitError> {
    let mut any = false;
    for f in active_test_files {
        if crate::repo::try_blob_at(repo, commit, f)?.is_some() {
            any = true;
            break;
        }
    }
    if any {
        Ok(Some(crate::blob::tests_hash_at_commit(
            repo,
            commit,
            active_test_files,
        )?))
    } else {
        Ok(None)
    }
}

/// For each `(criterion_id, files)` in order, [`crate::tests_hash`] over that
/// criterion's own files at `commit`, or `None` when none of them exists there.
/// The commit reader, CI's verify and tidy all call this one function so their
/// hashes agree.
pub fn observe_tests(
    repo_path: &Path,
    commit: &str,
    criteria: &[(String, Vec<String>)],
) -> Result<Vec<(String, Option<String>)>, GitError> {
    let repo = crate::repo::open(repo_path)?;
    let commit_id = crate::repo::resolve_commit(&repo, commit)?;
    let mut out = Vec::with_capacity(criteria.len());
    for (criterion_id, files) in criteria {
        let mut files = files.clone();
        files.sort();
        files.dedup();
        out.push((criterion_id.clone(), tests_hash_at(&repo, commit_id, &files)?));
    }
    Ok(out)
}

/// The sorted criterion ids whose anchor ranges this commit's hunks touch.
/// Each range is first mapped from its planning `sha` to the commit's first
/// parent, because earlier commits may have shifted the lines.
fn anchor_overlap(
    repo: &gix::Repository,
    commit: gix::ObjectId,
    changes: &[ChangedFile],
    anchors: &[Anchor],
) -> Result<Vec<String>, GitError> {
    let parent = crate::repo::first_parent(repo, commit)?;
    let mut overlap_ids: Vec<String> = Vec::new();
    for anchor in anchors {
        if anchor.line_start == 0 && anchor.line_end == 0 {
            // A whole-file anchor: any change to the file touches it.
            if changes.iter().any(|c| c.path == anchor.file_path) {
                overlap_ids.push(anchor.criterion_id.clone());
            }
            continue;
        }
        let base = crate::repo::resolve_commit(repo, &anchor.sha)?;
        let at_parent = match parent {
            Some(p) => {
                let hunks = file_line_diff(repo, base, p, &anchor.file_path)?;
                map_range(&hunks, anchor.line_start, anchor.line_end)
            }
            None => MappedRange::Range {
                line_start: anchor.line_start,
                line_end: anchor.line_end,
            },
        };
        if let MappedRange::Range {
            line_start,
            line_end,
        } = at_parent
        {
            let hunks: Vec<crate::Hunk> = changes
                .iter()
                .filter(|c| c.path == anchor.file_path)
                .flat_map(|c| c.hunks.clone())
                .collect();
            if overlap(&hunks, line_start, line_end) {
                overlap_ids.push(anchor.criterion_id.clone());
            }
        }
    }
    overlap_ids.sort();
    overlap_ids.dedup();
    Ok(overlap_ids)
}

/// The field values of a `commit_seen` fact, before the caller appends it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitSeenFields {
    /// The commit's own id, resolved.
    pub commit: String,
    /// Changed paths, sorted.
    pub files: Vec<String>,
    /// True when a changed file is one of the plan's test files.
    pub touches_tests: bool,
    /// True when a changed file is not one of the plan's test files.
    pub touches_code: bool,
    /// Sorted criterion ids whose anchor ranges this commit's hunks touch.
    pub anchor_overlap: Vec<String>,
}

impl From<CommitSeenFields> for familiar_ai_facts::CommitSeen {
    fn from(f: CommitSeenFields) -> Self {
        familiar_ai_facts::CommitSeen {
            commit: f.commit,
            files: f.files,
            touches_tests: f.touches_tests,
            touches_code: f.touches_code,
            anchor_overlap: f.anchor_overlap,
            // No longer written; `tests_observed` carries the test hashes.
            tests_hash: None,
            // Who made the commit is not read from git; the caller sets it.
            by: None,
        }
    }
}

/// The line diff of one file between two commits; a side where the file is
/// absent reads as empty.
pub(crate) fn file_line_diff(
    repo: &gix::Repository,
    from: gix::ObjectId,
    to: gix::ObjectId,
    path: &str,
) -> Result<Vec<crate::Hunk>, GitError> {
    let old = crate::repo::try_blob_at(repo, from, path)?.unwrap_or_default();
    let new = crate::repo::try_blob_at(repo, to, path)?.unwrap_or_default();
    Ok(crate::repo::line_hunks(&old, &new))
}
