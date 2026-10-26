//! `familiar-ai verify`: CI's recheck of the proof from committed data only,
//! never the laptop's record. The plan at HEAD and the commits since the intent
//! become facts, and the binary runs whatever the rules ask for until no move
//! is left. See docs/decisions.md#ci-proves-the-laptop-guides.

use std::path::{Path, PathBuf};

use familiar_ai_facts::{append_raw, last_of_kind, Fact, Log, Record};
use familiar_ai_rules::{action, View};
use serde_json::{json, Map, Value};

use crate::Outcome;
use crate::door::project::{find_project, plain, start_dir};
use crate::door::verify_report as report;
use crate::door::git_program::short;
use crate::door::approval::Payload;
use crate::door::verify_run::{run_at_read, At, Place, Ran};
use crate::door::{commits, setup, what_to_read, FAMILIAR_FOLDER, PLAN_FILE, RECORD_FILE, TENANT};
use crate::input::Surroundings;
use crate::teach::verify as words;

/// The criterion whose tests-first or teeth run did not happen, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stopped {
    pub criterion_id: String,
    pub commit: String,
    pub teeth: bool,
    pub why: String,
}

/// One run of the command, as the lines cite it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Run {
    pub criterion_id: String,
    pub commit: String,
    pub teeth: bool,
    pub ran: Ran,
}

pub struct Rechecked {
    pub log: Log,
    pub runs: Vec<Run>,
    pub stopped: Option<Stopped>,
    pub head: String,
    /// One recheck per part, each at its own last commit; empty when only
    /// the plan was read.
    pub parts: Vec<Rechecked>,
    /// Each criterion an approval at HEAD names that no signer CI allows
    /// vouches for, and why: the part is "approval not verified" (fail closed).
    pub unverified: Vec<(String, String)>,
    /// One line for each runner whose reader CI's own dry run chose differs from the one the
    /// adapter committed at HEAD names (plan A); it never changes the exit code.
    pub notes: Vec<String>,
}

/// Print one line per criterion and exit 0 when every criterion is proven or
/// released, 1 when any is not proven, 2 when the proof could not be established.
pub fn run_verify(around: &Surroundings) -> Outcome {
    let start = start_dir(around);
    let ci = report::Ci::from_env();
    let checked = match recheck(&start, around, true) {
        Ok(c) => c,
        Err(line) => {
            let err = report::write(&start, around.now, &ci, &report::Came::CouldNot(&line), 2).err().map(|e| report_line(&e)).unwrap_or_default();
            return plain(2, format!("{line}\n"), err);
        }
    };
    let (exit, rows) = words::rows(&checked);
    let mut out = rows.iter().map(|r| r.line.as_str()).collect::<Vec<_>>().join("\n");
    out.push('\n');
    for note in &checked.notes {
        out.push_str(note);
        out.push('\n');
    }
    // The report is written even when the record cannot be.
    let written = write_record(&checked, &start, around);
    let reported = report::write(&start, around.now, &ci, &report::Came::Rows(&rows), if written.is_ok() { exit } else { 2 });
    let mut err = String::new();
    if let Err(e) = &written {
        err.push_str(&format!("{}\n", words::could_not(&format!("its record could not be written: {e}"), "run it where .familiar can be written")));
    }
    if let Err(e) = &reported {
        err.push_str(&report_line(e));
    }
    plain(if err.is_empty() { exit } else { 2 }, out, err)
}

/// The line for a report that cannot be trusted: verify exits 2.
fn report_line(why: &str) -> String {
    format!("{}\n", words::could_not(&format!("its report cannot be trusted: {why}"), "untrack it (git rm --cached) and let verify write it where .familiar can be written"))
}

/// Build the log from the plan and commits and make every move the rules ask
/// for, one part at a time. `Err` is the one line for a proof that cannot even
/// start.
///
/// A part is proven at its own commits: walked from its own intent's sha to
/// its own last commit (the last that touches its place or its tests), under
/// the command in force there, so a later part's stack line never changes its
/// command and the merged head is not where it is proven. Which files are
/// tests is read from the plan at HEAD. See
/// docs/decisions.md#ci-proves-the-laptop-guides.
///
/// `fenced` is `run_verify`'s (plan §6.9): a pipeline stamp in the plan is not the pipeline's
/// own, and a mirrored `place_chosen`'s `hold`/`choice` stands only when a witness vouches for
/// that answer. The seed (setup) reads the plan as it is committed.
pub fn recheck(start: &Path, around: &Surroundings, fenced: bool) -> Result<Rechecked, String> {
    let fail = |why: String, next: &str| words::could_not(&why, next);
    let worktree = familiar_ai_git::worktree_root(start)
        .ok()
        .flatten()
        .ok_or_else(|| fail(format!("{} is not inside a git repository", start.display()), "run it in a clone of the repository"))?;
    let head = familiar_ai_git::head_commit(&worktree)
        .ok()
        .flatten()
        .ok_or_else(|| fail("the repository has no commit".into(), "commit the work and its plan"))?;
    let same = |p: PathBuf| std::fs::canonicalize(&p).unwrap_or(p);
    let worktree = same(worktree);
    let root = same(find_project(start).unwrap_or_else(|| worktree.clone()));
    // The person's answers come only from a witness: approvals at HEAD that a
    // signer of CI's own configuration signed (plan §6.9), never the plan.
    let mut witness = crate::door::approval::witnessed(&worktree, &root, &head, report::Ci::from_env().allowed_signers.as_deref());
    let mut plan = plan_facts(&worktree, &root, &head, around.now)?;
    if fenced {
        let vouched: Vec<&Payload> = witness.verified.iter().map(|w| &w.payload).collect();
        plan = fence(&plan, &vouched, around.now)?;
    }
    let crits = action::crits(&View::new(&plan));
    if crits.is_empty() {
        return Err(fail(format!("the plan committed at {} names no criterion", short(&head)), "commit the plan with the work"));
    }
    // A file that fails its check and names no part could be any part's: it stands against every one.
    let unnamed: Vec<String> = witness.unverified.iter().filter(|(c, _)| c.is_empty()).map(|(_, why)| why.clone()).collect();
    witness.unverified.retain(|(c, _)| !c.is_empty());
    for why in unnamed {
        witness.unverified.extend(crits.iter().map(|c| (c.clone(), why.clone())));
    }
    let place = Place { worktree: &worktree, root: &root, head: &head, wait: around.test_run_wait };
    let (choices, notes) = pipeline_dry_runs(&place, &plan);
    let readers: Vec<(String, String)> = choices.iter().map(|c| (c.runner.clone(), c.reader.clone())).collect();
    let mut walks: Vec<(String, Log)> = Vec::new();
    let mut parts: Vec<Rechecked> = Vec::new();
    for c in &crits {
        let sha = intent_of(&plan, c).ok_or_else(|| fail(format!("the plan committed at {} has no intent", short(&head)), "commit the plan with the work"))?;
        if !walks.iter().any(|(s, _)| *s == sha) {
            let mut walked = plan.clone();
            commit_facts(&mut walked, &worktree, &sha, &head, around.now)?;
            walks.push((sha.clone(), walked));
        }
        let walked = walks.iter().find(|(s, _)| *s == sha).map(|(_, l)| l).ok_or("no walk")?;
        let answers: Vec<&Payload> = witness.verified.iter().map(|w| &w.payload).filter(|p| p.criterion_id == *c).collect();
        parts.push(part(walked, plan.len(), c, &sha, &place, around.now, &answers, &readers)?);
    }
    // The record: the plan, the longest walk, and each part's pipeline facts.
    let mut log = walks.iter().map(|(_, l)| l).max_by_key(|l| l.len()).cloned().unwrap_or(plan);
    let (mut runs, mut stopped, mut unverified) = (Vec::new(), None, witness.unverified);
    for p in &parts {
        unverified.extend(p.unverified.iter().cloned());
        for f in p.log.iter().filter(|f| by_pipeline(f, &p.log)) {
            log = append_raw(&log, f.kind, f.fields.clone(), around.now, TENANT).map_err(|e| e.to_string())?;
        }
        runs.extend(p.runs.iter().cloned());
        stopped = stopped.or_else(|| p.stopped.clone());
    }
    // CI's own choices, ran_in pipeline: the pipeline's facts
    for c in &choices {
        log = familiar_ai_facts::append(&log, c, around.now, TENANT).map_err(|e| e.to_string())?;
    }
    Ok(Rechecked { log, runs, stopped, head, parts, unverified, notes })
}

/// The plan's fences (plan §6.9): a `fingerprint_stamped` with `ran_in: pipeline` is dropped (the
/// pipeline's stamps are its own, written here), and a `place_chosen`'s `hold`/`choice` (the
/// person's keep_going) is dropped unless a verified approval of that part gives that answer, so
/// the place reads as a plain re-anchor.
fn fence(plan: &Log, vouched: &[&Payload], at: f64) -> Result<Log, String> {
    let mut log: Log = Vec::new();
    for f in plan {
        if f.kind == "fingerprint_stamped" && f.fields.get("ran_in").and_then(Value::as_str) == Some("pipeline") {
            continue;
        }
        let mut fields = f.fields.clone();
        if f.kind == "place_chosen" && (fields.contains_key("hold") || fields.contains_key("choice")) {
            let said = |k: &str| fields.get(k).and_then(Value::as_str).unwrap_or("").to_string();
            let c = said("criterion_id");
            if !vouched.iter().any(|p| p.criterion_id == c && p.hold == said("hold") && p.choice == said("choice")) {
                fields.remove("hold");
                fields.remove("choice");
            }
        }
        log = append_raw(&log, f.kind, fields, at, TENANT).map_err(|e| e.to_string())?;
    }
    Ok(log)
}

/// What the pipeline itself wrote, as opposed to the plan (whose laptop stamps
/// and results are already in the record) and the commits: its runs (reserved
/// by the pipeline, their ends and translations) among them.
fn by_pipeline(f: &Fact, log: &Log) -> bool {
    let rid = |f: &Fact| f.fields.get("run_id").and_then(Value::as_str).map(str::to_string);
    let pipeline_run = |id: &str| log.iter().any(|r| r.kind == "run_reserved" && rid(r).as_deref() == Some(id) && r.fields.get("by").and_then(Value::as_str) == Some("pipeline"));
    f.kind == "pipeline_run_seen"
        || f.fields.get("ran_in").and_then(Value::as_str) == Some("pipeline")
        || (matches!(f.kind, "run_reserved" | "run_ended" | "test_log_translation") && rid(f).is_some_and(|id| pipeline_run(&id)))
}

/// The sha of the intent a criterion belongs to: the last intent before the
/// criterion's first claim in the plan, else the plan's last intent.
fn intent_of(plan: &Log, c: &str) -> Option<String> {
    let sha = |f: &Fact| f.fields.get("sha").and_then(Value::as_str).map(str::to_string);
    let mut last: Option<String> = None;
    for f in plan {
        if f.kind == "intent_submitted" {
            last = sha(f).or(last);
        } else if f.fields.get("criterion_id").and_then(Value::as_str) == Some(c) {
            return last;
        }
    }
    last_of_kind(plan, "intent_submitted").and_then(sha)
}

/// One part's recheck: the plan's claims of this part and its intent, none of another's,
/// the stack lines committed at its own last commit, the walk up to that
/// commit, and the moves the rules ask for there.
///
/// Each verified approval of this part is appended as the person's
/// `hold_answered` right after the `commit_seen` of the commit it was answered
/// at, or after the part's own facts when that commit comes later; one whose
/// commit is not in the history is not verified.
#[allow(clippy::too_many_arguments)]
fn part(walked: &Log, plan_len: usize, c: &str, sha: &str, place: &Place, at: f64, answers: &[&Payload], readers: &[(String, String)]) -> Result<Rechecked, String> {
    let v = View::new(walked);
    let files = action::test_files(&v, c);
    let greenfield = action::greenfield(&v, c);
    let seen_at = |f: &Fact| f.fields.get("commit").and_then(Value::as_str).map(str::to_string);
    let flag = |f: &Fact, k: &str| f.fields.get(k) == Some(&json!(true));
    // Its own: a commit on its place or its tests; for new work, whose fix is
    // any code (as the rules' overlap reads it), code on no part's place.
    let own = |f: &&Fact| {
        let overlap = f.fields.get("anchor_overlap").and_then(Value::as_array);
        f.kind == "commit_seen"
            && (overlap.is_some_and(|a| a.iter().any(|x| x.as_str() == Some(c)))
                || f.fields.get("files").and_then(Value::as_array).is_some_and(|a| a.iter().any(|p| p.as_str().is_some_and(|p| files.iter().any(|x| x == p))))
                || (greenfield && flag(f, "touches_code") && overlap.is_none_or(|a| a.is_empty())))
    };
    let next_seen = |i: usize| walked[i + 1..].iter().position(|f| f.kind == "commit_seen").map(|j| i + 1 + j);
    let last = walked[plan_len..].iter().rposition(|f| own(&f)).map(|i| i + plan_len);
    let (end, own_commit, at_commit) = match last {
        Some(i) => {
            // Commits right after it that touch neither code nor tests (the
            // plan committed after the fix) carry it to them.
            let mut h = i;
            while let Some(j) = next_seen(h).filter(|&j| !flag(&walked[j], "touches_code") && !flag(&walked[j], "touches_tests")) {
                h = j;
            }
            let commit_of = |k: usize| seen_at(&walked[k]).unwrap_or_else(|| place.head.to_string());
            (next_seen(h).unwrap_or(walked.len()), commit_of(i), commit_of(h))
        }
        None => (walked.len(), place.head.to_string(), place.head.to_string()),
    };
    let stacks = stacks_at(place, &own_commit).filter(|s| !s.is_empty()).unwrap_or_else(|| walked[..plan_len].iter().filter(|f| f.kind == "test_stack_detected").cloned().collect());
    let mut log: Log = Vec::new();
    // Another part's claims, and another ask's intent (whose sha would be
    // where its lines are put back), are not this part's.
    // Under a witnessed a_dependency, the other parts' tests, places and laptop stamps stay:
    // CI finds the dependency by its stamp at the re-earn commit and runs it there (§7.1).
    let dep = answers.iter().any(|a| a.hold == familiar_ai_rules::dependency::CRASHED && a.choice == familiar_ai_rules::dependency::A_DEPENDENCY);
    let read_of_a_dependency = |f: &Fact| dep && DEPENDENCY_KINDS.contains(&f.kind);
    let others = |f: &Fact| {
        (f.fields.get("criterion_id").and_then(Value::as_str).is_some_and(|x| x != c) && !read_of_a_dependency(f))
            || (f.kind == "intent_submitted" && f.fields.get("sha").and_then(Value::as_str) != Some(sha))
    };
    let answered_at = |a: &Payload, f: &Fact| f.kind == "commit_seen" && seen_at(f).as_deref() == Some(a.commit.as_str());
    for f in walked[..plan_len].iter().filter(|f| f.kind != "test_stack_detected" && !others(f)).chain(stacks.iter()).chain(walked[plan_len..end].iter()) {
        log = append_raw(&log, f.kind, f.fields.clone(), at, TENANT).map_err(|e| e.to_string())?;
        for a in answers.iter().filter(|a| answered_at(a, f)) {
            log = append_raw(&log, "hold_answered", a.answer(), at, TENANT).map_err(|e| e.to_string())?;
        }
    }
    let mut unverified = Vec::new();
    for a in answers.iter().filter(|a| !walked[plan_len..end].iter().any(|f| answered_at(a, f))) {
        if walked[end..].iter().any(|f| answered_at(a, f)) {
            log = append_raw(&log, "hold_answered", a.answer(), at, TENANT).map_err(|e| e.to_string())?;
        } else {
            unverified.push((c.to_string(), format!("the commit it was answered at, {}, is not in the history from the intent", short(&a.commit))));
        }
    }
    // The person's fix_what_broke_it on command_broke keeps the command they saw: CI does not
    // run one the agent changed after it (plan §6.9).
    for a in answers.iter().filter(|a| a.hold == "command_broke" && a.choice == "fix_what_broke_it") {
        let now = action::command(&View::new(&log), c);
        let then = plan_facts(place.worktree, place.root, &a.commit, at).ok().and_then(|p| action::command(&View::new(&p), c));
        if then.is_none() || then != now {
            unverified.push((c.to_string(), format!("the command changed after your fix_what_broke_it at {}", short(&a.commit))));
            return Ok(Rechecked { log, runs: Vec::new(), stopped: None, head: at_commit, parts: Vec::new(), unverified, notes: Vec::new() });
        }
    }
    let (runs, stopped) = moves(&mut log, place, at, readers)?;
    // H-4: the person's its_the_red / its_the_pass stands in CI only where CI's own run reads
    // can't-tell with the label and deciding line the approval binds; it is bound to that run
    for a in answers.iter().filter(|a| a.hold == "run_unclassified" && familiar_ai_rules::criterion::CONFIRMS.contains(&a.choice.as_str())) {
        match ci_confirms(&log, c, a) {
            Some(rid) => {
                let mut fields = a.answer();
                fields.insert("run_id".into(), Value::from(rid));
                log = append_raw(&log, "hold_answered", fields, at, TENANT).map_err(|e| e.to_string())?;
            }
            None => unverified.push((
                c.to_string(),
                format!("its {} at {} does not match CI's own run: CI did not read can't-tell with the label {:?} and the deciding line {:?}", a.choice, short(&a.commit), a.label, a.evidence),
            )),
        }
    }
    Ok(Rechecked { log, runs, stopped, head: at_commit, parts: Vec::new(), unverified, notes: Vec::new() })
}

/// H-4, CI's acceptance rule: CI's own run of `c` that reads can't-tell with the label and
/// deciding line the person's `its_the_red` / `its_the_pass` approval binds, where its reader
/// looks (the tests-first commit for `its_the_red`, a later commit for `its_the_pass`); its id.
pub fn ci_confirms(log: &Log, c: &str, p: &Payload) -> Option<String> {
    use familiar_ai_rules::criterion::{results, tests_first_commit, Place as RanIn};
    use familiar_ai_rules::outcome::{result_family, translation_of, Family};
    if p.label.is_empty() {
        return None;
    }
    let v = View::new(log);
    let tf = tests_first_commit(&v, c)?;
    let ix = v.idx();
    let said = |f: &Fact, k: &str| f.fields.get(k).and_then(Value::as_str).map(str::to_string);
    results(&v, c, RanIn::Pipeline, false).into_iter().rev().find_map(|r| {
        let rid = said(r, "run_id")?;
        let t = translation_of(&v, &rid)?;
        let at_tf = said(r, "commit").and_then(|id| ix.key_of(&id).map(str::to_string)).as_deref() == Some(tf.key.as_str());
        let stage = match p.choice.as_str() {
            "its_the_red" => at_tf,
            "its_the_pass" => !at_tf,
            _ => false,
        };
        // the label exactly; the deciding line without its run-specific noise (approval::deciding_line)
        let line = |e: &str| crate::door::approval::deciding_line(e);
        let same = said(t, "label").as_deref() == Some(p.label.as_str()) && said(t, "evidence").is_some_and(|e| line(&e) == line(&p.evidence));
        (stage && same && result_family(&v, c, r) == Family::Unknown).then_some(rid)
    })
}

/// Another part's claims a part keeps under a witnessed `a_dependency`.
const DEPENDENCY_KINDS: [&str; 6] = ["test_planned", "test_written", "fingerprint_stamped", "place_chosen", "new_work_spot_chosen", "new_work_no_spot"];

/// The `test_stack_detected` claims of the plan committed at `commit`, in file
/// order; `None` when no plan is committed there or it cannot be read, and
/// then the plan at HEAD's stack lines stand.
fn stacks_at(place: &Place, commit: &str) -> Option<Log> {
    let rel = setup::paths_in_repository(place.root, place.worktree)
        .map(|(_, plan)| plan)
        .unwrap_or_else(|_| format!("{FAMILIAR_FOLDER}/{PLAN_FILE}"));
    let claims = familiar_ai_plan::from_bytes(&familiar_ai_git::file_at(place.worktree, commit, &rel).ok()?).ok()?;
    let mut log: Log = Vec::new();
    for c in claims.into_iter().filter(|c| c.kind() == "test_stack_detected") {
        log = append_raw(&log, c.kind(), c.fields().clone(), 0.0, TENANT).ok()?;
    }
    Some(log)
}

/// `.familiar/plan` as committed at HEAD, its claims as facts in file order.
pub(crate) fn plan_facts(worktree: &Path, root: &Path, head: &str, at: f64) -> Result<Log, String> {
    let rel = setup::paths_in_repository(root, worktree)
        .map(|(_, plan)| plan)
        .unwrap_or_else(|_| format!("{FAMILIAR_FOLDER}/{PLAN_FILE}"));
    let bytes = familiar_ai_git::file_at(worktree, head, &rel)
        .map_err(|_| words::could_not(&format!("no {rel} is committed at {}", short(head)), "commit the plan with the work (familiar-ai setup keeps it tracked)"))?;
    let claims = familiar_ai_plan::from_bytes(&bytes)
        .map_err(|e| words::could_not(&format!("the plan committed at {} cannot be read: {e}", short(head)), "commit a plan the binary wrote"))?;
    if claims.is_empty() {
        return Err(words::could_not(&format!("the plan committed at {} is empty", short(head)), "commit the plan with the work"));
    }
    let mut log: Log = Vec::new();
    for c in claims {
        log = append_raw(&log, c.kind(), c.fields().clone(), at, TENANT).map_err(|e| e.to_string())?;
    }
    Ok(log)
}

/// One `commit_seen` per commit from the intent's sha to HEAD, oldest first,
/// with its observations. They are read with the laptop's helpers, so the
/// pipeline stamps the fingerprint the laptop stamped.
fn commit_facts(log: &mut Log, worktree: &Path, sha: &str, head: &str, at: f64) -> Result<(), String> {
    let ids = familiar_ai_git::commits_between(worktree, sha, head).map_err(|e| {
        words::could_not(
            &format!("the history does not reach the intent's commit {} ({e})", short(sha)),
            "check out the whole history (actions/checkout with fetch-depth: 0)",
        )
    })?;
    for id in ids {
        let w = what_to_read(log);
        let mut f: familiar_ai_facts::CommitSeen = familiar_ai_git::commit_seen(worktree, &id, &w.plan_test_files, &w.anchors)
            .map_err(|e| e.to_string())?
            .into();
        f.touches_code = f.files.iter().any(|p| !w.plan_test_files.contains(p) && !commits::is_familiars_own(p));
        *log = familiar_ai_facts::append(log, &f, at, TENANT).map_err(|e| e.to_string())?;
        for o in commits::observations(worktree, &id, &w.all_tests).map_err(|e| e.to_string())? {
            *log = familiar_ai_facts::append(log, &o, at, TENANT).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

/// Make every move the rules ask for, until none is left or a run whose tests
/// must fail did not happen. A run that did not happen is not a failing test,
/// so the recheck stops there rather than count it.
fn moves(log: &mut Log, place: &Place, at: f64, readers: &[(String, String)]) -> Result<(Vec<Run>, Option<Stopped>), String> {
    let mut runs: Vec<Run> = Vec::new();
    loop {
        let v = View::new(log);
        let next = action::action_admissible(&v);
        let Some(first) = next.first() else {
            return Ok((runs, None));
        };
        // Each run is reserved, run and ended with its evidence, then translated through the
        // laptop's tables (plan §6.5); its results name it.
        let mut first = first.clone();
        let fields = match first.kind.as_str() {
            "run_ended" => {
                let (c, commit, teeth) = reserved_run(&v, &field(&first.fields, "run_id")).ok_or("the rules ended a run never reserved")?;
                let ran = ran_for(log, &runs, place, &c, &commit, teeth, readers);
                // a crash ran: CI reads it through its translation (c_dep's crash at T)
                let crashed = ran.translation.as_ref().and_then(|t| t.get("label")) == Some(&json!("crashed"));
                if (teeth || action::tests_first(&v, &c) == Some(commit.as_str())) && !crashed {
                    if let Some(why) = ran.not_run.clone() {
                        runs.push(Run { criterion_id: c.clone(), commit: commit.clone(), teeth, ran });
                        return Ok((runs, Some(Stopped { criterion_id: c, commit, teeth, why })));
                    }
                }
                let mut fields = first.fields.clone();
                fields.extend(ran.end.clone());
                if !runs.iter().any(|r| r.criterion_id == c && r.commit == commit && r.teeth == teeth) {
                    runs.push(Run { criterion_id: c, commit, teeth, ran });
                }
                fields
            }
            "test_log_translation" => {
                let (c, commit, teeth) = reserved_run(&v, &field(&first.fields, "run_id")).ok_or("the rules translated a run never reserved")?;
                let ran = runs.iter().find(|r| r.criterion_id == c && r.commit == commit && r.teeth == teeth).map(|r| &r.ran).ok_or("the rules translated a run that did not run")?;
                match &ran.translation {
                    Some(t) => {
                        let mut fields = first.fields.clone();
                        fields.extend(t.clone());
                        fields
                    }
                    // a run that never started has no translation: its result comes next
                    None => {
                        first = next.iter().find(|m| m.kind == "pass_please_pass").cloned().ok_or("the rules offered no result for a run that never started")?;
                        first.fields.clone()
                    }
                }
            }
            _ => first.fields.clone(),
        };
        *log = action::action_append(log, &first.kind, &fields, at, TENANT)?;
    }
}

/// The part, commit and teeth of the pipeline run reserved as `rid`.
fn reserved_run(v: &View, rid: &str) -> Option<(String, String, bool)> {
    let r = v.kind("run_reserved").find(|f| f.fields.get("run_id").and_then(Value::as_str) == Some(rid))?;
    let (c, commit) = (field(&r.fields, "criterion_id"), field(&r.fields, "commit"));
    let teeth = action::run_id(v, &c, &commit, true) == rid;
    Some((c, commit, teeth))
}

fn field(m: &Map<String, Value>, k: &str) -> String {
    m.get(k).and_then(Value::as_str).unwrap_or("").to_string()
}

/// One run per criterion, commit and teeth serves every test file, since the
/// command is the criterion's; the head is run in place.
fn ran_for(log: &Log, runs: &[Run], place: &Place, c: &str, commit: &str, teeth: bool, readers: &[(String, String)]) -> Ran {
    let v = View::new(log);
    if let Some(r) = runs.iter().find(|r| r.criterion_id == c && r.commit == commit && r.teeth == teeth) {
        return r.ran.clone();
    }
    let Some(command) = action::command(&v, c) else {
        return crate::door::verify_run::not_run("no test command is claimed".into());
    };
    let anchors: Vec<familiar_ai_git::Anchor> = what_to_read(log).anchors.into_iter().filter(|a| a.criterion_id == c).collect();
    let at = match (teeth, commit == place.head) {
        (true, _) if anchors.is_empty() => {
            return crate::door::verify_run::not_run("the plan names no lines to take back out".into());
        }
        (true, _) => At::PutBack(commit, &anchors),
        (false, true) => At::Here,
        (false, false) => At::Commit(commit),
    };
    // the reader CI's own dry run chose for the command's runner, if any (plan A)
    let reader = familiar_ai_run::details_for(&command).and_then(|d| readers.iter().find(|(r, _)| r == d.runner)).map(|(_, id)| id.as_str());
    run_at_read(place, &command, &at, reader, &action::test_files(&v, c))
}

/// CI's own dry runs (plan A, the pipeline's twin of the laptop's): one at HEAD for each stack
/// command of the plan whose runner has a probe kit and whose (root, runner) the adapter
/// committed at HEAD names. A project with no adapter committed has nothing calibrated to
/// re-check: its runs are read by the tables, as before. Each choice is the pipeline's own
/// (`ran_in: pipeline`), used for its runs; a line names each that differs from the committed
/// file.
pub fn pipeline_dry_runs(place: &Place, plan: &Log) -> (Vec<familiar_ai_facts::ReaderChosen>, Vec<String>) {
    let committed = committed_adapter(place);
    if committed.is_empty() {
        return (Vec::new(), Vec::new());
    }
    let root = crate::door::runner::root_in_repo(place.root);
    let plan_files: Vec<String> = plan.iter().filter(|f| f.kind == "test_planned").filter_map(|f| f.fields.get("file_path").and_then(Value::as_str).map(str::to_string)).collect();
    let (mut choices, mut notes): (Vec<familiar_ai_facts::ReaderChosen>, Vec<String>) = (Vec::new(), Vec::new());
    for f in plan.iter().rev().filter(|f| f.kind == "test_stack_detected") {
        let s = |k: &str| f.fields.get(k).and_then(Value::as_str).unwrap_or("");
        let command = s("test_command");
        let Some(runner) = crate::door::dry_run::runner_for(s("test_stack"), command) else { continue };
        if choices.iter().any(|c| c.runner == runner) {
            continue; // the latest stack line of a runner
        }
        let Some((_, _, named)) = committed.iter().find(|(r, n, _)| *r == root && n == runner) else { continue };
        let id = format!("d-pipeline-{runner}");
        let Ok(mut chosen) = crate::door::dry_run::dry_run(place, &id, command, place.head, &plan_files, &mut |_| true) else { continue };
        chosen.ran_in = Some("pipeline".to_string());
        if chosen.reader != *named {
            notes.push(format!("the adapter committed at {} reads {runner} with {named}; CI's own dry run chose {} ({}), and CI reads its runs with that", short(place.head), chosen.reader, chosen.why));
        }
        choices.push(chosen);
    }
    (choices, notes)
}

/// `.familiar/adapter` as committed at HEAD: (root, runner, reader) per line.
fn committed_adapter(place: &Place) -> Vec<(String, String, String)> {
    let rel = setup::paths_in_repository(place.root, place.worktree)
        .map(|(_, plan)| plan)
        .unwrap_or_else(|_| format!("{FAMILIAR_FOLDER}/{PLAN_FILE}"));
    let rel = match rel.rsplit_once('/') {
        Some((dir, _)) => format!("{dir}/{}", crate::door::ADAPTER_FILE),
        None => crate::door::ADAPTER_FILE.to_string(),
    };
    let Ok(bytes) = familiar_ai_git::file_at(place.worktree, place.head, &rel) else { return Vec::new() };
    String::from_utf8_lossy(&bytes)
        .lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .map(|v| {
            let s = |k: &str| v.get(k).and_then(Value::as_str).unwrap_or("").to_string();
            (s("root"), s("runner"), s("reader"))
        })
        .collect()
}

/// Write the facts into `.familiar/record`: all of them into an empty record,
/// as in CI, and only the pipeline's own into one that already holds facts.
pub(crate) fn write_record(checked: &Rechecked, start: &Path, around: &Surroundings) -> Result<(), String> {
    let root: PathBuf = find_project(start)
        .or_else(|| familiar_ai_git::worktree_root(start).ok().flatten())
        .unwrap_or_else(|| start.to_path_buf());
    let folder = root.join(FAMILIAR_FOLDER);
    std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
    let record = Record::open_waiting(&folder.join(RECORD_FILE), around.record_wait).map_err(|e| e.to_string())?;
    let from = if record.is_empty().map_err(|e| e.to_string())? {
        0
    } else {
        checked.log.iter().position(|f| f.kind == "pipeline_run_seen").unwrap_or(checked.log.len())
    };
    for f in &checked.log[from..] {
        let Fact { kind, fields, at, .. } = f;
        record.append_raw(kind, fields.clone(), *at, TENANT).map_err(|e| e.to_string())?;
    }
    Ok(())
}
