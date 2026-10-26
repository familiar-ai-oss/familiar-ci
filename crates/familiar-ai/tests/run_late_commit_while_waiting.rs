//! A tests-first commit that landed while its part waited behind the head is
//! run and stamped at that commit when the part's turn comes, exactly as a
//! late-fed one is (docs/decisions.md, "One stamp per criterion per run").
//! The part has two test files: the stamp needs a result for each, so the
//! commit stays owed until both have run there.

mod common;

use common::Project;
use familiar_ai_facts::Fact;
use serde_json::json;

const SESSION: &str = "6d1e2f30-4a5b-4c6d-8e7f-90a1b2c3d4e5";
const BYE: &str = "#!/bin/sh\nbye() { echo bye; }\nbye\n";
const C2_COMMAND: &str = "sh tests/test_c2.sh && sh tests/test_c2_more.sh";

/// A set-up project planned to c1 (anchor src/app.sh, test tests/test_c1.sh)
/// with c2 strung on it (anchor src/bye.sh, tests tests/test_c2.sh and
/// tests/test_c2_more.sh).
fn project(name: &str) -> Project {
    let p = Project::builder(name).session(SESSION).file("src/bye.sh", BYE).create();
    p.claim(&["intent_submitted", "--for", "0", "--text", "greet with hello and part with farewell"]);
    p.claim(&["behavior", "--behavior_id", "b1", "--text", "the greeting"]);
    p.claim(&["promise", "--promise_id", "p1", "--behavior_id", "b1", "--accepts", "a name", "--gives_back", "a greeting", "--refuses", "nothing", "--cases", r#"[{"case_id":"n1","kind":"normal","given":"a name","expect":"a greeting"},{"case_id":"o1","kind":"opposite","given":"no name","expect":"no greeting"}]"#]);
    p.claim(&["criterion", "--criterion_id", "c1", "--text", "greet says hello", "--path", "full", "--behavior_id", "b1", "--promise_id", "p1"]);
    p.cases_kept("c1"); // the cases card passed (plan cases-card, 2026-10-25) [draft names]
    p.claim(&["place_chosen", "--criterion_id", "c1", "--places", r#"[{"file_path":"src/app.sh","symbol":"greet","line_start":1,"line_end":3}]"#]);
    p.claim(&["test_planned", "--criterion_id", "c1", "--file_path", "tests/test_c1.sh", "--will", "create", "--test_command", "sh tests/test_c1.sh"]);
    p.claim(&["criterion", "--criterion_id", "c2", "--text", "bye says farewell", "--path", "full", "--behavior_id", "b1", "--promise_id", "p1"]);
    p.cases_kept("c2"); // the cases card passed (plan cases-card, 2026-10-25) [draft names]
    p.claim(&["place_chosen", "--criterion_id", "c2", "--places", r#"[{"file_path":"src/bye.sh","symbol":"bye","line_start":1,"line_end":3}]"#]);
    p.claim(&["test_planned", "--criterion_id", "c2", "--file_path", "tests/test_c2.sh", "--will", "create", "--test_command", C2_COMMAND]);
    p.claim(&["test_planned", "--criterion_id", "c2", "--file_path", "tests/test_c2_more.sh", "--will", "create", "--test_command", C2_COMMAND]);
    p.claim(&["test_stack_detected", "--project_root", ".", "--test_stack", "sh", "--test_command", "for f in tests/test_*.sh; do sh \"$f\" || exit 1; done"]);
    p.claim(&["criterion_head", "--criterion_id", "c1"]);
    p.claim(&["criterion_assigned", "--criterion_id", "c1"]);
    p.claim(&["fix_location_category", "--criterion_id", "c1", "--category", "this_repository", "--where", ""]);
    p.claim(&["criterion_strung_on_head", "--criterion_id", "c2", "--head", "c1"]);
    p.claim(&["criterion_assigned", "--criterion_id", "c2"]);
    p.claim(&["fix_location_category", "--criterion_id", "c2", "--category", "this_repository", "--where", ""]);
    p
}

/// The criterion's facts of a kind at a commit.
fn at<'a>(facts: &'a [Fact], kind: &str, c: &str, commit: &str) -> Vec<&'a Fact> {
    facts
        .iter()
        .filter(|f| f.kind == kind && f.fields.get("criterion_id") == Some(&json!(c)) && f.fields.get("commit") == Some(&json!(commit)))
        .collect()
}

/// Hook calls until `done` holds of the record (the runs are detached), or a minute passes.
fn until(p: &Project, done: impl Fn(&[Fact]) -> bool) -> Vec<Fact> {
    let end = std::time::Instant::now() + std::time::Duration::from_secs(60);
    loop {
        let facts = p.facts();
        if done(&facts) || std::time::Instant::now() > end {
            return facts;
        }
        std::thread::sleep(std::time::Duration::from_millis(300));
        p.bash("PostToolUse", "git status");
    }
}

/// Whether `c` is stamped at `commit`.
fn stamped(c: &'static str, commit: &str) -> impl Fn(&[Fact]) -> bool {
    let commit = commit.to_string();
    move |facts| !at(facts, "fingerprint_stamped", c, &commit).is_empty()
}

/// c2's laptop results at `commit`, a failed one for each of its two files,
/// and one stamp there carrying its observation.
fn c2_red_and_stamped_at(facts: &[Fact], commit: &str) {
    let kinds: Vec<String> = facts
        .iter()
        .filter(|f| matches!(f.kind, "commit_seen" | "pass_please_pass" | "fingerprint_stamped"))
        .map(|f| format!("{} {:?} {:?} {:?}", f.kind, f.fields.get("criterion_id"), f.fields.get("commit"), f.fields.get("file_path")))
        .collect();
    let observed = at(facts, "tests_observed", "c2", commit);
    assert!(!observed.is_empty(), "c2 observed at {commit}: {kinds:?}");
    for file in ["tests/test_c2.sh", "tests/test_c2_more.sh"] {
        let results: Vec<_> = at(facts, "pass_please_pass", "c2", commit)
            .into_iter()
            .filter(|r| r.fields.get("ran_in") == Some(&json!("laptop")) && r.fields.get("file_path") == Some(&json!(file)))
            .collect();
        assert_eq!(results.len(), 1, "one laptop result for c2's {file} at {commit}: {kinds:?}");
        assert_eq!(results[0].fields.get("outcome"), Some(&json!("failed")), "c2's {file} fails at {commit}");
    }
    let stamps = at(facts, "fingerprint_stamped", "c2", commit);
    assert_eq!(stamps.len(), 1, "c2 stamped at {commit}: {kinds:?}");
    let hash = observed.last().and_then(|o| o.fields.get("tests_hash"));
    assert_eq!(stamps[0].fields.get("proof_not_tampered"), hash, "the stamp is c2's observation at {commit}");
}

#[test]
fn a_tests_first_commit_landed_while_its_part_waited_is_run_and_stamped_there_at_its_turn() {
    let p = project("waited");
    // c1 tests first: red, sealed
    p.write("tests/test_c1.sh", "grep -q hello src/app.sh\n");
    p.commit("c1: tests");
    let t1 = p.head();
    until(&p, stamped("c1", &t1));
    // c2's tests committed alone while c1 is testing: c2 waits behind c1
    p.write("tests/test_c2.sh", "grep -q farewell src/bye.sh\n");
    p.write("tests/test_c2_more.sh", "grep -q 'farewell; }' src/bye.sh\n");
    p.commit("c2: tests");
    let t2 = p.head();
    assert_eq!(p.state_name("c2"), "waiting", "c2 waits behind c1 at {t2}");
    // c1's fix lands and c1 greens
    p.write("src/app.sh", "#!/bin/sh\ngreet() { echo hello; }\ngreet\n");
    p.commit("c1: fix");
    let f1 = p.head();
    until(&p, stamped("c1", &f1));
    // c2's turn: the catch-up runs c2's tests at its tests-first commit and stamps it there
    let facts = until(&p, stamped("c2", &t2));
    c2_red_and_stamped_at(&facts, &t2);
}

/// As in the lane that found it: c2 sealed red, then its tests and c1's
/// change under an announcement in one commit, so c1 (green) goes red again
/// and c2 waits behind it; at c2's turn its red run at that commit is stamped there.
#[test]
fn an_announced_test_landed_while_its_part_waited_is_run_and_stamped_there_at_its_turn() {
    let p = project("announced");
    p.write("tests/test_c1.sh", "grep -q hello src/app.sh\n");
    p.commit("c1: tests");
    let t1 = p.head();
    until(&p, stamped("c1", &t1));
    p.write("src/app.sh", "#!/bin/sh\ngreet() { echo hello; }\ngreet\n");
    p.commit("c1: fix");
    let f1 = p.head();
    until(&p, stamped("c1", &f1));
    p.write("tests/test_c2.sh", "grep -q farewell src/bye.sh\n");
    p.write("tests/test_c2_more.sh", "grep -q 'farewell; }' src/bye.sh\n");
    p.commit("c2: tests");
    let t2 = p.head();
    until(&p, stamped("c2", &t2));
    // both parts' tests change under an announcement, committed alone together
    p.claim(&["test_change_announced", "--criterion_id", "c1", "--reason", "greet says hello there"]);
    p.claim(&["test_change_announced", "--criterion_id", "c2", "--reason", "bye says farewell friend"]);
    p.write("tests/test_c1.sh", "grep -q 'hello there' src/app.sh\n");
    p.write("tests/test_c2.sh", "grep -q 'farewell friend' src/bye.sh\n");
    p.write("tests/test_c2_more.sh", "grep -q 'farewell friend; }' src/bye.sh\n");
    p.commit("c1, c2: tests changed");
    let a = p.head();
    until(&p, stamped("c1", &a));
    assert_eq!(p.state_name("c2"), "waiting", "c2 waits behind c1, red again at {a}");
    // c1's fix lands and c1 greens; then c2's turn
    p.write("src/app.sh", "#!/bin/sh\ngreet() { echo hello there; }\ngreet\n");
    p.commit("c1: fix again");
    let f2 = p.head();
    until(&p, stamped("c1", &f2));
    let facts = until(&p, stamped("c2", &a));
    c2_red_and_stamped_at(&facts, &a);
}
