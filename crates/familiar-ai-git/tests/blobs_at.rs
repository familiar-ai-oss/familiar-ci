//! `blobs_at` reads every file of a commit's tree, each path from the repository's root with its
//! bytes, as `git ls-tree -r` lists them: the reviewer's sibling search reads the green commit
//! this way, never the work tree.

mod common;

use common::TestRepo;
use familiar_ai_git::blobs_at;

#[test]
fn blobs_at_reads_every_file_of_a_commit_from_the_root() {
    let repo = TestRepo::new();
    repo.write("README", b"one\n");
    repo.write("src/pkg/a.py", b"x = 1\n");
    let first = repo.commit("first");
    repo.write("src/pkg/a.py", b"x = 2\n");
    repo.write("src/b.py", b"y = 1\n");
    let second = repo.commit("second");
    std::fs::write(repo.dir.join("src/b.py"), b"changed in the work tree\n").unwrap();

    let at = blobs_at(&repo.dir, &second).unwrap();
    let paths: Vec<&str> = at.keys().map(String::as_str).collect();
    assert_eq!(paths, ["README", "src/b.py", "src/pkg/a.py"], "every file, nested ones too, sorted");
    let listed = repo.git(&["ls-tree", "-r", "--name-only", &second]);
    assert_eq!(paths.join("\n"), listed, "as git lists them");
    assert_eq!(at["src/pkg/a.py"], b"x = 2\n");
    assert_eq!(at["src/b.py"], b"y = 1\n", "read at the commit, not the work tree");
    assert_eq!(blobs_at(&repo.dir, &first).unwrap()["src/pkg/a.py"], b"x = 1\n");
    assert!(blobs_at(&repo.dir, "0000000000000000000000000000000000000000").is_err());
}
