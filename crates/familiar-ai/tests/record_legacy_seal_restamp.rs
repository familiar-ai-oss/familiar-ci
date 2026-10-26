//! An old binary's laptop stamp carries one legacy hash, so its seal reads
//! unsealed; on first use the replay appends a stamp with the observed
//! fingerprint at each such commit, oldest first, rewriting nothing.

mod common;
mod way_back;
use way_back::*;

use familiar_ai::TENANT;
use familiar_ai_facts::{CommitSeen, Fact, FingerprintStamped, Outcome, PassPleasePass, RanIn, Record, TestWritten};
use serde_json::Value;

fn record(p: &Sealed) -> Record {
    Record::open_waiting(&p.record_path(), std::time::Duration::from_secs(5)).expect("the record")
}

fn text<'a>(f: &'a Fact, k: &str) -> &'a str {
    f.fields.get(k).and_then(Value::as_str).unwrap_or("")
}

/// A commit made with no hook, plus the old binary's `commit_seen`, laptop
/// result and laptop stamp, each with the one legacy hash.
fn old_commit(p: &Sealed, message: &str, tests: bool, legacy: &str, outcome: Outcome) -> String {
    p.git(&["add", "-A"]);
    p.git(&["commit", "-q", "-m", message]);
    let id = p.git(&["rev-parse", "HEAD"]);
    let files: Vec<String> = p.git(&["show", "--name-only", "--format=", &id]).lines().map(str::to_string).collect();
    let r = record(p);
    r.append(
        &CommitSeen {
            commit: id.clone(),
            files,
            touches_tests: tests,
            touches_code: !tests,
            anchor_overlap: if tests { Vec::new() } else { vec!["c1".to_string()] },
            tests_hash: Some(legacy.to_string()),
            by: Some("agent".to_string()),
        },
        1.0,
        TENANT,
    )
    .expect("commit_seen");
    if tests {
        r.append(&TestWritten { criterion_id: "c1".into(), file_path: "tests/test_c1.sh".into(), test_name: String::new() }, 1.0, TENANT).expect("test_written");
    }
    r.append(
        &PassPleasePass { criterion_id: "c1".into(), file_path: "tests/test_c1.sh".into(), test_name: None, commit: id.clone(), outcome, ran_in: RanIn::Laptop, with_anchor_reverted: None },
        1.0,
        TENANT,
    )
    .expect("the result");
    r.append(&FingerprintStamped { criterion_id: "c1".into(), commit: id.clone(), proof_not_tampered: legacy.to_string(), ran_in: RanIn::Laptop }, 1.0, TENANT).expect("the legacy stamp");
    id
}

fn a_hook_call(p: &Sealed) -> String {
    p.bash("PostToolUse", "ls").to_string()
}

#[test]
fn an_old_records_legacy_seals_are_restamped_from_their_observations_and_the_proof_stands() {
    let p = Sealed::new("legacy-seal");
    p.claim(&["intent_submitted", "--for", "0", "--text", "greet with hello"]);
    p.claim(&["behavior", "--behavior_id", "b1", "--text", "the greeting"]);
    p.claim(&["promise", "--promise_id", "p1", "--behavior_id", "b1", "--accepts", "a name", "--gives_back", "a greeting", "--refuses", "nothing", "--cases", r#"[{"case_id":"n1","kind":"normal","given":"a name","expect":"a greeting"},{"case_id":"o1","kind":"opposite","given":"no name","expect":"no greeting"}]"#]);
    p.claim(&["criterion", "--criterion_id", "c1", "--text", "greet says hello", "--path", "full", "--behavior_id", "b1", "--promise_id", "p1"]);
    p.cases_kept("c1"); // the cases card passed (plan cases-card, 2026-10-25) [draft names]
    p.claim(&["place_chosen", "--criterion_id", "c1", "--places", r#"[{"file_path":"src/app.sh","symbol":"greet","line_start":1,"line_end":3}]"#]);
    p.claim(&["test_planned", "--criterion_id", "c1", "--file_path", "tests/test_c1.sh", "--will", "create"]);
    p.claim(&["test_stack_detected", "--project_root", ".", "--test_stack", "sh", "--test_command", "sh tests/test_c1.sh"]);
    p.claim(&["criterion_head", "--criterion_id", "c1"]);
    p.claim(&["criterion_assigned", "--criterion_id", "c1"]);
    p.claim(&["fix_location_category", "--criterion_id", "c1", "--category", "this_repository", "--where", ""]);
    std::fs::create_dir_all(p.repo().join("tests")).expect("tests");
    std::fs::write(p.repo().join("tests/test_c1.sh"), TEST).expect("test");
    // the old binary's one hash per commit covers the code, not c1's test file
    let base = p.git(&["rev-parse", "HEAD"]);
    let legacy = familiar_ai_git::tests_hash(&p.repo(), &base, &["src/app.sh".to_string()]).expect("hash");
    let seen_by_git = |id: &str| familiar_ai_git::tests_hash(&p.repo(), id, &["tests/test_c1.sh".to_string()]).expect("hash");
    let red = old_commit(&p, "c1 tests", true, &legacy, Outcome::Failed);
    std::fs::write(p.repo().join("src/app.sh"), "#!/bin/sh\ngreet() { echo hello; }\ngreet\n").expect("fix");
    let fix = old_commit(&p, "c1 the fix", false, &legacy, Outcome::Passed);
    assert_ne!(legacy, seen_by_git(&red), "the legacy hash is not c1's own fingerprint");
    let before = record(&p).read_all().expect("read").len();

    let told = a_hook_call(&p);
    let log = record(&p).read_all().expect("read");
    let obs: Vec<&Fact> = log.iter().filter(|f| f.kind == "tests_observed").collect();
    assert_eq!(obs.iter().map(|f| text(f, "commit")).collect::<Vec<_>>(), [red.as_str(), fix.as_str()], "every commit observed, oldest first; the hook told {told}");
    // a new laptop stamp at the tests-first commit and the fix, each the observation there
    let new_stamps: Vec<&Fact> = log[before..].iter().filter(|f| f.kind == "fingerprint_stamped" && text(f, "ran_in") == "laptop").collect();
    assert_eq!(new_stamps.iter().map(|s| text(s, "commit")).collect::<Vec<_>>(), [red.as_str(), fix.as_str()], "re-stamped oldest first: {new_stamps:?}");
    assert!(new_stamps.iter().all(|s| text(s, "proof_not_tampered") == seen_by_git(text(s, "commit"))), "{new_stamps:?}");
    // the legacy stamps stay
    assert_eq!(log.iter().filter(|f| f.kind == "fingerprint_stamped" && text(f, "proof_not_tampered") == legacy).count(), 2);
    let status = p.status();
    assert!(status.contains("\"greet says hello\" · locally green"), "{status}");
    assert!(!status.contains("tampered") && !status.contains("test comes first"), "{status}");

    // a second hook call: nothing more is appended
    let n = record(&p).read_all().expect("read").len();
    a_hook_call(&p);
    assert_eq!(record(&p).read_all().expect("read").len(), n, "replayed once");
}

#[test]
fn a_record_whose_stamps_are_its_observations_gets_no_new_stamp() {
    let p = Sealed::green("legacy-none");
    let stamps = || record(&p).read_by_kind("fingerprint_stamped").expect("read").len();
    let n = stamps();
    a_hook_call(&p);
    a_hook_call(&p);
    assert_eq!(stamps(), n, "no legacy stamp, nothing re-stamped");
}
