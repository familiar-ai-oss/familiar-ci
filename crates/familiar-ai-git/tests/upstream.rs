//! `upstream` reports the head's branch and how far it is ahead of the branch
//! it tracks, with a local bare repository as the remote.

mod common;

use common::TestRepo;
use familiar_ai_git::{upstream, Upstream};

#[test]
fn no_commit_is_none_no_upstream_is_unknown_then_ahead_counts_then_even() {
    let repo = TestRepo::new();
    assert_eq!(upstream(&repo.dir).unwrap(), None, "no commit yet");
    repo.write("a.txt", b"a\n");
    repo.commit("one");
    let main = |ahead| Some(Upstream { branch: "main".into(), ahead });
    assert_eq!(upstream(&repo.dir).unwrap(), main(None), "no upstream");
    let bare = repo.dir.with_extension("remote.git");
    repo.git(&["init", "-q", "--bare", bare.to_str().unwrap()]);
    repo.git(&["remote", "add", "origin", bare.to_str().unwrap()]);
    repo.git(&["push", "-q", "-u", "origin", "main"]);
    assert_eq!(upstream(&repo.dir).unwrap(), main(Some(0)), "pushed");
    repo.write("b.txt", b"b\n");
    repo.commit("two");
    repo.write("c.txt", b"c\n");
    repo.commit("three");
    assert_eq!(upstream(&repo.dir).unwrap(), main(Some(2)), "two ahead");
    repo.git(&["push", "-q"]);
    assert_eq!(upstream(&repo.dir).unwrap(), main(Some(0)), "pushed again");
    let head = repo.git(&["rev-parse", "HEAD"]);
    repo.git(&["checkout", "-q", &head]);
    assert_eq!(upstream(&repo.dir).unwrap(), None, "detached");
    let _ = std::fs::remove_dir_all(&bare);
}
