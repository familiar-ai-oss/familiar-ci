//! `familiar-ai tidy`: before the first push, rewrite the agent's unpushed
//! commits so each criterion leaves one seal commit and one fix commit, then
//! re-check the proof before moving the branch. Any mismatch leaves the branch
//! at its original head, which is also kept under `PRE_TIDY`. See
//! docs/decisions.md#tidy.

use std::collections::{BTreeSet, HashMap};
use std::path::Path;

use familiar_ai_facts::{append, CommitSeen, Fact, Log, Record, TestsObserved};
use familiar_ai_rules::criterion::{own_tests, results, sealed_at, Place};
use familiar_ai_rules::{lifecycle, plan as rules_plan, View};
use serde_json::Value;

use crate::Outcome;
use crate::door::commits::{is_familiars_own, last_commit_seen, BY_AGENT, MOST_COMMITS_AT_ONCE};
use crate::door::project::{open_project, plain};
use crate::door::git_program::{changed_paths, commit_info, commit_like, entries, git, git_line, scratch_index, short, tree_with};
use crate::door::tidy_plan::{self, Step, Tidied, Unpushed};
use crate::door::{FAMILIAR_FOLDER, PLAN_FILE, TENANT};
use crate::input::Surroundings;
use crate::teach::words::{quoted, LEAD};

/// Where the original head is kept: `<PRE_TIDY>/<seconds>`.
pub const PRE_TIDY: &str = "refs/familiar-ai/pre-tidy";

/// Why tidy did nothing: nothing to do (exit 0), or it could not (exit 1).
enum NotTidied {
    Nothing(String),
    Refused(String),
}

pub fn run_tidy(around: &Surroundings) -> Outcome {
    let (root, record) = match open_project(around) {
        Ok(x) => x,
        Err(out) => return out,
    };
    let mut lines: Vec<String> = Vec::new();
    let out = |lines: &[String]| lines.iter().map(|l| format!("{LEAD}{l}\n")).collect::<String>();
    match tidy(&root, &record, around, &mut lines) {
        Ok(()) => plain(0, out(&lines), String::new()),
        Err(NotTidied::Nothing(why)) => {
            lines.push(format!("nothing to tidy: {why}"));
            plain(0, out(&lines), String::new())
        }
        Err(NotTidied::Refused(why)) => plain(1, out(&lines), format!("{LEAD}{why}\n")),
    }
}

/// One tidied criterion, as the re-check and the facts need it.
#[derive(Clone)]
struct Crit {
    id: String,
    name: String,
    proof: String,
    files: Vec<String>,
    command: String,
}

fn tidy(root: &Path, record: &Record, around: &Surroundings, lines: &mut Vec<String>) -> Result<(), NotTidied> {
    let refused = NotTidied::Refused;
    let log = record.read_all().map_err(|e| refused(format!("cannot read its record: {e}")))?;
    let branch = git_line(root, &["symbolic-ref", "-q", "--short", "HEAD"])
        .map_err(|_| refused("the head is not on a branch, so nothing was tidied".to_string()))?;
    let old_head = git_line(root, &["rev-parse", "HEAD"]).map_err(refused)?;
    let ids = unpushed(root, &branch).map_err(NotTidied::Nothing)?;
    if ids.is_empty() {
        return Err(NotTidied::Nothing(format!("every commit of {branch} is pushed")));
    }
    // The agent's commits are those seen with `by: agent`; a commit made by hand has no `by` and is kept.
    let by_agent: BTreeSet<String> = log
        .iter()
        .filter(|f| f.kind == "commit_seen" && f.fields.get("by").and_then(Value::as_str) == Some(BY_AGENT))
        .filter_map(|f| f.fields.get("commit").and_then(Value::as_str).map(str::to_string))
        .collect();
    let mut commits: Vec<Unpushed> = Vec::with_capacity(ids.len());
    for id in &ids {
        let info = commit_info(root, id).map_err(refused)?;
        if info.parents.len() != 1 {
            return Err(NotTidied::Nothing(format!("{} is a merge (or the first commit), so the branch is left as it is", short(id))));
        }
        let paths = changed_paths(root, &info).map_err(refused)?;
        commits.push(Unpushed { by_agent: by_agent.contains(id), info, paths });
    }
    let (tidied, crits) = criteria(&log, &commits);
    if tidied.is_empty() {
        return Err(NotTidied::Nothing("no criterion has both its seal and its green among the unpushed commits".to_string()));
    }
    let rewrite = tidy_plan::plan(&commits, &tidied).map_err(NotTidied::Nothing)?;
    let crits: Vec<Crit> = rewrite.crits.iter().filter_map(|t| crits.iter().find(|c| c.id == t.criterion_id)).cloned().collect();
    let built = build(root, &commits, &rewrite.steps, around.now)?;
    if built.head == old_head {
        return Err(NotTidied::Nothing(format!("{branch} already has one seal commit and one fix commit per criterion")));
    }
    // The re-check checks commits out in the work tree, so tracked files must hold no change.
    // The plan file is excluded because the binary rewrites it from the record after every hook call.
    let plan = format!(":(exclude){FAMILIAR_FOLDER}/{PLAN_FILE}");
    let dirty = git(root, &["status", "--porcelain", "--untracked-files=no", "--", ":/", &plan], &[], "").map_err(refused)?;
    if !dirty.trim().is_empty() {
        return Err(refused(format!(
            "the work tree has uncommitted changes ({}), so nothing was tidied: commit them or set them aside, then run `familiar-ai tidy` again",
            dirty.lines().filter(|l| !l.trim().is_empty()).map(|l| l.get(3..).unwrap_or(l)).collect::<Vec<_>>().join(", ")
        )));
    }
    let tree = |c: &str| git_line(root, &["rev-parse", &format!("{c}^{{tree}}")]);
    if tree(&built.head).map_err(refused)? != tree(&old_head).map_err(refused)? {
        return Err(refused("the tidied commits would not end in the same files as the branch, so nothing was changed".to_string()));
    }
    let kept_at = format!("{PRE_TIDY}/{}", around.now as u64);
    git(root, &["update-ref", &kept_at, &old_head], &[], "").map_err(refused)?;
    lines.push(format!("the original head {} of {branch} is kept at {kept_at}", short(&old_head)));
    let base = commits[0].info.parents[0].clone();
    // Hold the plan as HEAD has it while other commits are checked out; it is
    // projected from the record again afterwards, whatever happens.
    let plan_path = format!("{FAMILIAR_FOLDER}/{PLAN_FILE}");
    let plan_changed = git(root, &["status", "--porcelain", "--untracked-files=no", "--", &plan_path], &[], "").map_err(refused)?;
    let checked = if plan_changed.trim().is_empty() { Ok(()) } else { git(root, &["checkout", "-q", "HEAD", "--", &plan_path], &[], "").map(|_| ()) }
        .and_then(|()| recheck(root, &base, &built, &crits, around, lines));
    let back = git(root, &["checkout", "-q", &branch], &[], "");
    let put_back = || crate::door::project_plan(root, &log);
    if let Err(why) = checked.and(back.map(|_| ())) {
        let _ = put_back();
        return Err(refused(restored(&branch, &old_head, &kept_at, &why)));
    }
    let facts = new_facts(root, &log, &built, &commits, around.now).map_err(|why| {
        let _ = put_back();
        refused(restored(&branch, &old_head, &kept_at, &why))
    })?;
    git(root, &["update-ref", "-m", "familiar-ai tidy", &format!("refs/heads/{branch}"), &built.head, &old_head], &[], "")
        .map_err(|why| {
            let _ = put_back();
            refused(restored(&branch, &old_head, &kept_at, &why))
        })?;
    for f in &facts {
        let appended = match f {
            Fresh::Seen(x) => record.append(x, around.now, TENANT),
            Fresh::Observed(x) => record.append(x, around.now, TENANT),
        };
        if let Err(e) = appended {
            let undo = git(root, &["update-ref", &format!("refs/heads/{branch}"), &old_head, &built.head], &[], "");
            let why = format!("the record could not take the new commits ({e}){}", undo.err().map(|u| format!("; and {u}")).unwrap_or_default());
            let _ = put_back();
            return Err(refused(restored(&branch, &old_head, &kept_at, &why)));
        }
    }
    // The fold moved the parts' green points to the folded commits, where under rule 3 they are
    // sealed until shown green there; the owed runs at the new head do that, with the machinery
    // the hooks use. The plan is re-projected from what the runs settled.
    let settled = owed_runs(root, record, around).map_err(refused)?;
    crate::door::project_plan(root, &settled).map_err(refused)?;
    lines.push(format!(
        "tidied {branch}: {} commits became {} ({} dropped, {} kept as {}); now at {}",
        commits.len(),
        commits.len() - rewrite.dropped(),
        counted(rewrite.dropped(), "failed attempt"),
        counted(rewrite.kept(), "commit"),
        if rewrite.kept() == 1 { "it was" } else { "they were" },
        short(&built.head)
    ));
    Ok(())
}

pub(crate) fn counted(n: usize, thing: &str) -> String {
    format!("{n} {thing}{}", if n == 1 { "" } else { "s" })
}

/// The line for an abort: the branch is at its original head.
fn restored(branch: &str, old_head: &str, kept_at: &str, why: &str) -> String {
    format!("{why}; nothing was changed: {branch} is at {} as before (also kept at {kept_at})", short(old_head))
}

/// The branch's unpushed commits, oldest first. With no upstream, only those
/// after the merge-base with `origin/HEAD`, else `main`, else `master`.
fn unpushed(root: &Path, branch: &str) -> Result<Vec<String>, String> {
    let tracks = git_line(root, &["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{u}"]).is_ok();
    let mut args: Vec<String> = ["rev-list", "--reverse", "--topo-order", "HEAD", "--not", "--remotes"].iter().map(|s| s.to_string()).collect();
    if !tracks {
        let default = git_line(root, &["symbolic-ref", "-q", "--short", "refs/remotes/origin/HEAD"])
            .ok()
            .or_else(|| ["main", "master"].iter().find(|b| git_line(root, &["rev-parse", "--verify", "-q", &format!("refs/heads/{b}")]).is_ok()).map(|b| b.to_string()))
            .ok_or_else(|| format!("{branch} tracks no branch and there is no main or master to tell its own commits from"))?;
        let base = git_line(root, &["merge-base", "HEAD", &default]).map_err(|_| format!("{branch} shares no history with {default}"))?;
        args.push(base);
    }
    let argv: Vec<&str> = args.iter().map(String::as_str).collect();
    Ok(git_line(root, &argv)?.lines().map(str::to_string).filter(|l| !l.is_empty()).collect())
}

/// The criteria whose latest red seal and first green after it are both
/// unpushed.
fn criteria(log: &Log, commits: &[Unpushed]) -> (Vec<Tidied>, Vec<Crit>) {
    let v = View::new(log);
    let ix = v.idx();
    let text = |f: &Fact, k: &str| f.fields.get(k).and_then(Value::as_str).map(str::to_string);
    let mut tidied = Vec::new();
    let mut crits = Vec::new();
    for c in rules_plan::criteria(&v) {
        if lifecycle::released(&v, &c) {
            continue;
        }
        let Some(seal) = sealed_at(&v, &c, Place::Laptop) else { continue };
        let (Some(seal_id), Some(proof)) = (text(seal, "commit"), text(seal, "proof_not_tampered")) else {
            continue;
        };
        // Match by the commit's own id, since two commits can share the
        // changed files' names and the tests' hash.
        let Some(at) = commits.iter().position(|u| u.info.id == seal_id) else { continue };
        let own = own_tests(&v, &c);
        let laptop = results(&v, &c, Place::Laptop, false);
        let passed_at = |id: &str| {
            !own.is_empty()
                && own.iter().all(|t| {
                    laptop.iter().rev().find(|r| text(r, "commit").as_deref() == Some(id) && text(r, "file_path").as_deref() == Some(t.file_path.as_str()))
                        .is_some_and(|r| text(r, "outcome").as_deref() == Some("passed"))
                })
        };
        let green = commits.iter().enumerate().skip(at + 1).find(|(_, u)| {
            passed_at(&u.info.id) && ix.key_of(&u.info.id).is_some_and(|k| lifecycle::green_at(&v, &c, Place::Laptop, Some(k)))
        });
        let (Some((green, _)), Some(command)) = (green, lifecycle::command_at(&v, &c, i64::MAX)) else { continue };
        let mut files: Vec<String> = Vec::new();
        for f in v.of("test_planned", &c).chain(v.of("test_written", &c)) {
            if let Some(fp) = text(f, "file_path") {
                if !files.contains(&fp) {
                    files.push(fp);
                }
            }
        }
        tidied.push(Tidied { criterion_id: c.clone(), seal: at, green });
        crits.push(Crit { id: c.clone(), name: quoted(&v, &c), proof, files, command: command.to_string() });
    }
    (tidied, crits)
}

/// The rewritten branch, and which original commit each new one stands for.
struct Built {
    head: String,
    seal: Vec<String>,
    fix: Vec<String>,
    from: HashMap<String, usize>,
}

/// Make the new commits on the first unpushed commit's parent; a commit that
/// comes out identical keeps its id. A kept commit that touches files a
/// dropped commit changed cannot stay in place, so nothing is tidied.
fn build(root: &Path, commits: &[Unpushed], steps: &[Step], now: f64) -> Result<Built, NotTidied> {
    let refused = NotTidied::Refused;
    let index = scratch_index(root, now).map_err(refused)?;
    let n = steps.iter().filter(|s| matches!(s, Step::Seal { .. })).count();
    let mut built = Built { head: commits[0].info.parents[0].clone(), seal: vec![String::new(); n], fix: vec![String::new(); n], from: HashMap::new() };
    for step in steps {
        let (from, paths) = match step {
            Step::Drop(_) => continue,
            Step::Keep(i) => {
                let k = &commits[*i];
                let (now_there, before) = (entries(root, &built.head, &k.paths).map_err(refused)?, entries(root, &k.info.parents[0], &k.paths).map_err(refused)?);
                if now_there != before {
                    return Err(NotTidied::Nothing(format!(
                        "{} (\"{}\") changes files the agent's dropped commits also changed, so it cannot be kept in place",
                        short(&k.info.id),
                        k.info.subject()
                    )));
                }
                (*i, &k.paths)
            }
            Step::Seal { from, paths, .. } | Step::Fix { from, paths, .. } => (*from, paths),
        };
        let like = &commits[from].info;
        let tree = tree_with(root, &index, &built.head, &like.id, paths).map_err(refused)?;
        let id = if built.head == like.parents[0] && tree == like.tree {
            like.id.clone()
        } else {
            commit_like(root, &tree, &built.head, like).map_err(refused)?
        };
        match step {
            Step::Seal { crit, .. } => built.seal[*crit] = id.clone(),
            Step::Fix { crit, .. } => built.fix[*crit] = id.clone(),
            _ => {}
        }
        built.from.insert(id.clone(), from);
        built.head = id;
    }
    Ok(built)
}

/// Re-check every tidied criterion: the proof is at the new seal, the tests
/// fail there and pass at the fix.
fn recheck(root: &Path, base: &str, built: &Built, crits: &[Crit], around: &Surroundings, lines: &mut Vec<String>) -> Result<(), String> {
    for (i, c) in crits.iter().enumerate() {
        let (seal, fix) = (&built.seal[i], &built.fix[i]);
        let hash = familiar_ai_git::tests_hash(root, seal, &c.files).map_err(|e| format!("{}: {e}", c.name))?;
        if hash != c.proof {
            return Err(format!("{}: the tests' hash at the new seal {} is not the recorded proof", c.name, short(seal)));
        }
        let first = familiar_ai_git::find_by_tests_hash(root, base, &built.head, &c.files, &c.proof).map_err(|e| format!("{}: {e}", c.name))?;
        if first.as_deref() != Some(seal.as_str()) {
            return Err(format!("{}: the first commit with the proof is not the new seal {}", c.name, short(seal)));
        }
        for (at, want_pass) in [(seal, false), (fix, true)] {
            git(root, &["checkout", "-q", "--detach", at], &[], "")?;
            let ran = familiar_ai_run::run_test_command(&c.command, root, around.test_run_wait);
            crate::door::kept::keep_run(root, at, &ran.stdout, &ran.stderr);
            let passed = ran.verdict == familiar_ai_run::Verdict::Passed;
            if passed != want_pass {
                let (what, should) = if want_pass { ("fix", "pass") } else { ("seal", "fail") };
                return Err(format!("{}: the tests should {should} at the {what} {} and did not", c.name, short(at)));
            }
        }
        lines.push(format!(
            "re-checked {}: the seal {} carries the proof {} and its tests fail there; they pass at the fix {}",
            c.name,
            short(seal),
            c.proof.get(..12).unwrap_or(&c.proof),
            short(fix)
        ));
    }
    Ok(())
}

/// A fact for the new commits: a `commit_seen`, or an observation beside it.
enum Fresh {
    Seen(CommitSeen),
    Observed(TestsObserved),
}

/// The `commit_seen` facts and observations for the new commits, as a hook
/// would write them. Refused if they would change any criterion's state.
fn new_facts(root: &Path, log: &Log, built: &Built, commits: &[Unpushed], now: f64) -> Result<Vec<Fresh>, String> {
    let last = last_commit_seen(log).ok_or("the record has seen no commit")?;
    let ids = familiar_ai_git::commits_between(root, &last, &built.head).map_err(|e| e.to_string())?;
    if ids.len() > MOST_COMMITS_AT_ONCE {
        return Err(format!("{} new commits are more than the record takes at once", ids.len()));
    }
    let mut grown = log.clone();
    let mut out = Vec::with_capacity(ids.len());
    for id in &ids {
        let w = crate::door::what_to_read(&grown);
        let from = built.from.get(id).copied();
        let mut fields: CommitSeen = familiar_ai_git::commit_seen(root, id, &w.plan_test_files, &w.anchors).map_err(|e| e.to_string())?.into();
        fields.touches_code = fields.files.iter().any(|f| !w.plan_test_files.contains(f) && !is_familiars_own(f));
        fields.by = from.and_then(|i| commits.get(i)).filter(|u| u.by_agent).map(|_| BY_AGENT.to_string());
        grown = append(&grown, &fields, now, TENANT).map_err(|e| e.to_string())?;
        out.push(Fresh::Seen(fields));
        for o in crate::door::commits::observations(root, id, &w.all_tests).map_err(|e| e.to_string())? {
            grown = append(&grown, &o, now, TENANT).map_err(|e| e.to_string())?;
            out.push(Fresh::Observed(o));
        }
    }
    let (before, after) = (View::new(log), View::new(&grown));
    for c in rules_plan::criteria(&before) {
        let (was, is) = (lifecycle::state(&before, &c), lifecycle::state(&after, &c));
        // a fold moves the green point; under rule 3 the part is sealed until shown green at the folded commit, which the owed runs right after the fold do
        if was != is && !(was == "green" && is == "sealed") {
            return Err(format!("the new commits would move {} from {was} to {is}", quoted(&before, &c)));
        }
    }
    Ok(out)
}

/// The owed runs at the new head, with the machinery the hooks use (catch-up): read the log
/// fresh, write the facts it owes, reserve and run them (inline or detached), and write what
/// they settle, until no run is live. The results land on the folded commits, which is what
/// shows them green there and lifts the state the fold moved it to.
fn owed_runs(root: &Path, record: &Record, around: &Surroundings) -> Result<Log, String> {
    let until = std::time::Instant::now() + around.run_inline;
    let mut log = record.read_all().map_err(|e| crate::door::cannot("read its record", &e))?;
    let mut children = Vec::new();
    loop {
        let moves = crate::door::run::binary_facts(record, &mut log, around.now, TENANT, around).map_err(|e| crate::door::cannot("write its record", &e))?;
        let launching = !moves.reserved.is_empty();
        if launching {
            children.extend(record.while_released(around.record_wait, || crate::door::runner::spawn(root, around, &moves.reserved)).map_err(|e| crate::door::cannot("reopen its record", &e))?);
            log = record.read_all().map_err(|e| crate::door::cannot("read its record", &e))?;
        }
        crate::door::runner::reap(&mut children);
        if !launching && !crate::door::runs::any_live(&log, around.now) && children.is_empty() {
            break;
        }
        let left = until.saturating_duration_since(std::time::Instant::now());
        if left.is_zero() {
            break;
        }
        record.while_released(around.record_wait, || crate::door::runner::nap(&mut children, left.min(std::time::Duration::from_millis(50)))).map_err(|e| crate::door::cannot("reopen its record", &e))?;
        log = record.read_all().map_err(|e| crate::door::cannot("read its record", &e))?;
    }
    Ok(log)
}
