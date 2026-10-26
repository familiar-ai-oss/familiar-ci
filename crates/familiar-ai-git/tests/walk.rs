//! `commits_between` matches `git rev-list --reverse`, and `find_by_tests_hash`
//! finds a commit after a history rewrite and none when the content never
//! existed.

mod common;

use common::TestRepo;
use familiar_ai_git::{commits_between, find_by_tests_hash, tests_hash};

#[test]
fn commits_between_matches_rev_list_reverse() {
    let repo = TestRepo::new();
    repo.write("a.txt", b"1\n");
    let c1 = repo.commit("c1");
    repo.write("a.txt", b"2\n");
    repo.commit("c2");
    repo.write("a.txt", b"3\n");
    repo.commit("c3");
    repo.write("a.txt", b"4\n");
    let c4 = repo.commit("c4");

    let mine = commits_between(&repo.dir, &c1, &c4).unwrap();
    assert_eq!(mine, repo.rev_list(&c1, &c4));
    assert_eq!(mine.len(), 3);
    assert_eq!(mine.last().unwrap(), &c4);
}

#[test]
fn find_by_tests_hash_survives_history_rewrite() {
    // base -> T (tests) -> K (code). Rewrite: amend T's message (every
    // commit id after changes). The tests-first commit must still be found
    // by content.
    let repo = TestRepo::new();
    repo.write("src/code.py", b"x = 0\n");
    let base = repo.commit("base");
    repo.write("tests/t.py", b"def test_t():\n    assert False\n");
    repo.commit("T: tests");
    repo.write("src/code.py", b"x = 1\n");
    let tip = repo.commit("K: code");

    let paths = vec!["tests/t.py".to_string()];
    let seal = tests_hash(&repo.dir, &tip, &paths).unwrap();
    let found_before = find_by_tests_hash(&repo.dir, &base, &tip, &paths, &seal).unwrap();
    assert!(found_before.is_some());

    // Rewrite: replay the same two changes with different messages.
    let repo2 = TestRepo::new();
    repo2.write("src/code.py", b"x = 0\n");
    let base2 = repo2.commit("base");
    repo2.write("tests/t.py", b"def test_t():\n    assert False\n");
    repo2.commit("T: tests, rewritten");
    repo2.write("src/code.py", b"x = 1\n");
    let tip2 = repo2.commit("K: code, rewritten");
    assert_ne!(tip, tip2);

    let found_after = find_by_tests_hash(&repo2.dir, &base2, &tip2, &paths, &seal).unwrap();
    assert!(found_after.is_some());
    // Found by content, not id: the id changed, the hash did not.
    assert_ne!(found_before, found_after);
    assert_eq!(
        tests_hash(&repo2.dir, found_after.as_ref().unwrap(), &paths).unwrap(),
        seal
    );
}

#[test]
fn find_by_tests_hash_none_when_content_never_existed() {
    let repo = TestRepo::new();
    repo.write("tests/t.py", b"a\n");
    let c1 = repo.commit("v1");
    repo.write("tests/t.py", b"b\n");
    let c2 = repo.commit("v2");
    let paths = vec!["tests/t.py".to_string()];
    // A hash of content that was never committed.
    let ghost = tests_hash(&repo.dir, &c2, &["tests/other.py".to_string()]).unwrap();
    assert_eq!(
        find_by_tests_hash(&repo.dir, &c1, &c2, &paths, &ghost).unwrap(),
        None
    );
}

#[test]
fn find_by_tests_hash_returns_oldest_match() {
    let repo = TestRepo::new();
    repo.write("seed.txt", b"s\n");
    let c0 = repo.commit("seed");
    repo.write("tests/t.py", b"same\n");
    let c1 = repo.commit("tests first");
    repo.write("src/x.py", b"x\n");
    let c2 = repo.commit("code after");
    let paths = vec!["tests/t.py".to_string()];
    let seal = tests_hash(&repo.dir, &c2, &paths).unwrap();
    // Both c1 and c2 have the same test content; the oldest is found.
    let found = find_by_tests_hash(&repo.dir, &c0, &c2, &paths, &seal).unwrap();
    assert_eq!(found, Some(c1));
}
