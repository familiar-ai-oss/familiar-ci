//! An old record (one `tests_hash` per `commit_seen`, no observation) is
//! replayed from git on the first hook call, and the laptop's next stamp
//! carries the observed fingerprint.

mod common;
mod way_back;
use way_back::*;

use familiar_ai::TENANT;
use familiar_ai_facts::{CommitSeen, Fact, Record};
use serde_json::Value;

fn record(p: &Sealed) -> Record {
    Record::open_waiting(&p.record_path(), std::time::Duration::from_secs(5)).expect("the record")
}

fn text<'a>(f: &'a Fact, k: &str) -> &'a str {
    f.fields.get(k).and_then(Value::as_str).unwrap_or("")
}

/// A commit made with no hook, and the old binary's `commit_seen` for it with
/// one legacy hash the observations must not repeat.
fn old_commit(p: &Sealed, message: &str, tests: bool, hash: &str) -> String {
    p.git(&["add", "-A"]);
    p.git(&["commit", "-q", "-m", message]);
    let id = p.git(&["rev-parse", "HEAD"]);
    let files: Vec<String> = p.git(&["show", "--name-only", "--format=", &id]).lines().map(str::to_string).collect();
    let old = CommitSeen {
        commit: id.clone(),
        files,
        touches_tests: tests,
        touches_code: !tests,
        anchor_overlap: Vec::new(),
        tests_hash: Some(hash.to_string()),
        by: None,
    };
    record(p).append(&old, 1.0, TENANT).expect("the old binary's fact");
    id
}

fn a_hook_call(p: &Sealed) -> String {
    p.bash("PostToolUse", "ls").to_string()
}

#[test]
fn the_first_hook_call_observes_every_commit_oldest_first_and_the_stamp_carries_it() {
    let p = Sealed::new("replay");
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
    // the old binary's one hash: over c1's file, the value the observation must give
    let seen_by_git = |id: &str| familiar_ai_git::tests_hash(&p.repo(), id, &["tests/test_c1.sh".to_string()]).expect("hash");
    let t1 = old_commit(&p, "c1 tests", true, "legacy-not-a-sha");
    std::fs::write(p.repo().join("tests/test_c1.sh"), REWORDED).expect("reworded");
    let t2 = old_commit(&p, "c1 tests reworded", true, "legacy-not-a-sha");
    assert_eq!(record(&p).read_by_kind("tests_observed").expect("read").len(), 0, "an old record: no observation");

    let told = a_hook_call(&p);
    let log = record(&p).read_all().expect("read");
    let obs: Vec<&Fact> = log.iter().filter(|f| f.kind == "tests_observed").collect();
    assert_eq!(obs.iter().map(|f| text(f, "commit")).collect::<Vec<_>>(), [t1.as_str(), t2.as_str()], "every commit, oldest first; the hook told {told}");
    assert!(obs.iter().all(|f| text(f, "criterion_id") == "c1" && text(f, "tests_hash") == seen_by_git(text(f, "commit"))), "read from git, never the legacy value");
    assert_ne!(seen_by_git(&t1), seen_by_git(&t2));
    assert!(obs.iter().all(|f| !text(f, "criterion_rev").is_empty()));
    // the laptop's next stamp carries the observed fingerprint
    let stamps: Vec<&Fact> = log.iter().filter(|f| f.kind == "fingerprint_stamped" && text(f, "ran_in") == "laptop").collect();
    assert!(!stamps.is_empty(), "the run at the tests-first commit was stamped: {told}");
    assert!(stamps.iter().all(|s| text(s, "proof_not_tampered") == seen_by_git(text(s, "commit"))), "{stamps:?}");

    // a second hook call appends nothing
    a_hook_call(&p);
    assert_eq!(record(&p).read_by_kind("tests_observed").expect("read").len(), 2, "replayed once");
}
