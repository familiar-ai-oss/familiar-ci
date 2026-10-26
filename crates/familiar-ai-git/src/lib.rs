//! The git reads behind the record's facts and the teeth run's scratch copy,
//! all through `gix`, see docs/decisions.md#git-reads-in-pure-rust-two-writes-through-git.

mod blob;
mod changed;
mod error;
mod facts;
mod ignore;
mod lines;
mod range;
mod repo;
mod scratch;
mod types;

pub use blob::{blobs_at, file_at, lines_hash, lines_hash_of, tests_hash};
pub use changed::{changed, changed_between};
pub use error::GitError;
pub use facts::{commit_seen, observe_tests, CommitSeenFields};
pub use ignore::{checkouts, is_ignored, main_worktree_root, worktree_root, Checkout};
pub use lines::{funcname_above, line_bytes, line_count, split_lines};
pub use range::{map_range, overlap};
pub use repo::{commits_between, head_commit, line_hunks, upstream, Upstream};
pub use scratch::{all_files, file_put_back, files_in, lines_put_back, root_commit, scratch_at, scratch_with_lines_put_back, top_level_names};
pub use types::{Anchor, ChangedFile, Hunk, MappedRange};

use std::path::Path as RepoPath;

/// The oldest commit of the range (as [`commits_between`]) whose `paths`
/// hash to `tests_hash`, or `None`. The tests-first commit is found by content
/// so that a rebase does not lose it.
pub fn find_by_tests_hash(
    repo_path: &RepoPath,
    from: &str,
    to: &str,
    paths: &[String],
    tests_hash: &str,
) -> Result<Option<String>, GitError> {
    let repo = repo::open(repo_path)?;
    for id in commits_between(repo_path, from, to)? {
        let oid = gix::ObjectId::from_hex(id.as_bytes()).map_err(|e| GitError::Parse {
            spec: id.clone(),
            message: e.to_string(),
        })?;
        if blob::tests_hash_at_commit(&repo, oid, paths)? == tests_hash {
            return Ok(Some(id));
        }
    }
    Ok(None)
}
