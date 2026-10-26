//! Write the binary's own facts after a commit lands, in the rules'
//! `binary_moves` order: `test_written` per planned test file, a laptop result per
//! own and touching test file, then `fingerprint_stamped` at each commit whose
//! results are all in. A result never runs the test command here; it reserves a
//! run that a detached child performs. The own tests' results come from the part's
//! own run and the touching tests' from a run of their own (the owner's plan of
//! 2026-10-21, step 0), so the own run's counts are its own tests' alone.


use familiar_ai_facts::{
    Fact, FactFields, FingerprintStamped, Log, Outcome, PassPleasePass, RanIn, Record, RecordError, TestWritten,
};
use crate::door::runner::{reserve, Work};
use crate::door::runs::{eligibility, Eligibility};
use crate::input::Surroundings;
use familiar_ai_rules::criterion::{
    command, criterion_rev, epoch, mine, observed, own_tests, test_files, touches_tests_of,
    touching_command, touching_tests,
};
use familiar_ai_rules::lifecycle::runnable_criteria;
use familiar_ai_rules::moves::{owed, reruns};
use familiar_ai_rules::View;
use serde_json::Value;

fn text<'a>(f: &'a Fact, key: &str) -> Option<&'a str> {
    f.fields.get(key).and_then(Value::as_str)
}

/// The next fact the rules say is missing.
enum Next {
    TestWritten(TestWritten),
    /// A test file needing a result at one of the criterion's commits, under its rev; a
    /// touching test's comes from the touching run.
    Result { criterion_id: String, file_path: String, commit: String, rev: String, touching: bool },
    Stamp(FingerprintStamped),
}

/// What `binary_moves` would write next for the runnable criteria in plan order,
/// with real values; `None` when nothing is owed.
fn next(log: &Log) -> Option<Next> {
    let v = View::new(log);
    runnable_criteria(&v).into_iter().find_map(|a| next_of(&v, a))
}

fn next_of(v: &View, a: String) -> Option<Next> {
    let tip = v.last("commit_seen")?;
    if i64::try_from(tip.t).unwrap_or(i64::MAX) <= epoch(v, &a) {
        return None;
    }
    // Any commit of a's tests since its epoch counts, not only the tip.
    let e = epoch(v, &a);
    if v.kind("commit_seen").any(|f| i64::try_from(f.t).unwrap_or(i64::MAX) > e && touches_tests_of(v, &a, f)) {
        for fp in test_files(v, &a) {
            if !v.of("test_written", &a).any(|w| text(w, "file_path") == Some(&fp)) {
                return Some(Next::TestWritten(TestWritten {
                    criterion_id: a.clone(),
                    file_path: fp,
                    test_name: String::new(), // results are read per file, so no name is invented
                }));
            }
        }
    }
    let own = own_tests(v, &a);
    if own.is_empty() {
        return None;
    }
    let tip_id = text(tip, "commit")?.to_string();
    // The person's try_again runs its commit again (plan §6.7): what ran and was stamped there
    // before the answer is not read, so the re-run's results and stamp come after it.
    let again = reruns(v, &a);
    let fresh = |f: &Fact| text(f, "commit").and_then(|id| again.iter().find(|(x, _)| x == id)).is_none_or(|(_, answer)| f.t > answer.t);
    // Include did-not-run results: they are no proof but still a run this step made.
    let laptop: Vec<&Fact> = mine(v, &a, "pass_please_pass")
        .into_iter()
        .filter(|f| text(f, "ran_in") == Some("laptop") && !f.fields.get("with_anchor_reverted").and_then(Value::as_bool).unwrap_or(false) && fresh(f))
        .collect();
    // Stamp, oldest first, each unstamped commit whose results are all in and ran,
    // with the fingerprint observed there; a commit with no observation is not stamped.
    let stamps: Vec<&Fact> = mine(v, &a, "fingerprint_stamped").into_iter().filter(|f| fresh(f)).collect();
    let tests: Vec<String> = own.iter().chain(touching_tests(v, &a).iter()).map(|t| t.file_path.clone()).collect();
    for f in v.kind("commit_seen").filter(|f| i64::try_from(f.t).unwrap_or(i64::MAX) > e) {
        let Some(id) = text(f, "commit") else { continue };
        let here: Vec<&&Fact> = laptop.iter().filter(|r| text(r, "commit") == Some(id)).collect();
        let all_in = tests.iter().all(|fp| here.iter().any(|r| text(r, "file_path") == Some(fp)));
        if !all_in || here.iter().any(|r| text(r, "outcome") == Some("did_not_run")) {
            continue;
        }
        if stamps.iter().any(|s| text(s, "ran_in") == Some("laptop") && text(s, "commit") == Some(id)) {
            continue;
        }
        let Some(proof) = observed(v, &a, id).and_then(|o| text(o, "tests_hash")) else { continue };
        return Some(Next::Stamp(FingerprintStamped {
            criterion_id: a,
            commit: id.to_string(),
            proof_not_tampered: proof.to_string(),
            ran_in: RanIn::Laptop,
        }));
    }
    // The latest laptop stamp is the rules' proof, so after a late stamp at an
    // older commit the tip is stamped again.
    let tip_proof = observed(v, &a, &tip_id).and_then(|o| text(o, "tests_hash"));
    let latest = stamps.iter().rfind(|s| text(s, "ran_in") == Some("laptop"));
    if let (Some(proof), Some(s)) = (tip_proof, latest) {
        let tip_stamped = stamps.iter().any(|s| text(s, "ran_in") == Some("laptop") && text(s, "commit") == Some(&tip_id));
        if tip_stamped && text(s, "commit") != Some(&tip_id) && text(s, "proof_not_tampered") != Some(proof) {
            return Some(Next::Stamp(FingerprintStamped {
                criterion_id: a,
                commit: tip_id,
                proof_not_tampered: proof.to_string(),
                ran_in: RanIn::Laptop,
            }));
        }
    }
    // Owed commits oldest first, then the tip: a result for every own and touching test at each.
    let mut work: Vec<(String, String)> = owed(v, &a).into_iter().map(|w| (w.commit, w.criterion_rev)).collect();
    if !work.iter().any(|(c, _)| *c == tip_id) {
        work.push((tip_id.clone(), criterion_rev(v, &a)));
    }
    // A touching test that is also one of the own is the own run's.
    let touching: Vec<String> = touching_tests(v, &a).into_iter().map(|t| t.file_path).filter(|f| !own.iter().any(|o| o.file_path == *f)).collect();
    let files: Vec<(&String, bool)> = own.iter().map(|t| (&t.file_path, false)).chain(touching.iter().map(|f| (f, true))).collect();
    for (commit, rev) in work {
        for &(fp, touching) in &files {
            let seen = laptop
                .iter()
                .any(|r| text(r, "commit") == Some(&commit) && text(r, "file_path") == Some(fp));
            if !seen {
                return Some(Next::Result { criterion_id: a.clone(), file_path: fp.clone(), commit, rev, touching });
            }
        }
    }
    None
}

/// The `test_command` in force for the criterion; none means the `no_test_stack` hold. A
/// touching test runs under the touching command, or with the own tests when the stack's table
/// cannot express it.
fn command_for(log: &Log, criterion_id: &str, touching: bool) -> Option<String> {
    let v = View::new(log);
    touching.then(|| touching_command(&v, criterion_id)).flatten().or_else(|| command(&v, criterion_id))
}

/// A touching file's result is its own: another file's failure in the same run is not its failure.
pub(crate) fn touching_outcome(whole: Outcome, failing_tests: &[String], file_path: &str) -> Outcome {
    // No per-test ids: the result is the run's own, as today.
    if failing_tests.is_empty() {
        return whole;
    }
    // A failing test whose file part, a leading "./" stripped from both sides, is this file:
    // the failure is its own.
    let file = file_path.strip_prefix("./").unwrap_or(file_path);
    if failing_tests.iter().any(|id| {
        let path = id.split("::").next().unwrap_or(id);
        path.strip_prefix("./").unwrap_or(path) == file
    }) {
        return Outcome::Failed;
    }
    // Else a run that never ran stays did-not-run; anything else that had no failure here passed.
    if whole == Outcome::DidNotRun {
        whole
    } else {
        Outcome::Passed
    }
}

/// What one call to [`binary_facts`] did.
pub struct Moves {
    /// The facts written, in order.
    pub written: Vec<Fact>,
    /// Runs this call reserved, for the caller to launch after releasing the record.
    pub reserved: Vec<String>,
}

/// Write every fact the binary owes, in order, stopping when a result needs a
/// run, which it reserves. `log` grows with what is written.
pub fn binary_facts(record: &Record, log: &mut Log, at: f64, tenant: &str, around: &Surroundings) -> Result<Moves, RecordError> {
    let mut moves = Moves { written: Vec::new(), reserved: Vec::new() };
    // A dry run owed (plan A) is reserved first, so the parts' runs read with its choice.
    match crate::door::runner::reserve_dry_run(record, log, at, around) {
        Ok(Some(id)) => moves.reserved.push(id),
        Ok(None) => {}
        Err(m) => return Err(RecordError::Db(m)),
    }
    while let Some(step) = next(log) {
        let fact = match step {
            Next::TestWritten(tw) => record.append(&tw, at, tenant)?,
            Next::Result { criterion_id, file_path, commit, rev, touching } => {
                let Some(command) = command_for(log, &criterion_id, touching) else {
                    break;
                };
                let (outcome, run_id) = match eligibility(log, &criterion_id, &rev, &commit, &command, at) {
                    Eligibility::Settled { outcome, run_id } => {
                        let whole = match outcome {
                            "passed" => Outcome::Passed,
                            "failed" => Outcome::Failed,
                            _ => Outcome::DidNotRun,
                        };
                        // A touching file's result is its own: another file's failure in the same run is
                        // not its failure, so read the run's failing tests and pass the result through.
                        let outcome = if touching {
                            let failing: Vec<String> = log
                                .iter()
                                .rfind(|f| f.kind == "test_log_translation" && f.fields.get("run_id").and_then(Value::as_str) == Some(run_id.as_str()))
                                // only an ordinary test failure names every failing test; a collection
                                // error or a crash may not, so it keeps the run's own result
                                .filter(|f| f.fields.get("label").and_then(Value::as_str) == Some("tests_failed"))
                                .and_then(|f| f.fields.get("failing_tests").and_then(Value::as_array))
                                .map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_string)).collect())
                                .unwrap_or_default();
                            touching_outcome(whole, &failing, &file_path)
                        } else {
                            whole
                        };
                        (outcome, run_id)
                    }
                    Eligibility::Free { .. } => {
                        let work = Work { criterion_id, criterion_rev: rev, commit, command };
                        match reserve(record, log, &work, at, around) {
                            Ok(Some(id)) => moves.reserved.push(id),
                            Ok(None) => {}
                            Err(m) => return Err(RecordError::Db(m)),
                        }
                        break;
                    }
                    Eligibility::Live | Eligibility::Stalled => break,
                };
                let result = PassPleasePass {
                    criterion_id,
                    file_path,
                    test_name: None,
                    commit,
                    outcome,
                    ran_in: RanIn::Laptop,
                    with_anchor_reverted: None,
                };
                // run_id is a field of the fact's form, not of the typed struct (S2).
                let mut fields = result.to_fields()?;
                fields.insert("run_id".to_string(), Value::String(run_id));
                record.append_raw(PassPleasePass::KIND, fields, at, tenant)?
            }
            Next::Stamp(s) => record.append(&s, at, tenant)?,
        };
        log.push(fact.clone());
        moves.written.push(fact);
    }
    Ok(moves)
}

#[cfg(test)]
mod tests {
    use super::*;
    use familiar_ai_facts::{append, CommitSeen, TestsObserved};
    use familiar_ai_rules::sample::criterion_fixture;

    fn fixture() -> Log {
        criterion_fixture(&["c1"], true, false, 0.0, "t").expect("fixture")
    }

    #[test]
    fn nothing_is_owed_before_a_commit_lands() {
        assert!(next(&fixture()).is_none());
    }

    #[test]
    fn after_a_tests_commit_the_order_is_written_then_result_then_stamp() {
        let log = fixture();
        let log = append(
            &log,
            &CommitSeen {
                commit: "T1".into(),
                files: vec!["tests/test_c1.py".into()],
                touches_tests: true,
                touches_code: false,
                anchor_overlap: vec![],
                tests_hash: None,
                by: None,
            },
            0.0,
            "t",
        )
        .expect("commit");
        // the fingerprint is c1's observation beside the commit
        let log = append(&log, &TestsObserved { criterion_id: "c1".into(), criterion_rev: "r".into(), commit: "T1".into(), tests_hash: "S1".into() }, 0.0, "t").expect("observed");
        let Some(Next::TestWritten(tw)) = next(&log) else { panic!("test_written first") };
        assert_eq!(tw.file_path, "tests/test_c1.py");
        assert_eq!(tw.test_name, "");
        let log = append(&log, &tw, 0.0, "t").expect("written");
        let Some(Next::Result { file_path, commit, .. }) = next(&log) else { panic!("result next") };
        assert_eq!((file_path.as_str(), commit.as_str()), ("tests/test_c1.py", "T1"));
        let log = append(
            &log,
            &PassPleasePass {
                criterion_id: "c1".into(),
                file_path,
                test_name: None,
                commit,
                outcome: Outcome::Failed,
                ran_in: RanIn::Laptop,
                with_anchor_reverted: None,
            },
            0.0,
            "t",
        )
        .expect("result");
        // the touching test of c1 is next, then the stamp
        let Some(Next::Result { file_path, .. }) = next(&log) else { panic!("touching next") };
        assert_eq!(file_path, "tests/test_login_old.py");
        let log = append(
            &log,
            &PassPleasePass {
                criterion_id: "c1".into(),
                file_path,
                test_name: None,
                commit: "T1".into(),
                outcome: Outcome::Passed,
                ran_in: RanIn::Laptop,
                with_anchor_reverted: None,
            },
            0.0,
            "t",
        )
        .expect("touching result");
        let Some(Next::Stamp(s)) = next(&log) else { panic!("stamp next") };
        assert_eq!((s.commit.as_str(), s.proof_not_tampered.as_str()), ("T1", "S1"));
        let log = append(&log, &s, 0.0, "t").expect("stamp");
        assert!(next(&log).is_none());
        assert_eq!(command_for(&log, "c1", false).as_deref(), Some("python -m pytest tests/test_c1.py"), "no typed command: the derived is what runs");
        assert_eq!(command_for(&log, "c1", true).as_deref(), Some("python -m pytest tests/test_login_old.py"), "the touching test's own run");
    }

    /// A legacy `commit_seen.tests_hash` is never stamped without an observation.
    #[test]
    fn a_legacy_hash_is_never_stamped() {
        let mut log = append(
            &fixture(),
            &CommitSeen {
                commit: "T1".into(),
                files: vec!["tests/test_c1.py".into()],
                touches_tests: true,
                touches_code: false,
                anchor_overlap: vec![],
                tests_hash: Some("S1".into()),
                by: None,
            },
            0.0,
            "t",
        )
        .expect("commit");
        for _ in 0..4 {
            match next(&log) {
                Some(Next::TestWritten(tw)) => log = append(&log, &tw, 0.0, "t").expect("written"),
                Some(Next::Result { file_path, commit, .. }) => {
                    let r = PassPleasePass { criterion_id: "c1".into(), file_path, test_name: None, commit, outcome: Outcome::Failed, ran_in: RanIn::Laptop, with_anchor_reverted: None };
                    log = append(&log, &r, 0.0, "t").expect("result");
                }
                Some(Next::Stamp(s)) => panic!("stamped from a legacy hash: {}", s.proof_not_tampered),
                _ => break,
            }
        }
        assert!(next(&log).is_none());
    }

    /// A late result at an older commit is stamped there, then the tip is stamped
    /// again so the latest stamp stays the tip's.
    #[test]
    fn a_late_stamp_at_an_older_commit_leaves_the_tip_the_latest_proof() {
        let commit = |log: &Log, id: &str, files: &[&str], tests: bool, hash: &str| {
            let cs = CommitSeen { commit: id.into(), files: files.iter().map(|f| f.to_string()).collect(), touches_tests: tests, touches_code: !tests, anchor_overlap: vec![], tests_hash: None, by: None };
            let log = append(log, &cs, 0.0, "t").expect("commit");
            append(&log, &TestsObserved { criterion_id: "c1".into(), criterion_rev: "r".into(), commit: id.into(), tests_hash: hash.into() }, 0.0, "t").expect("observed")
        };
        let result = |log: &Log, file: &str, id: &str, outcome: Outcome| {
            let r = PassPleasePass { criterion_id: "c1".into(), file_path: file.into(), test_name: None, commit: id.into(), outcome, ran_in: RanIn::Laptop, with_anchor_reverted: None };
            append(log, &r, 0.0, "t").expect("result")
        };
        let log = commit(&fixture(), "T1", &["tests/test_c1.py"], true, "S1");
        let log = commit(&log, "F1", &["src/login.py"], false, "S2");
        let tw = TestWritten { criterion_id: "c1".into(), file_path: "tests/test_c1.py".into(), test_name: String::new() };
        let log = append(&log, &tw, 0.0, "t").expect("written");
        // the tip's results and stamp came first; T1's results arrive late
        let mut log = log;
        for f in ["tests/test_c1.py", "tests/test_login_old.py"] {
            log = result(&log, f, "F1", Outcome::Passed);
        }
        let tip = FingerprintStamped { criterion_id: "c1".into(), commit: "F1".into(), proof_not_tampered: "S2".into(), ran_in: RanIn::Laptop };
        log = append(&log, &tip, 0.0, "t").expect("tip stamp");
        log = result(&log, "tests/test_c1.py", "T1", Outcome::Failed);
        log = result(&log, "tests/test_login_old.py", "T1", Outcome::Passed);
        let Some(Next::Stamp(s)) = next(&log) else { panic!("the late result is stamped where it ran") };
        assert_eq!((s.commit.as_str(), s.proof_not_tampered.as_str()), ("T1", "S1"));
        let log = append(&log, &s, 0.0, "t").expect("T1 stamp");
        let Some(Next::Stamp(s)) = next(&log) else { panic!("the tip is stamped again, the latest proof") };
        assert_eq!((s.commit.as_str(), s.proof_not_tampered.as_str()), ("F1", "S2"));
        let log = append(&log, &s, 0.0, "t").expect("tip again");
        assert!(next(&log).is_none());
    }
}

#[cfg(test)]
mod touching_outcome_tests {
    use super::touching_outcome;
    use familiar_ai_facts::Outcome;

    // (a) the run failed, but the failure is another file's: this file passes.
    #[test]
    fn another_files_failure_is_not_its_failure() {
        assert_eq!(
            touching_outcome(Outcome::Failed, &["tests/a.py::test_x".into()], "tests/b.py"),
            Outcome::Passed,
        );
    }

    // (b) the run failed and the failure is this file's: it fails.
    #[test]
    fn its_own_failure_is_its_failure() {
        assert_eq!(
            touching_outcome(Outcome::Failed, &["tests/a.py::test_x".into()], "tests/a.py"),
            Outcome::Failed,
        );
    }

    // (c) the run failed and no per-test ids are known: the result is the run's, unchanged.
    #[test]
    fn no_per_test_ids_leaves_the_whole_result() {
        assert_eq!(touching_outcome(Outcome::Failed, &[], "tests/a.py"), Outcome::Failed);
    }

    // (d) the run never ran and the failure is another file's: it stays did-not-run.
    #[test]
    fn a_matchless_failure_keeps_did_not_run() {
        assert_eq!(
            touching_outcome(Outcome::DidNotRun, &["tests/a.py::t".into()], "tests/b.py"),
            Outcome::DidNotRun,
        );
    }

    // (e) a failing id's file part, with a leading "./" stripped, is this file: it fails.
    #[test]
    fn a_dotted_failing_id_matches_the_file() {
        assert_eq!(
            touching_outcome(Outcome::DidNotRun, &["./tests/a.py::t".into()], "tests/a.py"),
            Outcome::Failed,
        );
    }

    // (f) the run passed and no per-test ids are known: it stays passed.
    #[test]
    fn no_per_test_ids_leaves_a_pass() {
        assert_eq!(touching_outcome(Outcome::Passed, &[], "tests/a.py"), Outcome::Passed);
    }
}
