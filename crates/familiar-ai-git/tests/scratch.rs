//! `scratch_with_lines_put_back` writes a copy equal to the tip except the
//! mapped region, which holds the earlier lines, and leaves the repository's
//! working folder byte-for-byte unchanged.

mod common;

use common::TestRepo;
use familiar_ai_git::scratch_with_lines_put_back;

#[test]
fn scratch_equals_tip_except_put_back_region() {
    let repo = TestRepo::new();
    // earlier: the function without the change.
    repo.write("src/login.py", b"def check_password():\n    a\n    b\n");
    repo.write("tests/t.py", b"def test_t():\n    assert False\n");
    let earlier = repo.commit("earlier");
    // tip: two lines added above, and the anchor lines changed.
    repo.write(
        "src/login.py",
        b"# header\n# more\ndef check_password():\n    A!\n    B!\n",
    );
    let tip = repo.commit("tip");

    let scratch = std::env::temp_dir().join(format!("familiar-scratch-{}", std::process::id()));
    std::fs::create_dir_all(&scratch).unwrap();
    let before = repo.workdir_bytes();

    // Put lines 2..=3 (as numbered at `earlier`) back.
    scratch_with_lines_put_back(&repo.dir, &scratch, &tip, &earlier, "src/login.py", 2, 3).unwrap();

    // The scratch copy equals the tip everywhere except the mapped region.
    let tip_files: std::collections::BTreeMap<_, _> = repo
        .workdir_bytes()
        .into_iter()
        .filter(|(p, _)| p != "scratch")
        .collect();
    let scratch_files: std::collections::BTreeMap<_, _> =
        common_collect(&scratch).into_iter().collect();
    assert_eq!(scratch_files.len(), tip_files.len());
    for (path, tip_bytes) in &tip_files {
        let got = &scratch_files[path];
        if path == "src/login.py" {
            // Header (added above the range) survives; the mapped region is
            // the earlier lines again.
            assert_eq!(
                got,
                b"# header\n# more\ndef check_password():\n    a\n    b\n"
            );
        } else {
            assert_eq!(got, tip_bytes);
        }
    }

    // The repository's working folder is byte-for-byte unchanged afterwards.
    let mut after = repo.workdir_bytes();
    after.retain(|(p, _)| p != "scratch");
    let mut before_clean = before;
    before_clean.retain(|(p, _)| p != "scratch");
    assert_eq!(before_clean, after);

    let _ = std::fs::remove_dir_all(&scratch);
}

#[test]
fn scratch_errors_when_file_missing_at_either_commit() {
    let repo = TestRepo::new();
    repo.write("a.txt", b"a\n");
    let c1 = repo.commit("c1");
    repo.write("b.txt", b"b\n");
    let c2 = repo.commit("c2");
    let scratch = repo.dir.join("scratch2");
    std::fs::create_dir_all(&scratch).unwrap();
    // b.txt missing at c1.
    assert!(scratch_with_lines_put_back(&repo.dir, &scratch, &c2, &c1, "b.txt", 1, 1).is_err());
    let _ = std::fs::remove_dir_all(&scratch);
}

// The teeth run must be able to put back a range deleted at the tip, so the
// deleted lines come back and nothing else changes.
#[test]
fn scratch_puts_back_a_range_that_was_deleted() {
    let repo = TestRepo::new();
    repo.write("f.txt", b"1\n2\n3\n");
    repo.write("other.txt", b"same\n");
    let c1 = repo.commit("c1");
    repo.write("f.txt", b"1\n");
    let c2 = repo.commit("deleted the range");
    let scratch = repo.dir.join("scratch3");
    std::fs::create_dir_all(&scratch).unwrap();
    scratch_with_lines_put_back(&repo.dir, &scratch, &c2, &c1, "f.txt", 2, 3).unwrap();
    assert_eq!(std::fs::read(scratch.join("f.txt")).unwrap(), b"1\n2\n3\n");
    assert_eq!(std::fs::read(scratch.join("other.txt")).unwrap(), b"same\n");
    // the repository's own working copy still has the tip's file
    assert_eq!(std::fs::read(repo.dir.join("f.txt")).unwrap(), b"1\n");
    let _ = std::fs::remove_dir_all(&scratch);
}

#[test]
fn scratch_at_writes_a_commits_files_and_keeps_the_executable_ones_executable() {
    use std::os::unix::fs::PermissionsExt;
    let repo = TestRepo::new();
    repo.write("run.sh", b"#!/bin/sh\nexit 0\n");
    repo.write("a/b.txt", b"one\n");
    std::fs::set_permissions(repo.dir.join("run.sh"), std::fs::Permissions::from_mode(0o755)).unwrap();
    let c1 = repo.commit("c1");
    repo.write("a/b.txt", b"two\n");
    repo.commit("c2");
    let scratch = std::env::temp_dir().join(format!("familiar-scratch-at-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&scratch);
    familiar_ai_git::scratch_at(&repo.dir, &scratch, &c1).unwrap();
    assert_eq!(std::fs::read(scratch.join("a/b.txt")).unwrap(), b"one\n");
    let mode = std::fs::metadata(scratch.join("run.sh")).unwrap().permissions().mode();
    assert_eq!(mode & 0o111, 0o111, "executable kept: {mode:o}");
    let mode = std::fs::metadata(scratch.join("a/b.txt")).unwrap().permissions().mode();
    assert_eq!(mode & 0o111, 0, "a plain file stays plain: {mode:o}");
    let mut names = familiar_ai_git::top_level_names(&repo.dir, &c1).unwrap();
    names.sort();
    assert_eq!(names, vec!["a".to_string(), "run.sh".to_string()]);
    let _ = std::fs::remove_dir_all(&scratch);
}

fn common_collect(dir: &std::path::Path) -> Vec<(String, Vec<u8>)> {
    let mut out = Vec::new();
    fn go(dir: &std::path::Path, prefix: &str, out: &mut Vec<(String, Vec<u8>)>) {
        for e in std::fs::read_dir(dir).unwrap() {
            let e = e.unwrap();
            let name = e.file_name().into_string().unwrap();
            let rel = if prefix.is_empty() {
                name.clone()
            } else {
                format!("{prefix}/{name}")
            };
            if rel == ".git" {
                // the copy's own git dir (a checkout of the commit), not a tracked file
                continue;
            }
            if e.file_type().unwrap().is_dir() {
                go(&e.path(), &rel, out);
            } else {
                out.push((rel, std::fs::read(e.path()).unwrap()));
            }
        }
    }
    go(dir, "", &mut out);
    out.sort();
    out
}
