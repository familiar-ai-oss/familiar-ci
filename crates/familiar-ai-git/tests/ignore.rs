//! `is_ignored` against `git check-ignore`'s own answer, and `worktree_root`
//! from inside and outside a repository. The file asked about need not exist.

mod common;

use std::path::Path;
use std::process::Command;

use common::TestRepo;
use familiar_ai_git::{is_ignored, worktree_root};

/// `git check-ignore -q <path>`: exit 0 when ignored, 1 when not.
fn git_says_ignored(repo: &TestRepo, rel: &str) -> bool {
    let out = Command::new("git")
        .args(["check-ignore", "-q", rel])
        .current_dir(&repo.dir)
        .output()
        .unwrap();
    match out.status.code() {
        Some(0) => true,
        Some(1) => false,
        other => panic!("git check-ignore exited {other:?}"),
    }
}

fn same_as_git(repo: &TestRepo, rel: &str) -> bool {
    let ours = is_ignored(&repo.dir, rel).unwrap_or_else(|e| panic!("{rel}: {e}"));
    assert_eq!(ours, git_says_ignored(repo, rel), "{rel}");
    ours
}

#[test]
fn a_fresh_repository_ignores_nothing_and_the_file_need_not_exist() {
    let repo = TestRepo::new();
    assert!(!same_as_git(&repo, ".familiar/record"));
    assert!(!same_as_git(&repo, ".familiar/plan"));
}

#[test]
fn the_root_gitignore_the_folders_own_and_info_exclude_all_count() {
    let repo = TestRepo::new();
    repo.write(".gitignore", b"target/\n.familiar/record\n");
    assert!(same_as_git(&repo, ".familiar/record"));
    assert!(!same_as_git(&repo, ".familiar/plan"));
    assert!(same_as_git(&repo, "target/x.o"));

    let repo = TestRepo::new();
    repo.write("sub/.gitignore", b"record\n");
    assert!(same_as_git(&repo, "sub/.familiar/record"));
    assert!(!same_as_git(&repo, ".familiar/record"));

    let repo = TestRepo::new();
    repo.write(".git/info/exclude", b".familiar/record\n");
    assert!(same_as_git(&repo, ".familiar/record"));

    // the whole folder ignored takes the plan with it
    let repo = TestRepo::new();
    repo.write(".gitignore", b".familiar/\n");
    assert!(same_as_git(&repo, ".familiar/record"));
    assert!(same_as_git(&repo, ".familiar/plan"));

    // a negation after the rule un-ignores
    let repo = TestRepo::new();
    repo.write(".gitignore", b".familiar/*\n!.familiar/plan\n");
    assert!(same_as_git(&repo, ".familiar/record"));
    assert!(!same_as_git(&repo, ".familiar/plan"));
}

#[test]
fn worktree_root_is_the_repositorys_working_directory_from_any_folder_inside() {
    let repo = TestRepo::new();
    repo.write("a/b/c.txt", b"x\n");
    let root = std::fs::canonicalize(&repo.dir).unwrap();
    for start in [repo.dir.clone(), repo.dir.join("a"), repo.dir.join("a/b")] {
        let found = worktree_root(&start).unwrap().expect("a working directory");
        assert_eq!(std::fs::canonicalize(found).unwrap(), root, "{}", start.display());
    }
}

#[test]
fn outside_a_repository_is_an_error_and_a_bare_one_has_no_working_directory() {
    let outside = std::env::temp_dir().join(format!("familiar-ai-git-no-repo-{}", std::process::id()));
    std::fs::create_dir_all(&outside).unwrap();
    assert!(
        worktree_root(&outside).is_err(),
        "a folder in no repository: {}",
        outside.display()
    );
    assert!(is_ignored(&outside, "x").is_err());

    let bare = outside.join("bare.git");
    let out = Command::new("git").args(["init", "-q", "--bare"]).arg(&bare).output().unwrap();
    assert!(out.status.success());
    assert_eq!(worktree_root(Path::new(&bare)).unwrap(), None);
    let _ = std::fs::remove_dir_all(&outside);
}
