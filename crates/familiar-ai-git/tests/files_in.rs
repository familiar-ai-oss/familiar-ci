//! `files_in` lists the files directly inside a folder at a commit (as
//! `git ls-tree --name-only <sha> <dir>/` does, files only), and
//! `root_commit` names the repository's first commit: verify reads the signed
//! approvals in `.familiar/approvals/` at HEAD and binds them to the
//! repository by its root commit (plan §6.9).

mod common;

use common::TestRepo;
use familiar_ai_git::{files_in, root_commit};

#[test]
fn files_in_lists_a_folders_files_at_a_commit() {
    let repo = TestRepo::new();
    repo.write("README", b"x\n");
    let first = repo.commit("first");
    assert_eq!(files_in(&repo.dir, &first, ".familiar/approvals").unwrap(), Vec::<String>::new(), "no folder: nothing");

    repo.write(".familiar/approvals/2.json", b"{}\n");
    repo.write(".familiar/approvals/2.sig", b"sig\n");
    repo.write(".familiar/approvals/10.json", b"{}\n");
    repo.write(".familiar/approvals/deeper/3.json", b"{}\n");
    repo.write(".familiar/plan", b"\n");
    let second = repo.commit("second");
    let mut want = vec!["10.json".to_string(), "2.json".into(), "2.sig".into()];
    want.sort();
    assert_eq!(files_in(&repo.dir, &second, ".familiar/approvals").unwrap(), want, "files only, sorted, no subfolder");
    assert_eq!(files_in(&repo.dir, &first, ".familiar/approvals").unwrap(), Vec::<String>::new(), "read at its commit, not the work tree");
    assert!(files_in(&repo.dir, "0000000000000000000000000000000000000000", ".familiar").is_err());
}

#[test]
fn root_commit_is_the_first_commit_whatever_the_tip() {
    let repo = TestRepo::new();
    repo.write("a", b"1\n");
    let first = repo.commit("first");
    repo.write("a", b"2\n");
    let second = repo.commit("second");
    assert_eq!(root_commit(&repo.dir, &second).unwrap(), first);
    assert_eq!(root_commit(&repo.dir, &first).unwrap(), first);
    assert_eq!(repo.git(&["rev-list", "--max-parents=0", "HEAD"]).trim(), first);
}
