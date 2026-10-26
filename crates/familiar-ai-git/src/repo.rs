//! Repository plumbing over `gix`: open a repository, resolve commits, read
//! blobs, diff lines and walk history.

use std::path::Path;

use crate::{GitError, Hunk};

/// Open the repository that holds `path`, searching upward, since a project
/// can be a folder inside a repository.
pub(crate) fn open(path: &Path) -> Result<gix::Repository, GitError> {
    gix::discover(path).map_err(|source| GitError::Open {
        path: path.to_path_buf(),
        message: source.to_string(),
    })
}

/// The commit the repository's head points at, or `None` when the
/// repository has no commit yet.
pub fn head_commit(repo_path: &Path) -> Result<Option<String>, GitError> {
    let repo = open(repo_path)?;
    let head = repo.head().map_err(|e| obj_err("HEAD", e))?;
    Ok(head.id().map(|id| id.detach().to_string()))
}

/// Resolve `spec` (a commit id, short id, or ref) to a commit's object id.
pub(crate) fn resolve_commit(
    repo: &gix::Repository,
    spec: &str,
) -> Result<gix::ObjectId, GitError> {
    let id = repo.rev_parse_single(spec).map_err(|e| GitError::Parse {
        spec: spec.to_string(),
        message: e.to_string(),
    })?;
    let object = id.object().map_err(|e| GitError::Object {
        spec: spec.to_string(),
        message: e.to_string(),
    })?;
    object.try_into_commit().map_err(|_| GitError::NotACommit {
        spec: spec.to_string(),
    })?;
    Ok(id.detach())
}

pub(crate) fn commit<'r>(
    repo: &'r gix::Repository,
    id: gix::ObjectId,
) -> Result<gix::Commit<'r>, GitError> {
    repo.find_object(id)
        .map_err(|e| GitError::Object {
            spec: id.to_string(),
            message: e.to_string(),
        })?
        .try_into_commit()
        .map_err(|_| GitError::NotACommit {
            spec: id.to_string(),
        })
}

/// The id of `commit`'s first parent, or `None` on a first commit.
pub(crate) fn first_parent(
    repo: &gix::Repository,
    id: gix::ObjectId,
) -> Result<Option<gix::ObjectId>, GitError> {
    Ok(commit(repo, id)?.parent_ids().next().map(|p| p.detach()))
}

/// The bytes of `path` in the tree of `commit`, or an error naming both.
pub(crate) fn blob_at(
    repo: &gix::Repository,
    commit: gix::ObjectId,
    path: &str,
) -> Result<Vec<u8>, GitError> {
    try_blob_at(repo, commit, path)?.ok_or(GitError::FileMissing {
        commit: commit.to_string(),
        path: path.to_string(),
    })
}

/// The bytes of `path` in the tree of `commit`, `None` when absent.
pub(crate) fn try_blob_at(
    repo: &gix::Repository,
    commit_id: gix::ObjectId,
    path: &str,
) -> Result<Option<Vec<u8>>, GitError> {
    let spec = commit_id.to_string();
    let tree = commit(repo, commit_id)?
        .tree()
        .map_err(|e| obj_err(&spec, e))?;
    let entry = tree
        .lookup_entry_by_path(path)
        .map_err(|e| obj_err(&spec, e))?;
    match entry {
        None => Ok(None),
        Some(e) => {
            let object = e.object().map_err(|e| obj_err(&spec, e))?;
            let blob = object.try_into_blob().map_err(|_| GitError::FileMissing {
                commit: spec,
                path: path.to_string(),
            })?;
            Ok(Some(blob.data.clone()))
        }
    }
}

pub(crate) fn obj_err(spec: &str, e: impl std::fmt::Display) -> GitError {
    GitError::Object {
        spec: spec.to_string(),
        message: e.to_string(),
    }
}

/// The 1-based line hunks from `old` to `new`. An end-of-file token is
/// appended to both sides so that a change at the very end of the file lands
/// after the last line, as `git diff` reports it, not before it.
pub fn line_hunks(old: &[u8], new: &[u8]) -> Vec<Hunk> {
    const EOF: &[u8] = &[0u8];
    let mut o = old.to_vec();
    o.extend_from_slice(EOF);
    let mut n = new.to_vec();
    n.extend_from_slice(EOF);
    let input = gix::diff::blob::InternedInput::new(o.as_slice(), n.as_slice());
    let diff =
        gix::diff::blob::diff_with_slider_heuristics(gix::diff::blob::Algorithm::Myers, &input);
    diff.hunks()
        .map(|h| Hunk {
            old_start: u64::from(h.before.start) + 1,
            old_count: u64::from(h.before.end - h.before.start),
            new_start: u64::from(h.after.start) + 1,
            new_count: u64::from(h.after.end - h.after.start),
        })
        .collect()
}

/// The commits reachable from `to` and not from `from`, oldest first.
pub fn commits_between(repo_path: &Path, from: &str, to: &str) -> Result<Vec<String>, GitError> {
    let repo = open(repo_path)?;
    let from_id = resolve_commit(&repo, from)?;
    let to_id = resolve_commit(&repo, to)?;
    let mut ids: Vec<String> = Vec::new();
    let walk = repo
        .rev_walk([to_id])
        .with_hidden([from_id])
        .all()
        .map_err(|e| obj_err(to, e))?;
    for info in walk {
        let info = info.map_err(|e| obj_err(to, e))?;
        ids.push(info.id.to_string());
    }
    ids.reverse();
    Ok(ids)
}

/// The branch the head is on and how many commits it is ahead of the branch
/// it tracks. `ahead` is `None` when the branch tracks nothing or the tracked
/// branch has not been fetched.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Upstream {
    pub branch: String,
    pub ahead: Option<usize>,
}

/// The head's branch and how far it is ahead of its upstream; `None` when
/// the head is detached or the repository has no commit yet.
pub fn upstream(repo_path: &Path) -> Result<Option<Upstream>, GitError> {
    let repo = open(repo_path)?;
    let Some(name) = repo.head_name().map_err(|e| obj_err("HEAD", e))? else {
        return Ok(None);
    };
    let Ok(head) = repo.head_id() else {
        return Ok(None);
    };
    let branch = name.shorten().to_string();
    let tracking = match repo.branch_remote_tracking_ref_name(name.as_ref(), gix::remote::Direction::Fetch) {
        Some(Ok(t)) => t,
        _ => return Ok(Some(Upstream { branch, ahead: None })),
    };
    let Ok(mut tracked) = repo.find_reference(tracking.as_ref()) else {
        return Ok(Some(Upstream { branch, ahead: None }));
    };
    let up = tracked.peel_to_id().map_err(|e| obj_err("upstream", e))?.detach();
    let walk = repo
        .rev_walk([head.detach()])
        .with_hidden([up])
        .all()
        .map_err(|e| obj_err("HEAD", e))?;
    let mut ahead = 0;
    for info in walk {
        info.map_err(|e| obj_err("HEAD", e))?;
        ahead += 1;
    }
    Ok(Some(Upstream { branch, ahead: Some(ahead) }))
}
