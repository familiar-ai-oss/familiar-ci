//! Record the commits that have landed since the last one seen, found by
//! comparing the head with the record on every hook call rather than by
//! reading shell commands. Each new commit gets one `commit_seen` fact and one
//! `tests_observed` per criterion whose test files exist there. Also decides
//! which changes are passive, opening no run.

use std::fmt;
use std::path::Path;

use familiar_ai_facts::{last_of_kind, CommitSeen, Fact, FactFields, FingerprintStamped, Log, RanIn, Record, RecordError, TestsObserved};
use familiar_ai_git::{commit_seen, commits_between, head_commit, Anchor, GitError};

/// What the log says to read at one commit. The rules derive these values;
/// nothing here derives them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WhatToRead {
    pub plan_test_files: Vec<String>,
    /// Every criterion of the plan in plan order, each observed at every commit.
    pub all_tests: Vec<CriterionTests>,
    pub anchors: Vec<Anchor>,
}

/// One criterion's test files, planned and written, and its current `criterion_rev`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CriterionTests {
    pub criterion_id: String,
    pub files: Vec<String>,
    pub criterion_rev: String,
}

/// Each criterion's fingerprint over its own files at `commit`, skipping a
/// criterion with no file there. The commit reader, verify and tidy all
/// observe commits through this one function.
pub fn observations(repo_path: &Path, commit: &str, of: &[CriterionTests]) -> Result<Vec<TestsObserved>, GitError> {
    let asked: Vec<(String, Vec<String>)> = of.iter().map(|c| (c.criterion_id.clone(), c.files.clone())).collect();
    let seen = familiar_ai_git::observe_tests(repo_path, commit, &asked)?;
    Ok(of
        .iter()
        .zip(seen)
        .filter_map(|(c, (_, hash))| {
            Some(TestsObserved { criterion_id: c.criterion_id.clone(), criterion_rev: c.criterion_rev.clone(), commit: commit.to_string(), tests_hash: hash? })
        })
        .collect())
}

/// The `by` of a commit that landed during the agent's own Bash call, written
/// only from that call's PostToolUse.
pub const BY_AGENT: &str = "agent";

/// More new commits than this are refused rather than read, since a hook that
/// runs out of time lets the action through.
pub const MOST_COMMITS_AT_ONCE: usize = 200;

#[derive(Debug)]
/// What recording the landed commits could not do.
pub enum CommitsError {
    Record(RecordError),
    Git(GitError),
    /// The head is this many commits past the last one seen.
    TooMany(usize),
}

impl fmt::Display for CommitsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CommitsError::Record(e) => write!(f, "{e}"),
            CommitsError::Git(e) => write!(f, "{e}"),
            CommitsError::TooMany(n) => write!(
                f,
                "{n} commits have landed since the last one recorded (at most {MOST_COMMITS_AT_ONCE} are read at once)"
            ),
        }
    }
}

impl std::error::Error for CommitsError {}

impl From<RecordError> for CommitsError {
    fn from(e: RecordError) -> Self {
        CommitsError::Record(e)
    }
}

impl From<GitError> for CommitsError {
    fn from(e: GitError) -> Self {
        CommitsError::Git(e)
    }
}

/// A path inside Familiar's own folder (`.familiar/plan`, `.familiar/record`).
pub fn is_familiars_own(path: &str) -> bool {
    path == crate::FAMILIAR_FOLDER || path.starts_with(&format!("{}/", crate::FAMILIAR_FOLDER))
}

/// Whether a change opens no run: a prose or support path, or a change of
/// comment and blank lines only. Without `change` (the file before and
/// after) only the path is judged.
pub fn passive(root: &Path, path: &str, change: Option<(&str, &str)>) -> bool {
    prose_path(root, path) || change.is_some_and(|(old, new)| first_code_line(path, old, new).is_none())
}

/// Whether the path alone marks prose or support files.
pub fn prose_path(root: &Path, path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path);
    let ext = name.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase()).unwrap_or_default();
    // Not .txt, since requirements.txt and test data are code.
    matches!(ext.as_str(), "md" | "rst" | "adoc")
        || ["docs/", ".claude/", ".claude-plugin/"].iter().any(|d| path.starts_with(d))
        || matches!(name, "LICENSE" | "NOTICE")
        || is_familiars_own(path)
        || documentation(root).iter().any(|pattern| glob(pattern, path))
}

/// The patterns the repository's root `.gitattributes` marks
/// `linguist-documentation` (set, not `-` or `=false`).
fn documentation(root: &Path) -> Vec<String> {
    let text = std::fs::read_to_string(root.join(".gitattributes")).unwrap_or_default();
    text.lines()
        .filter_map(|l| {
            let mut words = l.split_whitespace();
            let pattern = words.next().filter(|w| !w.starts_with('#'))?;
            words
                .any(|a| a == "linguist-documentation" || a == "linguist-documentation=true")
                .then(|| pattern.trim_start_matches('/').to_string())
        })
        .collect()
}

/// Match a gitattributes pattern against a path; a pattern with no `/`
/// matches the file name.
fn glob(pattern: &str, path: &str) -> bool {
    fn at(p: &[u8], s: &[u8]) -> bool {
        match p {
            [] => s.is_empty(),
            [b'*', b'*', rest @ ..] => {
                let rest = rest.strip_prefix(b"/").unwrap_or(rest);
                (0..=s.len()).any(|i| at(rest, &s[i..]))
            }
            [b'*', rest @ ..] => (0..=s.len()).take_while(|&i| i == 0 || s[i - 1] != b'/').any(|i| at(rest, &s[i..])),
            [c, rest @ ..] => s.first() == Some(c) && at(rest, &s[1..]),
        }
    }
    if let Some(folder) = pattern.strip_suffix('/') {
        return path.starts_with(&format!("{folder}/"));
    }
    if !pattern.contains('/') {
        let name = path.rsplit('/').next().unwrap_or(path);
        return at(pattern.as_bytes(), name.as_bytes());
    }
    at(pattern.as_bytes(), path.as_bytes())
}

/// The first changed line that is code, with its number and text, or `None`
/// when every changed line is a comment or blank. Only `.rs` and `.py`
/// comments are recognised; any other file's change is code.
pub fn first_code_line(path: &str, old: &str, new: &str) -> Option<(u64, String)> {
    let comment: &[&str] = match path.rsplit_once('.').map(|(_, e)| e) {
        Some("rs") => &["//"],
        Some("py") => &["#"],
        _ => &[],
    };
    let quiet = |l: &str| l.trim().is_empty() || comment.iter().any(|c| l.trim_start().starts_with(c));
    let (olds, news): (Vec<&str>, Vec<&str>) = (old.lines().collect(), new.lines().collect());
    for h in familiar_ai_git::line_hunks(old.as_bytes(), new.as_bytes()) {
        let side = |lines: &[&str], start: u64, count: u64| -> Option<(u64, String)> {
            (start..start + count).find_map(|n| {
                let l = lines.get(usize::try_from(n).ok()?.checked_sub(1)?)?;
                (!quiet(l) || comment.is_empty()).then(|| (n, l.trim().to_string()))
            })
        };
        if let Some(found) = side(&news, h.new_start, h.new_count).or_else(|| side(&olds, h.old_start, h.old_count)) {
            return Some(found);
        }
    }
    None
}

/// The latest `commit_seen`'s commit, else the intent's sha, else `None`
/// because no commit belongs to a run yet.
pub fn last_commit_seen(log: &Log) -> Option<String> {
    let text = |fact: &Fact, field: &str| {
        fact.fields
            .get(field)
            .and_then(|v| v.as_str())
            .map(str::to_string)
    };
    if let Some(sha) = last_of_kind(log, "commit_seen").and_then(|f| text(f, "commit")) {
        return Some(sha);
    }
    last_of_kind(log, "intent_submitted").and_then(|f| text(f, "sha"))
}

/// Record every commit landed since the last one seen, oldest first, with no
/// `by`, and return the `commit_seen` facts written.
pub fn record_new_commits(
    record: &Record,
    repo_path: &Path,
    what_to_read: &dyn Fn(&Log) -> WhatToRead,
    at: f64,
    tenant: &str,
) -> Result<Vec<Fact>, CommitsError> {
    record_commits(record, repo_path, what_to_read, at, tenant, None)
}

/// Record every commit landed since the last one seen, each `commit_seen`
/// carrying `by`. Commits already seen are first observed again where due.
pub fn record_commits(
    record: &Record,
    repo_path: &Path,
    what_to_read: &dyn Fn(&Log) -> WhatToRead,
    at: f64,
    tenant: &str,
    by: Option<&str>,
) -> Result<Vec<Fact>, CommitsError> {
    let mut log = record.read_all()?;
    let Some(last) = last_commit_seen(&log) else {
        return Ok(Vec::new());
    };
    let now = what_to_read(&log);
    observe_again(record, repo_path, &mut log, &now, at, tenant)?;
    restamp_legacy(record, &mut log, at, tenant)?;
    let Some(head) = head_commit(repo_path)? else {
        return Ok(Vec::new());
    };
    let new_commits = commits_between(repo_path, &last, &head)?;
    if new_commits.len() > MOST_COMMITS_AT_ONCE {
        return Err(CommitsError::TooMany(new_commits.len()));
    }
    let mut written = Vec::with_capacity(new_commits.len());
    for commit in &new_commits {
        let what = what_to_read(&log);
        let mut fields: CommitSeen = commit_seen(repo_path, commit, &what.plan_test_files, &what.anchors)?.into();
        // A change under `.familiar/` is not code, so a tests-first commit that
        // carries the plan stays tests-only; the file stays listed in `files`.
        fields.touches_code = fields
            .files
            .iter()
            .any(|f| !what.plan_test_files.contains(f) && !is_familiars_own(f));
        fields.by = by.map(str::to_string);
        // the standing change to tests that stood at the plan's base (the owner, 2026-10-20) [draft]
        let mut raw = fields.to_fields().map_err(RecordError::from)?;
        let base = last_of_kind(&log, "intent_submitted").and_then(|f| f.fields.get("sha")).and_then(|v| v.as_str()).map(str::to_string);
        if let Some(base) = base {
            let changed = crate::door::existing::changes(repo_path, &base, commit, &what.plan_test_files);
            if !changed.is_empty() {
                raw.insert(familiar_ai_rules::existing::FIELD.to_string(), serde_json::to_value(&changed).unwrap_or_default());
            }
        }
        let fact = record.append_raw(CommitSeen::KIND, raw, at, tenant)?;
        log.push(fact.clone());
        written.push(fact);
        for o in observations(repo_path, commit, &what.all_tests)? {
            log.push(record.append(&o, at, tenant)?);
        }
    }
    Ok(written)
}

/// Observe each seen commit again for every criterion with no observation
/// there, or one taken under another `criterion_rev` whose fingerprint now
/// differs. A commit git no longer holds is skipped.
fn observe_again(record: &Record, repo_path: &Path, log: &mut Log, what: &WhatToRead, at: f64, tenant: &str) -> Result<(), CommitsError> {
    let ids: Vec<String> = {
        let mut seen = std::collections::HashSet::new();
        log.iter()
            .filter(|f| f.kind == "commit_seen")
            .filter_map(|f| f.fields.get("commit").and_then(|v| v.as_str()).map(str::to_string))
            .filter(|id| seen.insert(id.clone()))
            .collect()
    };
    if ids.is_empty() || what.all_tests.is_empty() {
        return Ok(());
    }
    let mut out: Vec<TestsObserved> = Vec::new();
    {
        let v = familiar_ai_rules::View::new(log);
        for id in &ids {
            let latest: Vec<Option<(String, String)>> = what
                .all_tests
                .iter()
                .map(|c| {
                    familiar_ai_rules::criterion::observed(&v, &c.criterion_id, id).map(|o| {
                        let t = |k: &str| o.fields.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
                        (t("criterion_rev"), t("tests_hash"))
                    })
                })
                .collect();
            let due: Vec<usize> = (0..what.all_tests.len())
                .filter(|&i| latest[i].as_ref().is_none_or(|(rev, _)| *rev != what.all_tests[i].criterion_rev))
                .collect();
            if due.is_empty() {
                continue;
            }
            let asked: Vec<CriterionTests> = due.iter().map(|&i| what.all_tests[i].clone()).collect();
            let Ok(seen) = observations(repo_path, id, &asked) else {
                continue;
            };
            for o in seen {
                let i = due.iter().copied().find(|&i| what.all_tests[i].criterion_id == o.criterion_id).unwrap_or(0);
                if latest[i].as_ref().is_some_and(|(_, hash)| *hash == o.tests_hash) {
                    continue; // only the rev changed, not the files
                }
                out.push(o);
            }
        }
    }
    for o in out {
        log.push(record.append(&o, at, tenant)?);
    }
    Ok(())
}

/// Restamp laptop stamps an older binary made with a legacy hash, using the
/// observed fingerprint where a laptop result stands, then restamp the
/// criterion's last stamp so it stays the latest. Legacy hashes are never
/// used as proof, and a second pass appends nothing.
fn restamp_legacy(record: &Record, log: &mut Log, at: f64, tenant: &str) -> Result<(), CommitsError> {
    use std::collections::{HashMap, HashSet};
    let t = |f: &Fact, k: &str| f.fields.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
    let mut ids: Vec<String> = Vec::new();
    let mut first_seen: HashMap<(String, String), u64> = HashMap::new();
    let mut results: HashSet<(String, String)> = HashSet::new();
    // Criteria in first-stamped order, and the latest laptop stamp per (criterion, commit).
    let mut criteria: Vec<String> = Vec::new();
    let mut latest: HashMap<(String, String), &Fact> = HashMap::new();
    let mut last: HashMap<String, String> = HashMap::new();
    for f in log.iter() {
        let key = || (t(f, "criterion_id"), t(f, "commit"));
        match f.kind {
            "commit_seen" => ids.push(t(f, "commit")),
            "tests_observed" => {
                first_seen.entry(key()).or_insert(f.t);
            }
            "pass_please_pass"
                if t(f, "ran_in") == "laptop"
                    && f.fields.get("with_anchor_reverted").and_then(|x| x.as_bool()) != Some(true)
                    && t(f, "outcome") != "did_not_run" =>
            {
                results.insert(key());
            }
            "fingerprint_stamped" if t(f, "ran_in") == "laptop" => {
                let (c, id) = key();
                if !criteria.contains(&c) {
                    criteria.push(c.clone());
                }
                last.insert(c.clone(), id.clone());
                latest.insert((c, id), f);
            }
            _ => {}
        }
    }
    let mut seen = HashSet::new();
    ids.retain(|id| seen.insert(id.clone()));
    let mut out: Vec<FingerprintStamped> = Vec::new();
    {
        let v = familiar_ai_rules::View::new(log);
        for c in &criteria {
            // No stamp without a result.
            let restamp = |id: &str| -> Option<FingerprintStamped> {
                if !results.contains(&(c.clone(), id.to_string())) {
                    return None;
                }
                let hash = familiar_ai_rules::criterion::observed(&v, c, id).map(|o| t(o, "tests_hash"))?;
                Some(FingerprintStamped { criterion_id: c.clone(), commit: id.to_string(), proof_not_tampered: hash, ran_in: RanIn::Laptop })
            };
            let mut mine: Vec<FingerprintStamped> = Vec::new();
            for id in &ids {
                let key = (c.clone(), id.clone());
                let Some(stamp) = latest.get(&key) else {
                    continue;
                };
                if first_seen.get(&key).is_none_or(|&o| stamp.t > o) {
                    continue; // stamped after the observation, so not legacy
                }
                match restamp(id) {
                    Some(s) if s.proof_not_tampered != t(stamp, "proof_not_tampered") => mine.push(s),
                    _ => {}
                }
            }
            let last = last.get(c).cloned().unwrap_or_default();
            if !mine.is_empty() && mine.last().is_none_or(|s| s.commit != last) {
                mine.extend(restamp(&last));
            }
            out.extend(mine);
        }
    }
    for s in out {
        log.push(record.append(&s, at, tenant)?);
    }
    Ok(())
}
