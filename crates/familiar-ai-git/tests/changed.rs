//! `changed` matches `git diff --name-only` and hand-known hunks, including on
//! a first commit and for an added and a deleted file.

mod common;

use common::TestRepo;
use familiar_ai_git::{changed, Hunk};

#[test]
fn first_commit_against_empty_tree() {
    let repo = TestRepo::new();
    repo.write("a.txt", b"l1\nl2\nl3\n");
    let c1 = repo.commit("first");
    let files = changed(&repo.dir, &c1).unwrap();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].path, "a.txt");
    // Everything added: old empty, new lines 1..=3.
    assert_eq!(
        files[0].hunks,
        vec![Hunk {
            old_start: 1,
            old_count: 0,
            new_start: 1,
            new_count: 3
        }]
    );
    assert_eq!(
        repo.diff_name_only("4b825dc642cb6eb9a060e54bf8d69288fbee4904", &c1),
        vec!["a.txt"]
    );
}

#[test]
fn added_and_deleted_file() {
    let repo = TestRepo::new();
    repo.write("keep.txt", b"k\n");
    repo.write("gone.txt", b"g1\ng2\n");
    repo.commit("setup");
    repo.git(&["rm", "-q", "gone.txt"]);
    repo.write("new.txt", b"n1\n");
    let c2 = repo.commit("swap");
    let files = changed(&repo.dir, &c2).unwrap();
    let paths: Vec<&str> = files.iter().map(|f| f.path.as_str()).collect();
    assert_eq!(paths, repo.diff_name_only(&format!("{c2}~1"), &c2));
    assert_eq!(paths, vec!["gone.txt", "new.txt"]);

    let gone = files.iter().find(|f| f.path == "gone.txt").unwrap();
    assert_eq!(
        gone.hunks,
        vec![Hunk {
            old_start: 1,
            old_count: 2,
            new_start: 1,
            new_count: 0
        }]
    );
    let new = files.iter().find(|f| f.path == "new.txt").unwrap();
    assert_eq!(
        new.hunks,
        vec![Hunk {
            old_start: 1,
            old_count: 0,
            new_start: 1,
            new_count: 1
        }]
    );
}

#[test]
fn modified_file_hand_known_hunks() {
    let repo = TestRepo::new();
    repo.write("f.txt", b"a\nb\nc\nd\ne\nf\ng\n");
    repo.commit("v1");
    // Change line 2 (b -> B) and line 5 (e -> E).
    repo.write("f.txt", b"a\nB\nc\nd\nE\nf\ng\n");
    let c2 = repo.commit("v2");
    let files = changed(&repo.dir, &c2).unwrap();
    assert_eq!(files.len(), 1);
    // Two hunks: line 2 replaced; line 5 replaced.
    assert_eq!(
        files[0].hunks,
        vec![
            Hunk {
                old_start: 2,
                old_count: 1,
                new_start: 2,
                new_count: 1
            },
            Hunk {
                old_start: 5,
                old_count: 1,
                new_start: 5,
                new_count: 1
            },
        ]
    );
}

#[test]
fn insertion_at_end_of_file() {
    let repo = TestRepo::new();
    repo.write("f.txt", b"a\nb\n");
    repo.commit("v1");
    repo.write("f.txt", b"a\nb\nc\n");
    let c2 = repo.commit("v2");
    let files = changed(&repo.dir, &c2).unwrap();
    // Pure insertion after old line 2 (new line 3 appears before "old line 3",
    // which does not exist: between old lines 2 and 3).
    assert_eq!(
        files[0].hunks,
        vec![Hunk {
            old_start: 3,
            old_count: 0,
            new_start: 3,
            new_count: 1
        }]
    );
}

#[test]
fn no_change_no_files() {
    let repo = TestRepo::new();
    repo.write("f.txt", b"a\n");
    repo.commit("v1");
    let c2 = repo.commit("empty");
    assert!(changed(&repo.dir, &c2).unwrap().is_empty());
}
