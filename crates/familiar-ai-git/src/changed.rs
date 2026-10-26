//! The changed files of a commit, with line hunks: the `files` of
//! `commit_seen` and the input to [`crate::overlap`].

use std::path::Path;

use crate::repo;
use crate::{ChangedFile, GitError, Hunk};

/// The changed files of a commit against its first parent (a first commit:
/// against the empty tree), each with its line hunks.
pub fn changed(repo_path: &Path, commit: &str) -> Result<Vec<ChangedFile>, GitError> {
    let repo = repo::open(repo_path)?;
    let id = repo::resolve_commit(&repo, commit)?;
    changes_at(&repo, id)
}

/// The changed files from commit `from` to commit `to`, tree against tree, each with its line
/// hunks (old lines are `from`'s); sorted by path.
pub fn changed_between(repo_path: &Path, from: &str, to: &str) -> Result<Vec<ChangedFile>, GitError> {
    let repo = repo::open(repo_path)?;
    let (a, b) = (tree_of(&repo, from)?, tree_of(&repo, to)?);
    diff_trees(&a, &b)
}

/// The tree of the commit `spec` names.
fn tree_of<'r>(repo: &'r gix::Repository, spec: &str) -> Result<gix::Tree<'r>, GitError> {
    let id = repo::resolve_commit(repo, spec)?;
    repo::commit(repo, id)?.tree().map_err(|e| repo::obj_err(&id.to_string(), e))
}

/// The changed files of a resolved commit: its tree against its first
/// parent's, or against the empty tree on a first commit.
pub(crate) fn changes_at(
    repo: &gix::Repository,
    id: gix::ObjectId,
) -> Result<Vec<ChangedFile>, GitError> {
    let tree = repo::commit(repo, id)?
        .tree()
        .map_err(|e| repo::obj_err(&id.to_string(), e))?;
    let from = match repo::first_parent(repo, id)? {
        Some(p) => repo::commit(repo, p)?
            .tree()
            .map_err(|e| repo::obj_err(&p.to_string(), e))?,
        None => repo.empty_tree(),
    };
    diff_trees(&from, &tree)
}

/// The tree diff between two trees: changed files (sorted by path) with line
/// hunks; renames read as a deletion plus an addition.
fn diff_trees(from: &gix::Tree<'_>, to: &gix::Tree<'_>) -> Result<Vec<ChangedFile>, GitError> {
    let spec = format!("{}..{}", from.id, to.id);
    let mut out: Vec<ChangedFile> = Vec::new();
    let mut err: Option<GitError> = None;
    from.changes()
        .map_err(|e| obj_err(&spec, e))?
        .for_each_to_obtain_tree(to, |change| {
            if let Some(record) = blob_change(&change) {
                match hunks_of(record) {
                    Ok((path, hunks)) => out.push(ChangedFile { path, hunks }),
                    Err(e) => err = Some(e),
                }
            }
            Ok(std::ops::ControlFlow::<()>::Continue(()))
        })
        .map_err(|e| obj_err(&spec, e))?;
    if let Some(e) = err {
        return Err(e);
    }
    out.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(out)
}

/// A tree change that is a blob added, deleted, or modified becomes
/// `(path, old blob id, new blob id)`; anything else (mode-only change,
/// symlink, gitlink) becomes `None`.
fn blob_change<'a, 'old, 'new>(
    change: &gix::object::tree::diff::Change<'a, 'old, 'new>,
) -> Option<(String, Option<gix::Id<'old>>, Option<gix::Id<'new>>)> {
    use gix::object::tree::diff::Change;
    match change {
        Change::Addition {
            location,
            id,
            entry_mode,
            ..
        } if entry_mode.is_blob() => Some((location.to_string(), None, Some(*id))),
        Change::Deletion {
            location,
            id,
            entry_mode,
            ..
        } if entry_mode.is_blob() => Some((location.to_string(), Some(*id), None)),
        Change::Modification {
            location,
            previous_id,
            id,
            entry_mode,
            previous_entry_mode,
            ..
        } if entry_mode.is_blob() && previous_entry_mode.is_blob() => {
            Some((location.to_string(), Some(*previous_id), Some(*id)))
        }
        _ => None,
    }
}

/// The line hunks of one recorded change, between the two blob ids (an
/// absent side reads as empty).
fn hunks_of(
    (path, old_id, new_id): (String, Option<gix::Id<'_>>, Option<gix::Id<'_>>),
) -> Result<(String, Vec<Hunk>), GitError> {
    let read = |id: Option<gix::Id<'_>>| -> Result<Vec<u8>, GitError> {
        match id {
            None => Ok(Vec::new()),
            Some(id) => Ok(id
                .object()
                .map_err(|e| obj_err(&path, e))?
                .try_into_blob()
                .map_err(|_| obj_err(&path, "tree entry is not a blob"))?
                .data
                .clone()),
        }
    };
    let hunks = repo::line_hunks(&read(old_id)?, &read(new_id)?);
    Ok((path, hunks))
}

fn obj_err(spec: &str, e: impl std::fmt::Display) -> GitError {
    GitError::Object {
        spec: spec.to_string(),
        message: e.to_string(),
    }
}
