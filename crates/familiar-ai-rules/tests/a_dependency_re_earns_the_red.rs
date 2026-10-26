//! c_dep: the dependency flow (plan §7.1, §7.2; lane S9a). A part whose run crashed at its
//! tests-first commit T, answered `a_dependency` by the person (t2), gets a dependency drafted
//! and strung just before it (t3). Once the dependency is green at G (t4), the part's red is
//! re-earned at the re-earn commit: B, the first commit after G that touches no tests, no code
//! and no anchor, when B's `commit_seen` came before t4, else G; fixed at t4 and never
//! re-evaluated. It moves only when the dependency is green, the part's tests are unchanged
//! there, and no commit between T and it touched the part's anchor. One re-earn per approval.
//! The holds the readings feed (crashed, miscoped) are S7's; this file reads the facts only.

use familiar_ai_facts::{append_raw, Log};
use familiar_ai_rules::dependency::{crash_unanswered, keeps_string, miscoped, re_earn, restring};
use familiar_ai_rules::hooks::{decide, Decision, Event};
use familiar_ai_rules::sample::{apply_move, criterion_fixture, DEPENDENCY};
use familiar_ai_rules::{action, criterion, lifecycle, moves, plan, View};
use serde_json::{json, Value};

const AT: f64 = 0.0;
const TENANT: &str = "_default_";
const TOUCHING: &str = "tests/test_login_old.py";

fn raw(log: &Log, kind: &str, fields: Value) -> Log {
    let Value::Object(m) = fields else { unreachable!() };
    append_raw(log, kind, m, AT, TENANT).unwrap_or_else(|e| panic!("append {kind}: {e}"))
}

fn test_of(c: &str) -> String {
    format!("tests/test_{c}.py")
}

fn place_of(c: &str) -> String {
    if c == "c1" { "src/login.py".to_string() } else { format!("src/dep_{c}.py") }
}

/// Each part's fingerprint never changes in these stories.
fn hash_of(c: &str) -> String {
    if c == "c1" { "S1".to_string() } else { format!("H{c}") }
}

/// A commit, observed for every part whose tests exist at it, as the binary writes it.
fn commit(log: &Log, id: &str, files: &[String], tests: bool, code: bool, overlap: &[&str]) -> Log {
    let mut log = raw(log, "commit_seen", json!({ "commit": id, "files": files, "touches_tests": tests, "touches_code": code, "anchor_overlap": overlap }));
    for c in plan::criteria(&View::new(&log)) {
        let seen = log.iter().any(|f| f.kind == "tests_observed" && f.criterion_id() == Some(c.as_str()));
        if seen || files.contains(&test_of(&c)) {
            log = raw(&log, "tests_observed", json!({ "criterion_id": c, "criterion_rev": "R", "commit": id, "tests_hash": hash_of(&c) }));
        }
    }
    log
}

fn tests_commit(log: &Log, id: &str, c: &str) -> Log {
    let log = commit(log, id, &[test_of(c)], true, false, &[]);
    if log.iter().any(|f| f.kind == "test_written" && f.criterion_id() == Some(c)) {
        return log;
    }
    raw(&log, "test_written", json!({ "criterion_id": c, "file_path": test_of(c), "test_name": "t1" }))
}

fn code_commit(log: &Log, id: &str, c: &str) -> Log {
    commit(log, id, &[place_of(c)], false, true, &[c])
}

/// B: a commit that touches no tests, no code and no anchor.
fn empty_commit(log: &Log, id: &str) -> Log {
    commit(log, id, &["README.md".to_string()], false, false, &[])
}

/// A run of `c` at `id`: reserved, ended, translated as `label`, the own test's result (and c1's
/// touching test, passed), naming the run; the stamp is `stamp`'s.
fn results(log: &Log, c: &str, id: &str, label: &str) -> Log {
    let word = if label == "passed" { "passed" } else { "failed" };
    let rid = format!("r-{c}-{id}-{}", log.len());
    let mut log = raw(log, "run_reserved", json!({ "run_id": rid, "criterion_id": c, "criterion_rev": "R", "commit": id, "command": "python -m pytest", "resources": ["tests"], "wait": "60", "by": "binary" }));
    log = raw(&log, "run_ended", json!({ "run_id": rid, "outcome": word, "end": "exited" }));
    log = raw(&log, "test_log_translation", json!({ "run_id": rid, "criterion_id": c, "commit": id, "by": "regex", "label": label, "evidence": "", "failing_tests": [], "classified_on": "full" }));
    log = raw(&log, "pass_please_pass", json!({ "criterion_id": c, "file_path": test_of(c), "commit": id, "outcome": word, "ran_in": "laptop", "with_anchor_reverted": false, "run_id": rid }));
    if c == "c1" {
        log = raw(&log, "pass_please_pass", json!({ "criterion_id": c, "file_path": TOUCHING, "commit": id, "outcome": "passed", "ran_in": "laptop", "with_anchor_reverted": false, "run_id": rid }));
    }
    log
}

fn stamp(log: &Log, c: &str, id: &str) -> Log {
    raw(log, "fingerprint_stamped", json!({ "criterion_id": c, "commit": id, "proof_not_tampered": hash_of(c), "ran_in": "laptop" }))
}

fn run(log: &Log, c: &str, id: &str, label: &str) -> Log {
    stamp(&results(log, c, id, label), c, id)
}

fn start_check(log: &Log, c: &str) -> Log {
    let (symbol, from, to) = if c == "c1" { ("check_password".to_string(), 40, 62) } else { (format!("fn_{c}"), 1, 9) };
    raw(log, "anchor_sha_matched", json!({ "criterion_id": c, "file_path": place_of(c), "symbol": symbol, "line_start": from, "line_end": to, "sha": "base", "lines_hash": "L0", "matched": true, "checked_at": "start" }))
}

/// The person's `a_dependency` on `c`'s crashed card; its `t` is the approval.
fn approve(log: &Log, c: &str) -> (Log, u64) {
    let log = raw(log, "hold_answered", json!({ "criterion_id": c, "hold": "crashed", "choice": "a_dependency", "typed": "" }));
    let t = log.last().map(|f| f.t).unwrap_or(0);
    (log, t)
}

/// The draft and the dependency's lines in one batch, as S9c's picker writes them: `d` strung
/// just before `x`.
fn draft(log: &Log, approval: u64, d: &str, x: &str) -> Log {
    let mut log = raw(log, "dependency_drafted", json!({ "approval": approval, "criterion_id": d, "text": format!("{d}: what {x} needs"), "test_file": test_of(d) }));
    log = raw(&log, "criterion", json!({ "criterion_id": d, "text": format!("{d}: what {x} needs"), "path": "full", "behavior_id": "b1", "promise_id": "p1" }));
    let place = json!({ "file_path": place_of(d), "symbol": format!("fn_{d}"), "line_start": 1, "line_end": 9 });
    log = raw(&log, "place_chosen", json!({ "criterion_id": d, "places": [place] }));
    log = raw(&log, "anchor_sha_matched", json!({ "criterion_id": d, "file_path": place_of(d), "symbol": format!("fn_{d}"), "line_start": 1, "line_end": 9, "sha": "base", "lines_hash": "L0", "matched": true, "checked_at": "planning" }));
    log = raw(&log, "test_planned", json!({ "criterion_id": d, "file_path": test_of(d), "will": "create" }));
    for (kind, fields) in restring(&View::new(&log), d, x) {
        log = raw(&log, kind, Value::Object(fields));
    }
    log = raw(&log, "criterion_assigned", json!({ "criterion_id": d, "agent_conversation_id": "a1" }));
    raw(&log, "fix_location_category", json!({ "criterion_id": d, "category": "this_repository", "where": "" }))
}

/// c1 started, its test committed alone at k1, and the run there crashed (t1).
fn crashed() -> Log {
    let log = criterion_fixture(&["c1"], true, false, AT, TENANT).expect("fixture");
    let log = start_check(&log, "c1");
    let log = tests_commit(&log, "k1", "c1");
    run(&log, "c1", "k1", "crashed")
}

/// After the crash: the approval, the draft of c3 before c1, c3 started, its test red at k2.
fn drafted() -> Log {
    let (log, t2) = approve(&crashed(), "c1");
    let log = draft(&log, t2, "c3", "c1");
    let log = start_check(&log, "c3");
    let log = tests_commit(&log, "k2", "c3");
    run(&log, "c3", "k2", "tests_failed")
}

fn v(log: &Log) -> View<'_> {
    View::new(log)
}

fn tests_first_ids(log: &Log, c: &str) -> Vec<String> {
    criterion::tests_first_commit(&v(log), c).map(|t| t.ids).unwrap_or_default()
}

fn owed(log: &Log, c: &str) -> Vec<String> {
    moves::owed(&v(log), c).into_iter().map(|w| w.commit).collect()
}

fn re_earn_at(log: &Log, c: &str) -> Option<String> {
    re_earn(&v(log), c).map(|r| r.commit)
}

fn seal_commit(log: &Log, c: &str) -> Option<String> {
    criterion::seal_raw(&v(log), c, criterion::Place::Laptop).and_then(|s| s.fields.get("commit")).and_then(Value::as_str).map(str::to_string)
}

fn state(log: &Log, c: &str) -> &'static str {
    lifecycle::state(&v(log), c)
}

#[test]
fn the_crash_is_no_seal_and_the_card_reads_open_until_answered() {
    let log = crashed();
    assert_eq!(state(&log, "c1"), "held", "S7: the crash before the seal is the person's card, which holds the part");
    assert!(seal_commit(&log, "c1").is_none(), "a crashed run seals nothing");
    assert!(crash_unanswered(&v(&log), "c1").is_some(), "the crash at T is the card's reading");
    let (log, _) = approve(&log, "c1");
    assert!(crash_unanswered(&v(&log), "c1").is_none(), "answered");
}

#[test]
fn the_batch_strings_the_dependency_first_and_the_part_waits() {
    let (log, t2) = approve(&crashed(), "c1");
    let log = draft(&log, t2, "c3", "c1");
    let v = v(&log);
    assert!(plan::planning_confirm_complete(&v), "the batch leaves the plan whole");
    assert_eq!(plan::string_before(&v, "c1").as_deref(), Some("c3"));
    assert_eq!(state(&log, "c1"), "waiting");
    assert_eq!(state(&log, "c3"), "testing");
}

/// Assumption 24: once the batch lands, the waiting part's commits are refused, naming the
/// dependency's next step.
#[test]
fn a_waiting_parts_commits_are_refused() {
    let (log, t2) = approve(&crashed(), "c1");
    let log = draft(&log, t2, "c3", "c1");
    for touches_code in [false, true] {
        match decide(&v(&log), &Event::TestsCommit { criterion_id: "c1".into(), touches_code }) {
            Decision::Refuse(r) => {
                assert_eq!(r.state, Some("waiting"));
                assert_eq!(r.criterion_id.as_deref(), Some("c1"));
            }
            d => panic!("a waiting part's commit is refused, got {d:?}"),
        }
    }
}

/// G with no B: c3 green at k3 is t4; c1's red is re-earned at k3, owed there, sealed there.
#[test]
fn green_at_g_with_no_b_re_earns_the_red_at_g() {
    let log = code_commit(&drafted(), "k3", "c3");
    let log = results(&log, "c3", "k3", "passed");
    assert_eq!(re_earn_at(&log, "c1"), None, "no t4 before the stamp");
    assert_eq!(tests_first_ids(&log, "c1"), ["k1"]);
    let log = stamp(&log, "c3", "k3");
    assert_eq!(state(&log, "c3"), "green");
    assert_eq!(re_earn_at(&log, "c1"), Some("k3".into()));
    assert_eq!(tests_first_ids(&log, "c1"), ["k3"], "c1's tests-first moves to G");
    assert_eq!(owed(&log, "c1"), ["k3"], "neither B nor G touches c1, so owed names it");
    assert_eq!(state(&log, "c1"), "testing", "its turn again");
    let log = run(&log, "c1", "k3", "tests_failed");
    assert_eq!(seal_commit(&log, "c1").as_deref(), Some("k3"), "sealed at G");
    assert_eq!(state(&log, "c1"), "sealed");
    assert!(owed(&log, "c1").is_empty());
    assert_eq!(action::tests_first(&v(&log), "c1"), Some("k1"), "CI re-earns from its own runs alone (S9d-ci): with no run on the head, nothing moves");
    // the fix on c1's own place, then green
    let log = code_commit(&log, "k4", "c1");
    assert_eq!(owed(&log, "c1"), ["k4"]);
    let log = run(&log, "c1", "k4", "passed");
    assert_eq!(state(&log, "c1"), "green");
}

/// B before t4: B landed after G and before G's stamp, so the red is re-earned at B.
#[test]
fn b_before_t4_re_earns_the_red_at_b() {
    let log = code_commit(&drafted(), "k3", "c3");
    let log = results(&log, "c3", "k3", "passed");
    let log = empty_commit(&log, "kb");
    let log = stamp(&log, "c3", "k3");
    assert_eq!(re_earn_at(&log, "c1"), Some("kb".into()));
    assert_eq!(tests_first_ids(&log, "c1"), ["kb"]);
    assert_eq!(owed(&log, "c1"), ["kb"]);
    let log = run(&log, "c1", "kb", "tests_failed");
    assert_eq!(seal_commit(&log, "c1").as_deref(), Some("kb"));
    assert_eq!(action::tests_first(&v(&log), "c1"), Some("k1"), "no CI run, no re-earn on CI (S9d-ci)");
}

/// B after t4 still gives G: the re-earn commit is fixed at t4 and never re-evaluated.
#[test]
fn b_after_t4_still_gives_g() {
    let log = code_commit(&drafted(), "k3", "c3");
    let log = run(&log, "c3", "k3", "passed");
    let log = empty_commit(&log, "kb");
    assert_eq!(re_earn_at(&log, "c1"), Some("k3".into()));
    assert_eq!(owed(&log, "c1"), ["k3"]);
    let log = run(&log, "c1", "k3", "tests_failed");
    let log = run(&log, "c1", "kb", "tests_failed");
    assert_eq!(re_earn_at(&log, "c1"), Some("k3".into()), "never re-chosen");
    assert_eq!(seal_commit(&log, "c1").as_deref(), Some("k3"));
    assert_eq!(action::tests_first(&v(&log), "c1"), Some("k1"), "no CI run, no re-earn on CI (S9d-ci)");
}

/// An anchor touch before G: a commit between T and G landed on c1's place, so the red it would
/// re-earn could hold c1's own fix; nothing moves.
#[test]
fn an_anchor_touch_before_g_moves_nothing() {
    let log = code_commit(&drafted(), "kx", "c1");
    let log = code_commit(&log, "k3", "c3");
    let log = run(&log, "c3", "k3", "passed");
    assert_eq!(state(&log, "c3"), "green");
    assert_eq!(re_earn_at(&log, "c1"), None, "condition 3");
    assert_eq!(tests_first_ids(&log, "c1"), ["k1"]);
    assert!(!owed(&log, "c1").contains(&"k3".to_string()));
    assert_eq!(action::tests_first(&v(&log), "c1"), Some("k1"));
}

/// Green at G: c1 passed once c3 was fixed, so c1 was c3. The reading S7's `miscoped` raises.
#[test]
fn green_at_g_reads_miscoped() {
    let log = code_commit(&drafted(), "k3", "c3");
    let log = run(&log, "c3", "k3", "passed");
    assert!(!miscoped(&v(&log), "c1"), "not before c1 ran at G");
    let log = run(&log, "c1", "k3", "passed");
    assert!(miscoped(&v(&log), "c1"));
    assert!(seal_commit(&log, "c1").is_none());
}

/// A crash at G: a new card reading; the first approval re-earns no second time. A second
/// approval drafts c4 before c1, and c1 re-earns from G.
#[test]
fn a_crash_at_g_opens_a_new_card_and_one_re_earn_per_approval() {
    let log = code_commit(&drafted(), "k3", "c3");
    let log = run(&log, "c3", "k3", "passed");
    let log = run(&log, "c1", "k3", "crashed");
    assert!(crash_unanswered(&v(&log), "c1").is_some(), "the crash at G is a new card");
    assert!(seal_commit(&log, "c1").is_none());
    let log = empty_commit(&log, "kb");
    assert_eq!(re_earn_at(&log, "c1"), Some("k3".into()), "the first approval's commit stays G");
    assert_eq!(tests_first_ids(&log, "c1"), ["k3"]);
    let (log, t2) = approve(&log, "c1");
    assert!(crash_unanswered(&v(&log), "c1").is_none());
    let log = draft(&log, t2, "c4", "c1");
    let order: Vec<Option<String>> = ["c3", "c4", "c1"].iter().map(|c| plan::string_before(&v(&log), c)).collect();
    assert_eq!(order, [None, Some("c3".into()), Some("c4".into())], "c3, c4, c1");
    let log = start_check(&log, "c4");
    let log = tests_commit(&log, "k5", "c4");
    let log = run(&log, "c4", "k5", "tests_failed");
    let log = code_commit(&log, "k6", "c4");
    let log = run(&log, "c4", "k6", "passed");
    assert_eq!(re_earn_at(&log, "c1"), Some("k6".into()), "the second approval's own re-earn");
    let log = run(&log, "c1", "k6", "tests_failed");
    assert_eq!(seal_commit(&log, "c1").as_deref(), Some("k6"));
}

/// Three levels deep and back: c1 needs c3, c3 needs c4, c4 needs c5; each is strung before the
/// one that needs it, and each re-earns its red at the green of the one before.
#[test]
fn three_levels_deep_and_back() {
    let (log, t2) = approve(&crashed(), "c1");
    let log = draft(&log, t2, "c3", "c1");
    let log = start_check(&log, "c3");
    let log = tests_commit(&log, "k2", "c3");
    let log = run(&log, "c3", "k2", "crashed");
    let (log, t2) = approve(&log, "c3");
    let log = draft(&log, t2, "c4", "c3");
    let log = start_check(&log, "c4");
    let log = tests_commit(&log, "k3", "c4");
    let log = run(&log, "c4", "k3", "crashed");
    let (log, t2) = approve(&log, "c4");
    let log = draft(&log, t2, "c5", "c4");
    let chain: Vec<Option<String>> = ["c5", "c4", "c3", "c1"].iter().map(|c| plan::string_before(&v(&log), c)).collect();
    assert_eq!(chain, [None, Some("c5".into()), Some("c4".into()), Some("c3".into())], "c5, c4, c3, c1");
    for c in ["c4", "c3", "c1"] {
        assert_eq!(state(&log, c), "waiting", "{c} waits");
    }
    // c5: red, fixed, green at k5
    let log = start_check(&log, "c5");
    let log = tests_commit(&log, "k4", "c5");
    let log = run(&log, "c5", "k4", "tests_failed");
    let log = code_commit(&log, "k5", "c5");
    let log = run(&log, "c5", "k5", "passed");
    assert_eq!(state(&log, "c5"), "green");
    // c4 re-earns at k5, then its fix k6
    assert_eq!(re_earn_at(&log, "c4"), Some("k5".into()));
    assert_eq!(re_earn_at(&log, "c3"), None, "c4 is not green yet");
    let log = run(&log, "c4", "k5", "tests_failed");
    assert_eq!(state(&log, "c4"), "sealed");
    let log = code_commit(&log, "k6", "c4");
    let log = run(&log, "c4", "k6", "passed");
    assert_eq!(state(&log, "c4"), "green");
    // c3 re-earns at k6, then its fix k7
    assert_eq!(re_earn_at(&log, "c3"), Some("k6".into()));
    let log = run(&log, "c3", "k6", "tests_failed");
    let log = code_commit(&log, "k7", "c3");
    let log = run(&log, "c3", "k7", "passed");
    assert_eq!(state(&log, "c3"), "green");
    // c1 re-earns at k7, then its fix k8
    assert_eq!(re_earn_at(&log, "c1"), Some("k7".into()));
    assert_eq!(owed(&log, "c1"), ["k7"]);
    let log = run(&log, "c1", "k7", "tests_failed");
    assert_eq!(seal_commit(&log, "c1").as_deref(), Some("k7"));
    let log = code_commit(&log, "k8", "c1");
    let log = run(&log, "c1", "k8", "passed");
    for c in ["c5", "c4", "c3", "c1"] {
        assert_eq!(state(&log, c), "green", "{c} green and back");
    }
}

/// The plan facts of a string: c1 its head, then each member strung in order.
fn string(members: &[&str]) -> Log {
    let mut log: Log = Vec::new();
    for c in members.iter().chain(["d", "d2"].iter()) {
        log = raw(&log, "criterion", json!({ "criterion_id": c, "text": c, "path": "full", "behavior_id": "b1", "promise_id": "p1" }));
    }
    for (i, c) in members.iter().enumerate() {
        log = if i == 0 {
            raw(&log, "criterion_head", json!({ "criterion_id": c }))
        } else {
            raw(&log, "criterion_strung_on_head", json!({ "criterion_id": c, "head": members[0] }))
        };
    }
    log
}

fn insert(log: &Log, d: &str, x: &str) -> Log {
    let mut out = log.clone();
    for (kind, fields) in restring(&View::new(log), d, x) {
        out = raw(&out, kind, Value::Object(fields));
    }
    assert!(keeps_string(&View::new(log), &View::new(&out)), "the insert check");
    out
}

/// The order of the string headed by the first criterion with no predecessor.
fn order(log: &Log, all: &[&str]) -> Vec<String> {
    let v = View::new(log);
    let on: Vec<&str> = all.iter().copied().filter(|c| plan::on_string(&v, c)).collect();
    let mut out: Vec<String> = on.iter().filter(|c| plan::string_before(&v, c).is_none()).map(|c| c.to_string()).collect();
    while let Some(next) = on.iter().find(|c| plan::string_before(&v, c).as_deref() == out.last().map(String::as_str)) {
        out.push(next.to_string());
    }
    out
}

#[test]
fn the_insert_into_a_member_keeps_the_members_after_it() {
    let all = ["c1", "c2", "c3", "c4", "d", "d2"];
    let log = insert(&string(&["c1", "c2", "c3", "c4"]), "d", "c3");
    assert_eq!(order(&log, &all), ["c1", "c2", "d", "c3", "c4"]);
}

#[test]
fn the_insert_before_a_head_makes_the_dependency_the_head() {
    let all = ["c3", "c4", "d", "d2"];
    let log = insert(&string(&["c3", "c4"]), "d", "c3");
    assert_eq!(order(&log, &all), ["d", "c3", "c4"]);
    let nested = insert(&log, "d2", "d");
    assert_eq!(order(&nested, &all), ["d2", "d", "c3", "c4"], "nested: the same insert, before d");
}

#[test]
fn a_restring_that_leaves_a_member_off_the_string_fails_the_insert_check() {
    let before = string(&["c3", "c4"]);
    // the head moved to d and c3 re-strung, but c4 left strung on c3, no longer a head
    let after = raw(&raw(&before, "criterion_head", json!({ "criterion_id": "d" })), "criterion_strung_on_head", json!({ "criterion_id": "c3", "head": "d" }));
    assert!(!keeps_string(&View::new(&before), &View::new(&after)));
}

/// The moves: the binary's draft after the approval, one batch, the dependency row's lines.
#[test]
fn the_binary_drafts_the_dependency_once_per_approval() {
    let (log, t2) = approve(&crashed(), "c1");
    let v0 = View::new(&log);
    let m = moves::binary_moves(&v0).into_iter().find(|m| m.kind == "dependency_drafted").expect("the draft is the binary's next move");
    assert_eq!(m.fields.get("approval"), Some(&json!(t2)));
    assert_eq!(m.fields.get("criterion_id"), Some(&json!(DEPENDENCY.criterion_id)));
    let log = apply_move(&log, &m, AT, TENANT).expect("the batch lands");
    let v1 = View::new(&log);
    assert!(plan::planning_confirm_complete(&v1));
    assert!(keeps_string(&v0, &v1));
    assert_eq!(plan::string_before(&v1, "c1").as_deref(), Some(DEPENDENCY.criterion_id));
    assert_eq!(lifecycle::state(&v1, "c1"), "waiting");
    assert!(!moves::binary_moves(&v1).iter().any(|m| m.kind == "dependency_drafted"), "once per approval");
}
