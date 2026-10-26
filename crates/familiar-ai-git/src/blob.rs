//! Read a file's bytes at a commit and hash them: `lines_hash` for anchor
//! lines, `tests_hash` for a set of test files. Both hash content only, so a
//! rebase changes neither.

use std::collections::BTreeMap;
use std::path::Path;

use sha2::{Digest, Sha256};

use crate::repo;
use crate::GitError;

/// The bytes of a file at a commit; an error when the commit or the path
/// does not exist.
pub fn file_at(repo_path: &Path, commit: &str, path: &str) -> Result<Vec<u8>, GitError> {
    let repo = repo::open(repo_path)?;
    let commit_id = repo::resolve_commit(&repo, commit)?;
    repo::blob_at(&repo, commit_id, path)
}

/// Every file of `commit`'s tree, its path from the repository's root and its bytes, sorted by
/// path; symlinks and submodules left out. The place lister reads the planning commit with it.
pub fn blobs_at(repo_path: &Path, commit: &str) -> Result<BTreeMap<String, Vec<u8>>, GitError> {
    let repo = repo::open(repo_path)?;
    let id = repo::resolve_commit(&repo, commit)?;
    Ok(tree_blobs(&repo, id)?.into_iter().map(|(path, (bytes, _))| (path, bytes)).collect())
}

/// The SHA-256, lower-case hex, of lines `line_start..=line_end` (1-based,
/// inclusive, exact bytes including line endings) of a file at a commit.
pub fn lines_hash(
    repo_path: &Path,
    commit: &str,
    path: &str,
    line_start: u64,
    line_end: u64,
) -> Result<String, GitError> {
    let bytes = file_at(repo_path, commit, path)?;
    lines_hash_of(&bytes, line_start, line_end).ok_or(GitError::RangeOutside {
        path: path.to_string(),
        line_start,
        line_end,
        line_count: crate::lines::line_count(&bytes),
    })
}

/// [`lines_hash`] over bytes already in hand, such as a file in the working
/// tree, so the same lines hash the same wherever they were read. `None` when
/// the range is outside the bytes.
pub fn lines_hash_of(bytes: &[u8], line_start: u64, line_end: u64) -> Option<String> {
    let range = crate::lines::line_bytes(bytes, line_start, line_end)?;
    Some(hex(&Sha256::digest(&range)))
}

/// The SHA-256, lower-case hex, of a set of files at a commit. Paths are
/// sorted and deduplicated, each framed as `path \0 len-be content \0`, and a
/// file missing at that commit as `path \0 ABSENT \0`.
pub fn tests_hash(repo_path: &Path, commit: &str, paths: &[String]) -> Result<String, GitError> {
    let repo = repo::open(repo_path)?;
    let commit_id = repo::resolve_commit(&repo, commit)?;
    tests_hash_at_commit(&repo, commit_id, paths)
}

/// [`tests_hash`] on an already-opened repository and resolved commit.
pub(crate) fn tests_hash_at_commit(
    repo: &gix::Repository,
    commit: gix::ObjectId,
    paths: &[String],
) -> Result<String, GitError> {
    let mut sorted: Vec<&String> = paths.iter().collect();
    sorted.sort();
    sorted.dedup();
    let mut h = Sha256::new();
    for path in sorted {
        h.update(path.as_bytes());
        h.update([0u8]);
        match repo::try_blob_at(repo, commit, path)? {
            Some(bytes) => {
                h.update((bytes.len() as u64).to_be_bytes());
                h.update(&bytes);
            }
            None => h.update(b"ABSENT"),
        }
        h.update([0u8]);
    }
    Ok(hex(&h.finalize()))
}

/// Every blob in the tree of a commit: path -> (bytes, executable), sorted
/// by path.
pub(crate) fn tree_blobs(
    repo: &gix::Repository,
    commit: gix::ObjectId,
) -> Result<BTreeMap<String, (Vec<u8>, bool)>, GitError> {
    let spec = commit.to_string();
    let commit_obj = repo::commit(repo, commit)?;
    let tree = commit_obj.tree().map_err(|e| GitError::Object {
        spec: spec.clone(),
        message: e.to_string(),
    })?;
    let mut out = BTreeMap::new();
    walk_tree(&tree, "", &spec, &mut out)?;
    Ok(out)
}

fn walk_tree(
    tree: &gix::Tree<'_>,
    prefix: &str,
    spec: &str,
    out: &mut BTreeMap<String, (Vec<u8>, bool)>,
) -> Result<(), GitError> {
    use gix::bstr::ByteSlice;
    for entry in tree.iter() {
        let entry = entry.map_err(|e| GitError::Object {
            spec: spec.into(),
            message: e.to_string(),
        })?;
        let name = entry
            .filename()
            .to_str()
            .map_err(|_| GitError::NotUtf8Path)?;
        let path = if prefix.is_empty() {
            name.to_string()
        } else {
            format!("{prefix}/{name}")
        };
        if entry.mode().is_tree() {
            let sub = entry
                .object()
                .map_err(|e| GitError::Object {
                    spec: spec.into(),
                    message: e.to_string(),
                })?
                .into_tree();
            walk_tree(&sub, &path, spec, out)?;
        } else if entry.mode().is_blob() {
            let object = entry.object().map_err(|e| GitError::Object {
                spec: spec.into(),
                message: e.to_string(),
            })?;
            out.insert(path, (object.data.clone(), entry.mode().is_executable()));
        }
        // Symlinks and gitlinks are skipped.
    }
    Ok(())
}

pub(crate) fn hex(digest: &[u8]) -> String {
    let mut s = String::with_capacity(digest.len() * 2);
    for b in digest {
        s.push_str(&format!("{b:02x}"));
    }
    s
}
