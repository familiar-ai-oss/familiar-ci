//! Find the working directory a path sits in and whether git ignores a file
//! there, for `familiar-ai setup`. The ignore rules are read through `gix`,
//! the same ones `git check-ignore` reads.

use std::path::{Path, PathBuf};

use crate::{repo, GitError};

/// The working directory of the repository that holds `path`: `Err` when
/// `path` is not inside a repository, `Ok(None)` when the repository has no
/// working directory (bare).
pub fn worktree_root(path: &Path) -> Result<Option<PathBuf>, GitError> {
    Ok(repo::open(path)?.workdir().map(Path::to_path_buf))
}

/// Main's working directory when `path` is in a linked worktree; `None` in
/// main itself, or when main is bare. Reads only.
pub fn main_worktree_root(path: &Path) -> Result<Option<PathBuf>, GitError> {
    let repo = repo::open(path)?;
    if repo.kind() != gix::repository::Kind::LinkedWorkTree {
        return Ok(None);
    }
    let main = repo.main_repo().map_err(|e| GitError::Open { path: path.to_path_buf(), message: e.to_string() })?;
    Ok(main.workdir().map(Path::to_path_buf))
}

/// One checkout of a repository: its working directory, the branch it has
/// checked out (`None` when detached or unreadable), whether it is main's own
/// working directory, and the reason its worktree is locked with, when it is
/// (`git worktree lock --reason`; empty when locked without one).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Checkout {
    pub top: PathBuf,
    pub branch: Option<String>,
    pub is_main: bool,
    pub locked: Option<String>,
}

/// Every checkout of the repository that holds `path`: main first (unless
/// bare), then each linked worktree whose folder is there. Reads only.
pub fn checkouts(path: &Path) -> Result<Vec<Checkout>, GitError> {
    let open = |message: String| GitError::Open { path: path.to_path_buf(), message };
    let main = repo::open(path)?.main_repo().map_err(|e| open(e.to_string()))?;
    let branch_of = |r: &gix::Repository| r.head_name().ok().flatten().map(|n| n.shorten().to_string());
    let mut out: Vec<Checkout> =
        main.workdir().map(|top| Checkout { top: top.to_path_buf(), branch: branch_of(&main), is_main: true, locked: None }).into_iter().collect();
    for proxy in main.worktrees().map_err(|e| open(e.to_string()))? {
        let Ok(top) = proxy.base() else {
            continue;
        };
        if !top.is_dir() {
            continue;
        }
        let locked = proxy.lock_reason().map(|r| r.to_string());
        let branch = proxy.into_repo_with_possibly_inaccessible_worktree().ok().and_then(|r| branch_of(&r));
        out.push(Checkout { top, branch, is_main: false, locked });
    }
    Ok(out)
}

/// Whether git ignores the file at `rel_path` (relative to the working
/// directory, `/`-separated); the file need not exist yet.
pub fn is_ignored(repo_path: &Path, rel_path: &str) -> Result<bool, GitError> {
    let repo = repo::open(repo_path)?;
    let rules = |message: String| GitError::IgnoreRules {
        path: repo_path.to_path_buf(),
        message,
    };
    let index = repo.index_or_empty().map_err(|e| rules(e.to_string()))?;
    let mut stack = repo
        .excludes(&index, None, Default::default())
        .map_err(|e| rules(e.to_string()))?;
    let platform = stack
        .at_path(rel_path, Some(gix::index::entry::Mode::FILE))
        .map_err(|source| GitError::Io {
            op: "read the ignore rules for",
            path: repo_path.join(rel_path),
            source,
        })?;
    Ok(platform.is_excluded())
}
