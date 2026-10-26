//! `commit_seen` follows an anchor's shift, reports overlap and sets the
//! touches flags; `observe_tests` hashes each criterion's own tests.

mod common;

use common::TestRepo;
use familiar_ai_git::{commit_seen, observe_tests, tests_hash, Anchor};

fn anchor(criterion: &str, path: &str, ls: u64, le: u64, sha: &str) -> Anchor {
    Anchor {
        criterion_id: criterion.to_string(),
        file_path: path.to_string(),
        line_start: ls,
        line_end: le,
        sha: sha.to_string(),
    }
}

/// One criterion's observed hash at `commit`, read through `observe_tests`.
fn observed(repo: &TestRepo, commit: &str, criterion: &str, files: &[&str]) -> Option<String> {
    let crit = vec![(criterion.to_string(), files.iter().map(|f| f.to_string()).collect())];
    let mut out = observe_tests(&repo.dir, commit, &crit).unwrap();
    assert_eq!(out.len(), 1);
    let (id, hash) = out.remove(0);
    assert_eq!(id, criterion);
    hash
}

fn strings(files: &[&str]) -> Vec<String> {
    files.iter().map(|f| f.to_string()).collect()
}

// The fingerprint is per criterion, from `observe_tests`.
#[test]
fn code_commit_after_tests_commit_same_tests_hash() {
    let repo = TestRepo::new();
    repo.write("tests/test_c1.py", b"def test_t1():\n    assert False\n");
    let t1 = repo.commit("T1: c1's tests alone");
    repo.write("src/login.py", b"def check_password():\n    return True\n");
    let k1 = repo.commit("K1: c1's code");

    let active = vec!["tests/test_c1.py".to_string()];
    let seen_t = commit_seen(&repo.dir, &t1, &active, &[]).unwrap();
    let seen_k = commit_seen(&repo.dir, &k1, &active, &[]).unwrap();

    assert!(seen_t.touches_tests && !seen_t.touches_code);
    assert!(!seen_k.touches_tests && seen_k.touches_code);
    // The code commit leaves the tests alone, so its tests hash equals the
    // previous commit's.
    let h_t = observed(&repo, &t1, "c1", &["tests/test_c1.py"]);
    let h_k = observed(&repo, &k1, "c1", &["tests/test_c1.py"]);
    assert!(h_t.is_some());
    assert_eq!(h_t, h_k);
}

// The fingerprint is per criterion, from `observe_tests`.
#[test]
fn later_edit_of_test_file_changes_hash() {
    let repo = TestRepo::new();
    repo.write("tests/t.py", b"a\n");
    let t1 = repo.commit("tests");
    let active = vec!["tests/t.py".to_string()];
    let h1 = observed(&repo, &t1, "c1", &["tests/t.py"]);
    assert!(h1.is_some());

    repo.write("tests/t.py", b"a!\n");
    let t2 = repo.commit("tamper");
    let seen = commit_seen(&repo.dir, &t2, &active, &[]).unwrap();
    assert_ne!(h1, observed(&repo, &t2, "c1", &["tests/t.py"]));
    assert!(seen.touches_tests && !seen.touches_code);
}

// The fingerprint is per criterion, from `observe_tests`.
#[test]
fn before_any_test_file_it_is_absent() {
    let repo = TestRepo::new();
    repo.write("src/x.py", b"x\n");
    let c1 = repo.commit("code only");
    assert_eq!(observed(&repo, &c1, "c1", &["tests/t.py"]), None);
}

#[test]
fn two_criteria_in_one_commit_each_get_their_own_hash() {
    let repo = TestRepo::new();
    repo.write("tests/test_c1.py", b"def test_c1():\n    assert False\n");
    repo.write("tests/test_c2.py", b"def test_c2():\n    assert False\n");
    let t = repo.commit("T: both criteria's tests");
    let crits = vec![
        ("c1".to_string(), strings(&["tests/test_c1.py"])),
        ("c2".to_string(), strings(&["tests/test_c2.py"])),
    ];
    let out = observe_tests(&repo.dir, &t, &crits).unwrap();
    let c1_alone = tests_hash(&repo.dir, &t, &strings(&["tests/test_c1.py"])).unwrap();
    let c2_alone = tests_hash(&repo.dir, &t, &strings(&["tests/test_c2.py"])).unwrap();
    assert_eq!(
        out,
        vec![("c1".to_string(), Some(c1_alone.clone())), ("c2".to_string(), Some(c2_alone.clone()))]
    );
    assert_ne!(c1_alone, c2_alone);
}

#[test]
fn an_edit_of_one_criterions_file_leaves_the_other_hash() {
    let repo = TestRepo::new();
    repo.write("tests/test_c1.py", b"def test_c1():\n    assert False\n");
    repo.write("tests/test_c2.py", b"def test_c2():\n    assert False\n");
    let t = repo.commit("T: both criteria's tests");
    repo.write("tests/test_c1.py", b"def test_c1():\n    assert True\n");
    let e = repo.commit("E: c1's test edited");
    let crits = vec![
        ("c1".to_string(), strings(&["tests/test_c1.py"])),
        ("c2".to_string(), strings(&["tests/test_c2.py"])),
    ];
    let before = observe_tests(&repo.dir, &t, &crits).unwrap();
    let after = observe_tests(&repo.dir, &e, &crits).unwrap();
    assert!(before[0].1.is_some() && after[0].1.is_some());
    assert_ne!(before[0].1, after[0].1);
    assert!(before[1].1.is_some());
    assert_eq!(before[1].1, after[1].1);
}

#[test]
fn a_criterion_with_no_file_at_the_commit_is_not_observed() {
    let repo = TestRepo::new();
    repo.write("tests/test_c1.py", b"def test_c1():\n    assert False\n");
    let t = repo.commit("T: c1's tests only");
    let crits = vec![
        ("c1".to_string(), strings(&["tests/test_c1.py"])),
        ("c2".to_string(), strings(&["tests/test_c2.py", "tests/test_c2b.py"])),
    ];
    let out = observe_tests(&repo.dir, &t, &crits).unwrap();
    assert_eq!(out.len(), 2);
    assert_eq!(out[0].0, "c1");
    assert!(out[0].1.is_some());
    assert_eq!(out[1], ("c2".to_string(), None));
}

#[test]
fn order_and_duplicates_of_files_do_not_change_the_hash() {
    let repo = TestRepo::new();
    repo.write("tests/a.py", b"a\n");
    repo.write("tests/b.py", b"b\n");
    let t = repo.commit("T: two files of one criterion");
    let one = observed(&repo, &t, "c1", &["tests/a.py", "tests/b.py"]);
    let other = observed(&repo, &t, "c1", &["tests/b.py", "tests/a.py", "tests/b.py"]);
    assert!(one.is_some());
    assert_eq!(one, other);
    assert_eq!(one, Some(tests_hash(&repo.dir, &t, &strings(&["tests/a.py", "tests/b.py"])).unwrap()));
}

#[test]
fn mixed_commit_touches_both() {
    let repo = TestRepo::new();
    repo.write("tests/t.py", b"a\n");
    repo.commit("setup");
    repo.write("tests/t.py", b"b\n");
    repo.write("src/x.py", b"x\n");
    let c = repo.commit("mixed");
    let plan = vec!["tests/t.py".to_string()];
    let seen = commit_seen(&repo.dir, &c, &plan, &[]).unwrap();
    assert!(seen.touches_tests && seen.touches_code);
    assert_eq!(seen.files, vec!["src/x.py", "tests/t.py"]);
}

#[test]
fn anchor_overlap_through_a_shift() {
    // An earlier commit inserts two lines above the anchor. A later commit
    // changing the anchor's lines at their new numbers overlaps; one changing
    // the lines now at the old numbers does not.
    let repo = TestRepo::new();
    repo.write(
        "src/login.py",
        b"def f():\n    a\n    b\n    c\n    d\n", // anchor: lines 3..=4 (b, c)
    );
    let base = repo.commit("base");

    // Earlier commit: insert two lines at the top (lines shift to 5..=6).
    repo.write(
        "src/login.py",
        b"h1\nh2\ndef f():\n    a\n    b\n    c\n    d\n",
    );
    let shift = repo.commit("add header lines above");

    // The shifting commit does not overlap the anchor's own lines.
    let a = anchor("c1", "src/login.py", 3, 4, &base);
    let seen_shift = commit_seen(&repo.dir, &shift, &[], std::slice::from_ref(&a)).unwrap();
    assert!(seen_shift.anchor_overlap.is_empty());

    // A commit changing the anchor's lines at their new numbers (5..=6).
    repo.write(
        "src/login.py",
        b"h1\nh2\ndef f():\n    a\n    B!\n    C!\n    d\n",
    );
    let hit = repo.commit("change the anchor lines");
    let seen_hit = commit_seen(&repo.dir, &hit, &[], std::slice::from_ref(&a)).unwrap();
    assert_eq!(seen_hit.anchor_overlap, vec!["c1"]);

    // A commit changing the lines now sitting at the old numbers (3..=4):
    // "def f():" and "    a" — outside the anchor's true lines.
    repo.git(&["checkout", "-q", &shift]);
    repo.write(
        "src/login.py",
        b"h1\nh2\ndef G():\n    A!\n    b\n    c\n    d\n",
    );
    let miss = repo.commit("change the old-numbered lines");
    let seen_miss = commit_seen(&repo.dir, &miss, &[], &[a]).unwrap();
    assert!(seen_miss.anchor_overlap.is_empty());
}

#[test]
fn anchor_overlap_variants() {
    let repo = TestRepo::new();
    repo.write("src/m.py", b"1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n");
    let base = repo.commit("base");
    let a = |id: &str, ls, le| anchor(id, "src/m.py", ls, le, &base);

    // Commit changing a file outside any anchor: no overlap.
    repo.write("src/other.py", b"x\n");
    let c_out = repo.commit("outside any anchor");
    let seen = commit_seen(&repo.dir, &c_out, &[], &[a("c1", 2, 5)]).unwrap();
    assert!(seen.anchor_overlap.is_empty());
    assert_eq!(seen.files, vec!["src/other.py"]);

    // Commit changing the anchor's file but outside its lines.
    repo.git(&["checkout", "-q", &base]);
    repo.write("src/m.py", b"1\n2\n3\n4\n5\n6\n7\n8\n9\nTEN\n");
    let c_file = repo.commit("same file, outside the lines");
    let seen = commit_seen(&repo.dir, &c_file, &[], &[a("c1", 2, 5)]).unwrap();
    assert!(seen.anchor_overlap.is_empty());

    // Commit inside the lines.
    repo.git(&["checkout", "-q", &base]);
    repo.write("src/m.py", b"1\n2\nTHREE\n4\n5\n6\n7\n8\n9\n10\n");
    let c_in = repo.commit("inside");
    let seen = commit_seen(&repo.dir, &c_in, &[], &[a("c1", 2, 5), a("c2", 8, 9)]).unwrap();
    assert_eq!(seen.anchor_overlap, vec!["c1"]);

    // Overlap list is sorted.
    repo.git(&["checkout", "-q", &base]);
    repo.write("src/m.py", b"1\n2\n3\n4\n5\n6\n7\n8\nNINE\n10\n");
    let c_both = repo.commit("two anchors");
    let seen = commit_seen(
        &repo.dir,
        &c_both,
        &[],
        &[a("c9", 8, 9), a("c3", 9, 10)],
    )
    .unwrap();
    assert_eq!(seen.anchor_overlap, vec!["c3", "c9"]);
}

#[test]
fn insertion_after_anchor_last_line_overlaps() {
    let repo = TestRepo::new();
    repo.write("f.py", b"def f():\n    a\n    b\n");
    let base = repo.commit("base");
    // Append a statement inside the function (insertion after old line 3,
    // the anchor's last line 2..=3).
    repo.write("f.py", b"def f():\n    a\n    b\n    c\n");
    let c = repo.commit("add statement at end");
    let seen = commit_seen(
        &repo.dir,
        &c,
        &[],
        &[anchor("c1", "f.py", 2, 3, &base)],
    )
    .unwrap();
    assert_eq!(seen.anchor_overlap, vec!["c1"]);
}
