//! Write a commit's tree to an empty folder as a real checkout, optionally with
//! one file's lines put back to an earlier commit, for the teeth run. The copy
//! borrows the repository's objects, so git inside it answers for the commit,
//! and the repository and its working tree are never changed.

use std::path::Path;

use crate::GitError;

/// Write commit `tip` into the empty folder `dir`, then in that copy put
/// `file`'s lines `line_start..=line_end`, numbered at `earlier`, back to how
/// they were at `earlier` (see [`lines_put_back`]). Errors when the file does
/// not exist at either commit.
pub fn scratch_with_lines_put_back(
    repo_path: &Path,
    dir: &Path,
    tip: &str,
    earlier: &str,
    file: &str,
    line_start: u64,
    line_end: u64,
) -> Result<(), GitError> {
    let repo = crate::repo::open(repo_path)?;
    let tip_id = crate::repo::resolve_commit(&repo, tip)?;
    let earlier_id = crate::repo::resolve_commit(&repo, earlier)?;

    let earlier_bytes = crate::repo::blob_at(&repo, earlier_id, file)?;
    let tip_bytes = crate::repo::blob_at(&repo, tip_id, file)?;

    let hunks = crate::facts::file_line_diff(&repo, earlier_id, tip_id, file)?;
    let rebuilt = lines_put_back(
        &earlier_bytes,
        &tip_bytes,
        &hunks,
        file,
        line_start,
        line_end,
    )?;

    scratch_at(repo_path, dir, tip)?;
    let dest = dir.join(file);
    std::fs::write(&dest, rebuilt).map_err(|e| GitError::Io {
        op: "write",
        path: dest,
        source: e,
    })
}

/// Put a whole-file anchor's file back into the copy at `dir` as it was at
/// `sha`, or remove it when it did not exist there, see
/// docs/decisions.md#new-work-names-its-integration-point.
pub fn file_put_back(repo_path: &Path, dir: &Path, sha: &str, path: &str) -> Result<(), GitError> {
    let target = dir.join(path);
    let io = |op: &'static str, e: std::io::Error| GitError::Io { op, path: target.clone(), source: e };
    match crate::blob::file_at(repo_path, sha, path) {
        Ok(bytes) => {
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent).map_err(|e| io("create the folder of", e))?;
            }
            std::fs::write(&target, bytes).map_err(|e| io("write", e))
        }
        Err(GitError::FileMissing { .. }) => match std::fs::remove_file(&target) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(io("remove", e)),
            _ => Ok(()),
        },
        Err(e) => Err(e),
    }
}

/// Write the files of `commit` into the empty folder `dir`, keeping executable
/// bits, and make `dir` a checkout of the commit: a detached `HEAD`, the
/// repository's objects shared and an index equal to its tree.
pub fn scratch_at(repo_path: &Path, dir: &Path, commit: &str) -> Result<(), GitError> {
    let repo = crate::repo::open(repo_path)?;
    let id = crate::repo::resolve_commit(&repo, commit)?;
    let io = |op: &'static str, path: &Path, e: std::io::Error| GitError::Io {
        op,
        path: path.to_path_buf(),
        source: e,
    };
    std::fs::create_dir_all(dir).map_err(|e| io("create", dir, e))?;
    for (path, (bytes, executable)) in crate::blob::tree_blobs(&repo, id)? {
        let dest = dir.join(&path);
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent).map_err(|e| io("create", parent, e))?;
        }
        std::fs::write(&dest, bytes).map_err(|e| io("write", &dest, e))?;
        if executable {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&dest, std::fs::Permissions::from_mode(0o755))
                .map_err(|e| io("write", &dest, e))?;
        }
    }
    write_git_dir(&repo, dir, id)
}

/// `dir/.git`: a detached `HEAD` at `id`, the source repository's objects
/// shared through `objects/info/alternates` (nothing is copied, and nothing
/// git writes later reaches the source), a `shallow` list when the source has
/// one, and an index equal to `id`'s tree with each file's stat as written.
fn write_git_dir(repo: &gix::Repository, dir: &Path, id: gix::ObjectId) -> Result<(), GitError> {
    let io = |op: &'static str, path: &Path, e: std::io::Error| GitError::Io {
        op,
        path: path.to_path_buf(),
        source: e,
    };
    let git = dir.join(".git");
    for sub in ["objects/info", "refs/heads", "refs/tags"] {
        let p = git.join(sub);
        std::fs::create_dir_all(&p).map_err(|e| io("create", &p, e))?;
    }
    let put = |name: &str, text: String| -> Result<(), GitError> {
        let p = git.join(name);
        std::fs::write(&p, text).map_err(|e| io("write", &p, e))
    };
    put("HEAD", format!("{id}\n"))?;
    put(
        "config",
        "[core]\n\trepositoryformatversion = 0\n\tfilemode = true\n\tbare = false\n\tlogallrefupdates = false\n".into(),
    )?;
    let common = repo.common_dir();
    let objects = std::fs::canonicalize(common.join("objects")).map_err(|e| io("read", common, e))?;
    put("objects/info/alternates", format!("{}\n", objects.display()))?;
    if let Ok(shallow) = std::fs::read(common.join("shallow")) {
        let p = git.join("shallow");
        std::fs::write(&p, shallow).map_err(|e| io("write", &p, e))?;
    }

    let tree = crate::repo::commit(repo, id)?
        .tree_id()
        .map_err(|e| GitError::Object { spec: id.to_string(), message: e.to_string() })?;
    let mut index = repo
        .index_from_tree(&tree)
        .map_err(|e| GitError::Object { spec: id.to_string(), message: format!("{e:?}") })?;
    for (entry, path) in index.entries_mut_with_paths() {
        let on_disk = dir.join(gix::path::from_bstr(path));
        if let Ok(meta) = gix::index::fs::Metadata::from_path_no_follow(&on_disk) {
            if let Ok(stat) = gix::index::entry::Stat::from_fs(&meta) {
                entry.stat = stat;
            }
        }
    }
    let p = git.join("index");
    let out = std::fs::File::create(&p).map_err(|e| io("create", &p, e))?;
    index
        .write_to(std::io::BufWriter::new(out), Default::default())
        .map_err(|e| GitError::Object { spec: id.to_string(), message: e.to_string() })?;
    Ok(())
}

/// The names at the top of `commit`'s tree, which tell what the repository
/// tracks apart from what the project's setup put beside it.
pub fn top_level_names(repo_path: &Path, commit: &str) -> Result<Vec<String>, GitError> {
    let repo = crate::repo::open(repo_path)?;
    let id = crate::repo::resolve_commit(&repo, commit)?;
    let spec = id.to_string();
    let obj = |e: &dyn std::fmt::Display| GitError::Object {
        spec: spec.clone(),
        message: e.to_string(),
    };
    let tree = crate::repo::commit(&repo, id)?.tree().map_err(|e| obj(&e))?;
    let mut out = Vec::new();
    for entry in tree.iter() {
        let entry = entry.map_err(|e| obj(&e))?;
        out.push(entry.filename().to_string());
    }
    Ok(out)
}

/// The files directly inside the folder `dir` of `commit`'s tree, sorted;
/// empty when the folder is absent. Subfolders are left out.
pub fn files_in(repo_path: &Path, commit: &str, dir: &str) -> Result<Vec<String>, GitError> {
    let repo = crate::repo::open(repo_path)?;
    let id = crate::repo::resolve_commit(&repo, commit)?;
    let spec = id.to_string();
    let obj = |e: &dyn std::fmt::Display| GitError::Object {
        spec: spec.clone(),
        message: e.to_string(),
    };
    let tree = crate::repo::commit(&repo, id)?.tree().map_err(|e| obj(&e))?;
    let Some(entry) = tree.lookup_entry_by_path(dir).map_err(|e| obj(&e))? else {
        return Ok(Vec::new());
    };
    if !entry.mode().is_tree() {
        return Ok(Vec::new());
    }
    let folder = entry.object().map_err(|e| obj(&e))?.into_tree();
    let mut out = Vec::new();
    for e in folder.iter() {
        let e = e.map_err(|e| obj(&e))?;
        if e.mode().is_blob() {
            out.push(e.filename().to_string());
        }
    }
    out.sort();
    Ok(out)
}

/// Every file of `commit`'s tree, its path from the root, sorted; folders named in `skip`
/// (such as `node_modules`) are not entered.
pub fn all_files(repo_path: &Path, commit: &str, skip: &[&str]) -> Result<Vec<String>, GitError> {
    let repo = crate::repo::open(repo_path)?;
    let id = crate::repo::resolve_commit(&repo, commit)?;
    let spec = id.to_string();
    let obj = |e: &dyn std::fmt::Display| GitError::Object { spec: spec.clone(), message: e.to_string() };
    let root = crate::repo::commit(&repo, id)?.tree().map_err(|e| obj(&e))?;
    let mut out = Vec::new();
    let mut todo: Vec<(String, gix::Tree<'_>)> = vec![(String::new(), root)];
    while let Some((at, tree)) = todo.pop() {
        for e in tree.iter() {
            let e = e.map_err(|e| obj(&e))?;
            let name = e.filename().to_string();
            let path = if at.is_empty() { name.clone() } else { format!("{at}/{name}") };
            if e.mode().is_tree() {
                if !skip.contains(&name.as_str()) {
                    let sub = e.object().map_err(|e| obj(&e))?.into_tree();
                    todo.push((path, sub));
                }
            } else if e.mode().is_blob() {
                out.push(path);
            }
        }
    }
    out.sort();
    Ok(out)
}

/// The repository's first commit reached from `commit` by first parents: the
/// same in every full clone, so it names the repository.
pub fn root_commit(repo_path: &Path, commit: &str) -> Result<String, GitError> {
    let repo = crate::repo::open(repo_path)?;
    let mut id = crate::repo::resolve_commit(&repo, commit)?;
    while let Some(parent) = crate::repo::first_parent(&repo, id)? {
        id = parent;
    }
    Ok(id.to_string())
}

/// The tip version of one file with the range `line_start..=line_end`,
/// numbered in `earlier`, put back; `hunks` is the line diff from `earlier`
/// to `tip` in file order, and `path` only names the file in an error.
///
/// Every hunk that touches the range (the rule of [`crate::overlap`]) is undone
/// whole, even where it reaches outside the range, because a half-undone hunk
/// leaves a file that is neither version.
pub fn lines_put_back(
    earlier: &[u8],
    tip: &[u8],
    hunks: &[crate::Hunk],
    path: &str,
    line_start: u64,
    line_end: u64,
) -> Result<Vec<u8>, GitError> {
    let earlier_lines = crate::lines::split_lines(earlier);
    let tip_lines = crate::lines::split_lines(tip);
    let outside = |line_start: u64, line_end: u64, line_count: usize| GitError::RangeOutside {
        path: path.to_string(),
        line_start,
        line_end,
        line_count: line_count as u64,
    };
    if line_start == 0 || line_end < line_start || line_end > earlier_lines.len() as u64 {
        return Err(outside(line_start, line_end, earlier_lines.len()));
    }
    // Lines `from..from+count` (1-based) of one version, or `None` when the
    // hunks do not fit the text.
    let slice = |lines: &[&[u8]], from: u64, count: u64| -> Option<Vec<u8>> {
        let start = usize::try_from(from.checked_sub(1)?).ok()?;
        let end = start.checked_add(usize::try_from(count).ok()?)?;
        Some(lines.get(start..end)?.concat())
    };
    let mut rebuilt = Vec::new();
    let mut next_tip: u64 = 1; // the first tip line not yet written
    for h in hunks {
        let kept = h
            .new_start
            .checked_sub(next_tip)
            .and_then(|n| slice(&tip_lines, next_tip, n));
        rebuilt.extend(kept.ok_or_else(|| outside(h.new_start, h.new_start, tip_lines.len()))?);
        let lines = if crate::range::touches(h, line_start, line_end) {
            slice(&earlier_lines, h.old_start, h.old_count)
        } else {
            slice(&tip_lines, h.new_start, h.new_count)
        };
        rebuilt
            .extend(lines.ok_or_else(|| outside(h.old_start, h.old_start, earlier_lines.len()))?);
        next_tip = h.new_start + h.new_count;
    }
    let rest = (tip_lines.len() as u64 + 1)
        .checked_sub(next_tip)
        .and_then(|n| slice(&tip_lines, next_tip, n));
    rebuilt.extend(rest.ok_or_else(|| outside(next_tip, next_tip, tip_lines.len()))?);
    Ok(rebuilt)
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::process::Command;

    fn git(dir: &Path, args: &[&str]) -> String {
        let out = Command::new("git")
            .current_dir(dir)
            .args(["-c", "user.name=t", "-c", "user.email=t@example.com", "-c", "commit.gpgsign=false"])
            .args(args)
            .output()
            .expect("run git");
        assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    }

    /// A repository of three commits; the first commit holds a script, the
    /// second changes `a.txt`, the third adds `c.txt`.
    fn repo(name: &str) -> (PathBuf, [String; 3]) {
        let dir = std::env::temp_dir().join(format!("familiar-git-scratch-unit-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        git(&dir, &["init", "-q", "-b", "main"]);
        let mut ids = Vec::new();
        for (file, text) in [("a.txt", "one\n"), ("a.txt", "two\n"), ("c.txt", "three\n")] {
            std::fs::write(dir.join(file), text).unwrap();
            git(&dir, &["add", "-A"]);
            git(&dir, &["commit", "-q", "-m", text]);
            ids.push(git(&dir, &["rev-parse", "HEAD"]));
        }
        (dir, [ids[0].clone(), ids[1].clone(), ids[2].clone()])
    }

    fn fresh(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("familiar-git-scratch-copy-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn the_copy_is_a_checkout_git_resolves_head_and_every_commit_in() {
        let (repo, [c1, c2, c3]) = repo("checkout");
        let dir = fresh("checkout");
        super::scratch_at(&repo, &dir, &c2).unwrap();
        assert_eq!(git(&dir, &["rev-parse", "HEAD"]), c2, "HEAD is the commit, not the repository's tip");
        assert_eq!(git(&dir, &["diff", "--name-only", &c1, "HEAD"]), "a.txt", "an earlier commit is reachable");
        assert_eq!(git(&dir, &["cat-file", "-t", &c3]), "commit", "so is a later one: the objects are the repository's");
        assert_eq!(git(&dir, &["rev-list", "--count", "HEAD"]), "2");
        assert_eq!(git(&dir, &["status", "--porcelain"]), "", "the files are the commit's: nothing changed, nothing missing");
        assert_eq!(git(&dir, &["diff", "HEAD", "--stat"]), "");
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_dir_all(&repo);
    }

    #[test]
    fn nothing_git_writes_in_the_copy_reaches_the_repository() {
        let (repo, [_, c2, c3]) = repo("isolated");
        let dir = fresh("isolated");
        super::scratch_at(&repo, &dir, &c2).unwrap();
        std::fs::write(dir.join("new.txt"), "x\n").unwrap();
        git(&dir, &["add", "-A"]);
        git(&dir, &["commit", "-q", "-m", "in the copy"]);
        assert_eq!(git(&repo, &["rev-parse", "HEAD"]), c3, "the repository's head did not move");
        assert_eq!(git(&repo, &["status", "--porcelain"]), "");
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_dir_all(&repo);
    }

    #[test]
    fn a_linked_worktree_as_the_source_works_too() {
        let (repo, [c1, c2, _]) = repo("linked");
        let linked = fresh("linked-src");
        git(&repo, &["worktree", "add", "-q", "--detach", linked.to_str().unwrap(), &c2]);
        let dir = fresh("linked");
        super::scratch_at(&linked, &dir, &c1).unwrap();
        assert_eq!(git(&dir, &["rev-parse", "HEAD"]), c1);
        assert_eq!(git(&dir, &["diff", "--name-only", &c1, &c2]), "a.txt");
        assert_eq!(git(&dir, &["status", "--porcelain"]), "");
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_dir_all(&linked);
        let _ = std::fs::remove_dir_all(&repo);
    }

    #[test]
    fn a_line_put_back_shows_as_the_one_change_git_sees() {
        let (repo, [c1, _, c3]) = repo("putback");
        let dir = fresh("putback");
        super::scratch_with_lines_put_back(&repo, &dir, &c3, &c1, "a.txt", 1, 1).unwrap();
        assert_eq!(git(&dir, &["rev-parse", "HEAD"]), c3);
        assert_eq!(git(&dir, &["status", "--porcelain"]), "M a.txt");
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_dir_all(&repo);
    }
}
