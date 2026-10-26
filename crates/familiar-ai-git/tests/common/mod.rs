//! Build small repositories in a temporary folder with the `git` program and
//! ask `git` for an independent answer to compare with. Tests may run `git`;
//! the library does not.

// Each integration test is its own crate and uses a different subset.
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::process::Command;

pub struct TestRepo {
    pub dir: PathBuf,
}

impl TestRepo {
    pub fn new() -> Self {
        // one folder per test: the process id, a counter no two tests in
        // this process share, and the clock; a folder already there (another
        // run's) is never reused
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        std::fs::create_dir_all(std::env::temp_dir()).unwrap();
        let dir = (0..1000)
            .find_map(|_| {
                let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                let dir = std::env::temp_dir().join(format!(
                    "familiar-ai-git-test-{}-{}-{}",
                    std::process::id(),
                    n,
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_nanos())
                        .unwrap_or(0)
                ));
                std::fs::create_dir(&dir).is_ok().then_some(dir)
            })
            .expect("a fresh temporary folder for the test repository");
        let r = TestRepo { dir };
        r.git(&["init", "-q", "-b", "main"]);
        r.git(&["config", "user.email", "t@example.com"]);
        r.git(&["config", "user.name", "T"]);
        r
    }

    pub fn git(&self, args: &[&str]) -> String {
        let out = Command::new("git")
            .args(args)
            .current_dir(&self.dir)
            .env("GIT_AUTHOR_DATE", "2000-01-01T00:00:00Z")
            .env("GIT_COMMITTER_DATE", "2000-01-01T00:00:00Z")
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "git {:?} failed: {}",
            args,
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    }

    /// `git show <sha>:<path>` — the independent answer for file_at.
    pub fn show(&self, sha: &str, path: &str) -> Vec<u8> {
        let out = Command::new("git")
            .args(["show", &format!("{sha}:{path}")])
            .current_dir(&self.dir)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "git show failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        out.stdout
    }

    pub fn rev_list(&self, from: &str, to: &str) -> Vec<String> {
        self.git(&["rev-list", "--reverse", &format!("{from}..{to}")])
            .lines()
            .map(|l| l.to_string())
            .collect()
    }

    pub fn diff_name_only(&self, from: &str, to: &str) -> Vec<String> {
        let out = self.git(&["diff", "--name-only", from, to]);
        out.lines().map(|l| l.to_string()).collect()
    }

    pub fn write(&self, path: &str, bytes: &[u8]) {
        let p = self.dir.join(path);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, bytes).unwrap();
    }

    pub fn commit(&self, message: &str) -> String {
        self.git(&["add", "-A"]);
        self.git(&["commit", "-q", "-m", message, "--allow-empty"]);
        self.git(&["rev-parse", "HEAD"])
    }

    pub fn head(&self) -> String {
        self.git(&["rev-parse", "HEAD"])
    }

    pub fn workdir_bytes(&self) -> Vec<(String, Vec<u8>)> {
        let mut out = Vec::new();
        collect(&self.dir, "", &mut out);
        out.sort();
        out
    }
}

fn collect(dir: &Path, prefix: &str, out: &mut Vec<(String, Vec<u8>)>) {
    for e in std::fs::read_dir(dir).unwrap() {
        let e = e.unwrap();
        let name = e.file_name().into_string().unwrap();
        if name == ".git" {
            continue;
        }
        let rel = if prefix.is_empty() {
            name.clone()
        } else {
            format!("{prefix}/{name}")
        };
        if e.file_type().unwrap().is_dir() {
            collect(&e.path(), &rel, out);
        } else {
            out.push((rel, std::fs::read(e.path()).unwrap()));
        }
    }
}

impl Drop for TestRepo {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// SHA-256 lower-case hex of `bytes`, independent of the library.
pub fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let d = Sha256::digest(bytes);
    d.iter().map(|b| format!("{b:02x}")).collect()
}
