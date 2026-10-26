//! `tests_hash` gives the same hash for the same content whatever the commit
//! id.

mod common;

use common::TestRepo;
use familiar_ai_git::tests_hash;

#[test]
fn same_content_different_commits_equal_hash() {
    let repo = TestRepo::new();
    repo.write("tests/a.py", b"ta\n");
    repo.write("tests/b.py", b"tb\n");
    let c1 = repo.commit("tests");
    repo.write("src/code.py", b"x = 1\n");
    let c2 = repo.commit("code on top");
    let paths = vec!["tests/a.py".to_string(), "tests/b.py".to_string()];
    assert_eq!(
        tests_hash(&repo.dir, &c1, &paths).unwrap(),
        tests_hash(&repo.dir, &c2, &paths).unwrap()
    );
}

#[test]
fn one_byte_changed_changes_hash() {
    let repo = TestRepo::new();
    repo.write("t/a.py", b"ta\n");
    repo.write("t/b.py", b"tb\n");
    let c1 = repo.commit("v1");
    repo.write("t/b.py", b"tX\n");
    let c2 = repo.commit("v2");
    let paths = vec!["t/a.py".to_string(), "t/b.py".to_string()];
    assert_ne!(
        tests_hash(&repo.dir, &c1, &paths).unwrap(),
        tests_hash(&repo.dir, &c2, &paths).unwrap()
    );
}

#[test]
fn renaming_a_file_changes_hash() {
    let repo = TestRepo::new();
    repo.write("t/a.py", b"ta\n");
    repo.write("t/b.py", b"tb\n");
    let c1 = repo.commit("v1");
    repo.git(&["rm", "-q", "t/b.py"]);
    repo.write("t/renamed.py", b"tb\n");
    let c2 = repo.commit("v2");
    // Same set framing: the plan's paths are still a.py and b.py; b.py is absent now.
    let paths = vec!["t/a.py".to_string(), "t/b.py".to_string()];
    assert_ne!(
        tests_hash(&repo.dir, &c1, &paths).unwrap(),
        tests_hash(&repo.dir, &c2, &paths).unwrap()
    );
    // And hashing the renamed set differs too (path is in the frame).
    let paths2 = vec!["t/a.py".to_string(), "t/renamed.py".to_string()];
    assert_ne!(
        tests_hash(&repo.dir, &c1, &paths).unwrap(),
        tests_hash(&repo.dir, &c2, &paths2).unwrap()
    );
}

#[test]
fn deleting_a_file_changes_hash() {
    let repo = TestRepo::new();
    repo.write("t/a.py", b"ta\n");
    repo.write("t/b.py", b"tb\n");
    let c1 = repo.commit("v1");
    repo.git(&["rm", "-q", "t/b.py"]);
    let c2 = repo.commit("v2");
    let paths = vec!["t/a.py".to_string(), "t/b.py".to_string()];
    assert_ne!(
        tests_hash(&repo.dir, &c1, &paths).unwrap(),
        tests_hash(&repo.dir, &c2, &paths).unwrap()
    );
}

#[test]
fn moving_bytes_between_files_changes_hash() {
    // Unambiguous framing: a byte moved from b to a must not collide.
    let repo = TestRepo::new();
    repo.write("a", b"X");
    repo.write("b", b"Y");
    let c1 = repo.commit("v1");
    repo.write("a", b"");
    repo.write("b", b"XY");
    let c2 = repo.commit("v2");
    let paths = vec!["a".to_string(), "b".to_string()];
    assert_ne!(
        tests_hash(&repo.dir, &c1, &paths).unwrap(),
        tests_hash(&repo.dir, &c2, &paths).unwrap()
    );
}

#[test]
fn rebase_like_rewrite_same_hash() {
    // The same change committed again with another message (different commit
    // id) must give the same hash: this is why a rebase changes nothing.
    let repo = TestRepo::new();
    repo.write("t/a.py", b"ta\n");
    let c1 = repo.commit("tests, first message");

    let repo2 = TestRepo::new();
    repo2.write("t/a.py", b"ta\n");
    let c2 = repo2.commit("tests, SECOND message");
    assert_ne!(c1, c2);

    let paths = vec!["t/a.py".to_string()];
    assert_eq!(
        tests_hash(&repo.dir, &c1, &paths).unwrap(),
        tests_hash(&repo2.dir, &c2, &paths).unwrap()
    );
}

#[test]
fn all_absent_still_hashes() {
    let repo = TestRepo::new();
    repo.write("a.txt", b"a\n");
    let c1 = repo.commit("no tests yet");
    // Hashing a set of files none of which exists is defined (all absent);
    // the "absent" None case for commit_seen is decided by the caller.
    let paths = vec!["t/x.py".to_string()];
    let h1 = tests_hash(&repo.dir, &c1, &paths).unwrap();
    let h2 = tests_hash(&repo.dir, &c1, &paths).unwrap();
    assert_eq!(h1, h2);
}
