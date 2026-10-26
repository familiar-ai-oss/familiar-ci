//! `file_at` and `lines_hash` match `git show <sha>:<path>`.

mod common;

use common::{sha256_hex, TestRepo};
use familiar_ai_git::{file_at, lines_hash, GitError};

#[test]
fn file_at_matches_git_show() {
    let repo = TestRepo::new();
    repo.write("src/login.py", b"def check_password():\n    pass\n");
    repo.write("tests/test_c1.py", b"def test_t1():\n    assert True\n");
    let sha = repo.commit("c1");

    assert_eq!(
        file_at(&repo.dir, &sha, "src/login.py").unwrap(),
        repo.show(&sha, "src/login.py")
    );
    assert_eq!(
        file_at(&repo.dir, &sha, "tests/test_c1.py").unwrap(),
        repo.show(&sha, "tests/test_c1.py")
    );

    // Clear errors: missing path, missing commit.
    match file_at(&repo.dir, &sha, "nope.py") {
        Err(GitError::FileMissing { path, .. }) => assert_eq!(path, "nope.py"),
        other => panic!("expected FileMissing, got {other:?}"),
    }
    assert!(file_at(&repo.dir, "0000000000000000000000000000000000000000", "x").is_err());
}

#[test]
fn file_at_tracks_history() {
    let repo = TestRepo::new();
    repo.write("a.txt", b"one\n");
    let c1 = repo.commit("one");
    repo.write("a.txt", b"one\ntwo\n");
    let c2 = repo.commit("two");
    assert_eq!(file_at(&repo.dir, &c1, "a.txt").unwrap(), b"one\n");
    assert_eq!(file_at(&repo.dir, &c2, "a.txt").unwrap(), b"one\ntwo\n");
}

#[test]
fn lines_hash_is_exact_bytes_of_range() {
    let repo = TestRepo::new();
    let content: &[u8] = b"alpha\nbeta\ngamma\ndelta\n";
    repo.write("f.txt", content);
    let sha = repo.commit("f");

    // lines 2..=3 are "beta\ngamma\n", including line endings.
    assert_eq!(
        lines_hash(&repo.dir, &sha, "f.txt", 2, 3).unwrap(),
        sha256_hex(b"beta\ngamma\n")
    );
    assert_eq!(
        lines_hash(&repo.dir, &sha, "f.txt", 1, 4).unwrap(),
        sha256_hex(content)
    );

    // A range outside the file is an error.
    match lines_hash(&repo.dir, &sha, "f.txt", 3, 5) {
        Err(GitError::RangeOutside {
            line_start: 3,
            line_end: 5,
            ..
        }) => {}
        other => panic!("expected RangeOutside, got {other:?}"),
    }
    assert!(lines_hash(&repo.dir, &sha, "f.txt", 0, 1).is_err());
}
