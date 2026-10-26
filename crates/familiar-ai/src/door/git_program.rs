//! Run the git program for the binary's two writes, `setup`'s commit and
//! `tidy`'s rewrite, which need the person's hooks and signing that gix's
//! write side lacks. Every read still goes through gix. See
//! docs/decisions.md#git-reads-in-pure-rust-two-writes-through-git

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::door::child::{self, Ended};

pub const GIT_WAIT: Duration = Duration::from_secs(120);

/// Run `git <args>` in `dir` with `set` in its environment and `stdin` fed to
/// it, returning stdout on exit 0, else a one-line error.
pub fn git(dir: &Path, args: &[&str], set: &[(&str, &str)], stdin: &str) -> Result<String, String> {
    let argv: Vec<String> = args.iter().map(|a| a.to_string()).collect();
    let ran = child::run("git", &argv, dir, set, stdin, GIT_WAIT);
    match ran.ended {
        Ended::Exited(Some(0)) => Ok(ran.stdout),
        _ => {
            let why = ran.stderr.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("").to_string();
            Err(format!("`git {}` failed ({}){}", args.join(" "), ran.how(), if why.is_empty() { String::new() } else { format!(": {why}") }))
        }
    }
}

/// The exit code of `git <args>`, or `None` when it did not exit by itself.
pub fn git_code(dir: &Path, args: &[&str]) -> Option<i32> {
    let argv: Vec<String> = args.iter().map(|a| a.to_string()).collect();
    match child::run("git", &argv, dir, &[], "", GIT_WAIT).ended {
        Ended::Exited(code) => code,
        _ => None,
    }
}

/// The trimmed stdout of `git <args>`.
pub fn git_line(dir: &Path, args: &[&str]) -> Result<String, String> {
    git(dir, args, &[], "").map(|s| s.trim().to_string())
}

/// One commit as `git cat-file commit` shows it, which a rewrite copies.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitInfo {
    pub id: String,
    pub tree: String,
    pub parents: Vec<String>,
    /// `Name <email> <seconds> <zone>`.
    pub author: String,
    pub committer: String,
    /// The message, byte for byte.
    pub message: String,
}

impl CommitInfo {
    /// The message's first line.
    pub fn subject(&self) -> &str {
        self.message.lines().next().unwrap_or("").trim()
    }
}

pub fn commit_info(dir: &Path, id: &str) -> Result<CommitInfo, String> {
    let raw = git(dir, &["cat-file", "commit", id], &[], "")?;
    let (head, message) = raw.split_once("\n\n").unwrap_or((raw.as_str(), ""));
    let mut info = CommitInfo { id: id.to_string(), tree: String::new(), parents: Vec::new(), author: String::new(), committer: String::new(), message: message.to_string() };
    for line in head.lines() {
        match line.split_once(' ') {
            Some(("tree", v)) => info.tree = v.to_string(),
            Some(("parent", v)) => info.parents.push(v.to_string()),
            Some(("author", v)) => info.author = v.to_string(),
            Some(("committer", v)) => info.committer = v.to_string(),
            _ => {}
        }
    }
    if info.tree.is_empty() {
        return Err(format!("{id} is not a commit git can read"));
    }
    Ok(info)
}

/// Split `Name <email> <seconds> <zone>` into name, email and the date git
/// accepts back.
fn ident(line: &str) -> Option<(String, String, String)> {
    let open = line.find('<')?;
    let close = line[open..].find('>')? + open;
    let name = line[..open].trim().to_string();
    let email = line[open + 1..close].to_string();
    let date = line[close + 1..].trim().to_string();
    Some((name, email, date))
}

/// The sorted paths a commit changed against its first parent, a rename
/// counting as a delete and an add.
pub fn changed_paths(dir: &Path, c: &CommitInfo) -> Result<Vec<String>, String> {
    let Some(parent) = c.parents.first() else {
        return Err(format!("{} has no parent", short(&c.id)));
    };
    let out = git(dir, &["diff-tree", "-r", "-z", "--no-renames", "--name-only", parent.as_str(), c.id.as_str()], &[], "")?;
    let mut paths: Vec<String> = out.split('\0').filter(|p| !p.is_empty()).map(str::to_string).collect();
    paths.sort();
    paths.dedup();
    Ok(paths)
}

/// The `mode blob-id` of each of `paths` in `commit`'s tree; a path the
/// commit lacks is absent from the map.
pub fn entries(dir: &Path, commit: &str, paths: &[String]) -> Result<BTreeMap<String, String>, String> {
    let mut out = BTreeMap::new();
    if paths.is_empty() {
        return Ok(out);
    }
    let mut args: Vec<&str> = vec!["--literal-pathspecs", "ls-tree", "-r", "-z", "--full-tree", commit, "--"];
    args.extend(paths.iter().map(String::as_str));
    for entry in git(dir, &args, &[], "")?.split('\0').filter(|e| !e.is_empty()) {
        // <mode> SP <type> SP <object> TAB <path>
        let Some((meta, path)) = entry.split_once('\t') else { continue };
        let parts: Vec<&str> = meta.split(' ').collect();
        if let [mode, _, object] = parts.as_slice() {
            out.insert(path.to_string(), format!("{mode} {object}"));
        }
    }
    Ok(out)
}

/// The tree of `base` with each of `paths` taken from `from`, or removed where
/// `from` lacks it. It is built in a scratch index, never the person's.
pub fn tree_with(dir: &Path, scratch_index: &Path, base: &str, from: &str, paths: &[String]) -> Result<String, String> {
    let index = scratch_index.to_string_lossy().into_owned();
    let env = [("GIT_INDEX_FILE", index.as_str())];
    let _ = std::fs::remove_file(scratch_index);
    git(dir, &["read-tree", base], &env, "")?;
    let have = entries(dir, from, paths)?;
    let mut info = String::new();
    for p in paths {
        match have.get(p) {
            Some(mode_object) => info.push_str(&format!("{mode_object}\t{p}\0")),
            None => info.push_str(&format!("0 0000000000000000000000000000000000000000\t{p}\0")),
        }
    }
    git(dir, &["update-index", "-z", "--index-info"], &env, &info)?;
    let tree = git(dir, &["write-tree"], &env, "")?.trim().to_string();
    let _ = std::fs::remove_file(scratch_index);
    Ok(tree)
}

/// A new commit of `tree` on `parent`, with `like`'s author, committer and
/// message.
pub fn commit_like(dir: &Path, tree: &str, parent: &str, like: &CommitInfo) -> Result<String, String> {
    let (Some((an, ae, ad)), Some((cn, ce, cd))) = (ident(&like.author), ident(&like.committer)) else {
        return Err(format!("the author of {} cannot be read", short(&like.id)));
    };
    let env = [
        ("GIT_AUTHOR_NAME", an.as_str()),
        ("GIT_AUTHOR_EMAIL", ae.as_str()),
        ("GIT_AUTHOR_DATE", ad.as_str()),
        ("GIT_COMMITTER_NAME", cn.as_str()),
        ("GIT_COMMITTER_EMAIL", ce.as_str()),
        ("GIT_COMMITTER_DATE", cd.as_str()),
    ];
    git(dir, &["commit-tree", tree, "-p", parent], &env, &like.message).map(|s| s.trim().to_string())
}

/// A scratch index path inside the repository's git folder.
pub fn scratch_index(dir: &Path, now: f64) -> Result<PathBuf, String> {
    let git_dir = git_line(dir, &["rev-parse", "--absolute-git-dir"])?;
    Ok(PathBuf::from(git_dir).join(format!("familiar-ai-tidy-index-{}", now as u64)))
}

/// The first seven characters of a commit id.
pub fn short(id: &str) -> &str {
    id.get(..7).unwrap_or(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_ident_line_splits_into_name_email_and_the_date_git_takes_back() {
        assert_eq!(
            ident("A Person <a@example.com> 1759700000 -0400"),
            Some(("A Person".into(), "a@example.com".into(), "1759700000 -0400".into()))
        );
        assert_eq!(ident("no email here"), None);
        assert_eq!(short("0123456789abcdef"), "0123456");
        assert_eq!(short("abc"), "abc");
    }
}
