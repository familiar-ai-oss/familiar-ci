//! Two heads, each with its own test file and command, walked through the
//! real binary.
#![allow(dead_code)]

use std::ops::Deref;
use std::path::{Path, PathBuf};

use crate::common::Project;
use familiar_ai_facts::Fact;
use serde_json::Value;

#[allow(unused_imports)]
pub use crate::common::project::facts_at;

pub const SESSION: &str = "3c1d2e4f-5a6b-4c7d-8e9f-a0b1c2d3e4f5";
pub const T1: &str = "tests/test_c1.sh";
pub const T2: &str = "tests/test_c2.sh";
pub const TEST1: &str = "sh src/a.sh | grep -qx hello\n";
pub const TEST2: &str = "sh src/b.sh | grep -qx bye\n";

/// The shared project, with two heads planned on top.
pub struct Heads(Project);

impl Deref for Heads {
    type Target = Project;

    fn deref(&self) -> &Project {
        &self.0
    }
}

impl Heads {
    /// Builds main at `base` (two scripts, each greeting wrong) and branch `work`
    /// with setup's commit.
    pub fn new(name: &str) -> Heads {
        let p = Project::builder(name)
            .session(SESSION)
            .no_app()
            .file("src/a.sh", "#!/bin/sh\ngreet() { echo hi; }\ngreet\n")
            .file("src/b.sh", "#!/bin/sh\npart() { echo hi; }\npart\n")
            .setup_on_branch("work")
            .create();
        Heads(p)
    }

    /// Claims c1 on `src/a.sh` and c2 on `src/b.sh`, each running its own test file.
    pub fn two_heads(name: &str) -> Heads {
        let p = Heads::new(name);
        p.claim(&["intent_submitted", "--for", "0", "--text", "a says hello, b says bye"]);
        for (c, file, sym, text) in [("c1", "src/a.sh", "greet", "a says hello"), ("c2", "src/b.sh", "part", "b says bye")] {
            p.claim(&["behavior", "--behavior_id", "b1", "--text", "the greeting"]);
            p.claim(&["promise", "--promise_id", "p1", "--behavior_id", "b1", "--accepts", "a name", "--gives_back", "a greeting", "--refuses", "nothing", "--cases", r#"[{"case_id":"n1","kind":"normal","given":"a name","expect":"a greeting"},{"case_id":"o1","kind":"opposite","given":"no name","expect":"no greeting"}]"#]);
            p.claim(&["criterion", "--criterion_id", c, "--text", text, "--path", "full", "--behavior_id", "b1", "--promise_id", "p1"]);
            p.cases_kept(c); // the cases card passed (plan cases-card, 2026-10-25) [draft names]
            let places = format!(r#"[{{"file_path":"{file}","symbol":"{sym}","line_start":1,"line_end":3}}]"#);
            p.claim(&["place_chosen", "--criterion_id", c, "--places", &places]);
            let t = format!("tests/test_{c}.sh");
            p.claim(&["test_planned", "--criterion_id", c, "--file_path", &t, "--will", "create", "--test_command", &format!("sh {t}")]);
        }
        p.claim(&["test_stack_detected", "--project_root", ".", "--test_stack", "sh", "--test_command", "false"]);
        for c in ["c1", "c2"] {
            p.claim(&["criterion_head", "--criterion_id", c]);
            p.claim(&["criterion_assigned", "--criterion_id", c]);
            p.claim(&["fix_location_category", "--criterion_id", c, "--category", "this_repository", "--where", ""]);
        }
        std::fs::create_dir_all(p.path("tests")).expect("tests");
        p
    }

    /// Commits both tests (red), then both fixes (green), on the laptop.
    pub fn both_green(name: &str) -> Heads {
        let p = Heads::two_heads(name);
        p.write(T1, TEST1);
        p.write(T2, TEST2);
        p.commit("both criteria's tests");
        p.write("src/a.sh", "#!/bin/sh\ngreet() { echo hello; }\ngreet\n");
        p.write("src/b.sh", "#!/bin/sh\npart() { echo bye; }\npart\n");
        p.commit("both fixes");
        p
    }

    /// Makes the agent's commit, calls the hook after it, and returns its id.
    pub fn commit(&self, message: &str) -> String {
        self.0.commit(message);
        self.head()
    }

    /// Commits the plan as the binary last wrote it, alone.
    pub fn commit_plan(&self) {
        self.git(&["add", ".familiar/plan"]);
        if !self.git(&["status", "--porcelain", "--", ".familiar/plan"]).is_empty() {
            self.git(&["commit", "-q", "-m", "the plan"]);
        }
    }

    /// Runs `verify` in a fresh clone of the branch; returns the exit code,
    /// the output and the clone's folder.
    pub fn verify_in_clone(&self, name: &str) -> (i32, String, PathBuf) {
        let clone = self.dir.join(format!("clone-{name}"));
        let _ = std::fs::remove_dir_all(&clone);
        self.git_in(&self.dir, &["clone", "-q", "--branch", "work", self.repo().to_str().expect("path"), clone.to_str().expect("path")]);
        let (code, out, err) = self.run_in(&clone, &["verify"], "");
        (code, format!("{out}{err}"), clone)
    }
}

pub fn text<'a>(f: &'a Fact, k: &str) -> &'a str {
    f.fields.get(k).and_then(Value::as_str).unwrap_or("")
}

/// Returns the `tests_observed` facts of a commit as (criterion, hash), in record order.
pub fn observations(log: &[Fact], commit: &str) -> Vec<(String, String)> {
    log.iter()
        .filter(|f| f.kind == "tests_observed" && text(f, "commit") == commit)
        .map(|f| (text(f, "criterion_id").to_string(), text(f, "tests_hash").to_string()))
        .collect()
}

/// Returns the proof of every stamp of `c` at a place, in record order.
pub fn stamps(log: &[Fact], c: &str, place: &str) -> Vec<(String, String)> {
    log.iter()
        .filter(|f| f.kind == "fingerprint_stamped" && text(f, "criterion_id") == c && text(f, "ran_in") == place)
        .map(|f| (text(f, "commit").to_string(), text(f, "proof_not_tampered").to_string()))
        .collect()
}

/// Returns the fingerprint of one file at a commit, as git holds it.
pub fn hash_of(repo: &Path, commit: &str, file: &str) -> String {
    familiar_ai_git::tests_hash(repo, commit, &[file.to_string()]).expect("hash")
}
