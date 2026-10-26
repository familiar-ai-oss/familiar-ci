//! The crate's one error type, returned by every fallible function.

use std::fmt;
use std::path::PathBuf;

/// An error from reading the repository or writing a scratch copy.
#[derive(Debug)]
pub enum GitError {
    /// The repository at `path` could not be opened.
    Open {
        path: PathBuf,
        message: String,
    },
    /// A commit spec (id, short id, or ref) could not be resolved.
    Parse {
        spec: String,
        message: String,
    },
    /// The spec resolved, but not to a commit.
    NotACommit {
        spec: String,
    },
    /// A git object could not be read or had the wrong kind.
    Object {
        spec: String,
        message: String,
    },
    /// No file `path` exists in the tree of `commit`.
    FileMissing {
        commit: String,
        path: String,
    },
    /// A 1-based line range lies outside the file it names.
    RangeOutside {
        path: String,
        line_start: u64,
        line_end: u64,
        line_count: u64,
    },
    /// A changed path in the repository is not UTF-8.
    NotUtf8Path,
    /// Git's ignore rules could not be assembled for the repository.
    IgnoreRules {
        path: PathBuf,
        message: String,
    },
    /// A filesystem operation failed.
    Io {
        op: &'static str,
        path: PathBuf,
        source: std::io::Error,
    },
}

impl fmt::Display for GitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GitError::Open { path, message } => {
                write!(f, "cannot open repository {}: {message}", path.display())
            }
            GitError::Parse { spec, message } => {
                write!(f, "cannot resolve commit '{spec}': {message}")
            }
            GitError::NotACommit { spec } => write!(f, "'{spec}' does not name a commit"),
            GitError::Object { spec, message } => write!(f, "object error at '{spec}': {message}"),
            GitError::FileMissing { commit, path } => {
                write!(f, "no file '{path}' at commit {commit}")
            }
            GitError::RangeOutside {
                path,
                line_start,
                line_end,
                line_count,
            } => write!(
                f,
                "lines {line_start}..={line_end} are outside '{path}', which has {line_count} lines"
            ),
            GitError::NotUtf8Path => write!(f, "a changed path is not UTF-8"),
            GitError::IgnoreRules { path, message } => {
                write!(f, "cannot read git's ignore rules in {}: {message}", path.display())
            }
            GitError::Io { op, path, source } => {
                write!(f, "cannot {op} {}: {source}", path.display())
            }
        }
    }
}

impl std::error::Error for GitError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            GitError::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}
