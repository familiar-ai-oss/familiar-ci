//! Helpers for integration tests that run the built binary with real argv,
//! stdin, exit codes and streams.

#![allow(dead_code)]

use std::io::Write;
use std::process::{Command, Output, Stdio};

pub mod project;
#[allow(unused_imports)]
pub use project::{cases_kept_at, fixture, temp_dir, Project};

/// A scratch folder for this test run's setup markers (`FAMILIAR_AI_HOME`),
/// never the real home.
pub fn test_home() -> std::path::PathBuf {
    let dir = temp_dir("home");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// Runs the built binary with argv and stdin and returns its full output.
pub fn run_binary(args: &[&str], stdin_text: &str) -> Output {
    run_binary_in(None, args, stdin_text)
}

/// `run_binary` in the given working folder (None: the test's own).
pub fn run_binary_in(dir: Option<&std::path::Path>, args: &[&str], stdin_text: &str) -> Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_familiar-ai"));
    // The binary under test never sees the project of the session running the tests.
    cmd.env_remove("CLAUDE_PROJECT_DIR");
    if let Some(dir) = dir {
        cmd.current_dir(dir);
    }
    let mut child = cmd
        .env("FAMILIAR_AI_JUDGE", "off")
        .env("FAMILIAR_AI_INVESTIGATOR", "off")
        .args(args)
        .env("FAMILIAR_AI_HOME", test_home())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn familiar-ai");
    let mut stdin = child.stdin.take().expect("stdin is piped");
    // a command that never reads stdin may have exited already; a broken pipe is fine
    if let Err(e) = stdin.write_all(stdin_text.as_bytes()) {
        assert_eq!(e.kind(), std::io::ErrorKind::BrokenPipe, "write stdin: {e}");
    }
    drop(stdin); // close the pipe so the binary sees EOF
    child.wait_with_output().expect("wait for familiar-ai")
}

pub fn exit_code(output: &Output) -> i32 {
    output.status.code().expect("exit code (not killed)")
}

pub fn stdout(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).expect("stdout is utf-8")
}

pub fn stderr(output: &Output) -> String {
    String::from_utf8(output.stderr.clone()).expect("stderr is utf-8")
}

/// A fresh folder under the temp dir, in no repository, removed on drop.
pub struct Scratch {
    pub dir: std::path::PathBuf,
}

impl Scratch {
    pub fn new(name: &str) -> Scratch {
        let dir = temp_dir(name);
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("temp folder");
        Scratch { dir }
    }

    pub fn git_init(&self) {
        let out = Command::new("git").args(["init", "-q", "-b", "main"]).current_dir(&self.dir).output().expect("git");
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    }

    /// Whether git ignores `path` here.
    pub fn git_ignores(&self, path: &str) -> bool {
        let out = Command::new("git").args(["check-ignore", "-q", path]).current_dir(&self.dir).output().expect("git");
        out.status.code() == Some(0)
    }

    /// Runs the binary in this folder with `CLAUDE_PROJECT_DIR` unset;
    /// returns (exit code, stdout, stderr).
    pub fn run(&self, args: &[&str]) -> (i32, String, String) {
        let output = Command::new(env!("CARGO_BIN_EXE_familiar-ai"))
            .env("FAMILIAR_AI_JUDGE", "off")
            .env("FAMILIAR_AI_INVESTIGATOR", "off")
            .args(args)
            .current_dir(&self.dir)
            .env_remove("CLAUDE_PROJECT_DIR")
            .env("FAMILIAR_AI_HOME", test_home())
            .stdin(Stdio::null())
            .output()
            .expect("run familiar-ai");
        (exit_code(&output), stdout(&output), stderr(&output))
    }

    /// Runs `git <args>` here, which must succeed, and returns its trimmed stdout.
    pub fn git(&self, args: &[&str]) -> String {
        let out = Command::new("git").args(args).current_dir(&self.dir).output().expect("git");
        assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    }

    pub fn read(&self, path: &str) -> String {
        std::fs::read_to_string(self.dir.join(path)).unwrap_or_default()
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// A repository with one anchor in a subfolder, planned up to the test command.
pub fn planned_to_line_7(name: &str) -> Scratch {
    let p = Scratch::new(name);
    p.git_init();
    std::fs::create_dir_all(p.dir.join("pydantic")).expect("folder");
    std::fs::write(p.dir.join("pydantic/json_schema.py"), "def schema():\n    return {}\n").expect("write");
    assert_eq!(p.run(&["setup"]).0, 0);
    for args in [
        vec!["add", "-A"],
        vec!["-c", "user.name=t", "-c", "user.email=t@example.com", "-c", "commit.gpgsign=false", "commit", "-q", "-m", "base"],
    ] {
        let out = Command::new("git").args(&args).current_dir(&p.dir).output().expect("git");
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    }
    for args in [
        vec!["claim", "intent_submitted", "--for", "0", "--text", "schema of a model"],
        vec!["claim", "behavior", "--behavior_id", "b1", "--text", "the greeting"],
        vec!["claim", "promise", "--promise_id", "p1", "--behavior_id", "b1", "--accepts", "a name", "--gives_back", "a greeting", "--refuses", "nothing", "--cases", r#"[{"case_id":"n1","kind":"normal","given":"a name","expect":"a greeting"},{"case_id":"o1","kind":"opposite","given":"no name","expect":"no greeting"}]"#],
        vec!["claim", "criterion", "--criterion_id", "c1", "--text", "schema names the model", "--path", "full", "--behavior_id", "b1", "--promise_id", "p1"],
        vec![
            "claim",
            "place_chosen",
            "--criterion_id",
            "c1",
            "--places",
            r#"[{"file_path":"pydantic/json_schema.py","symbol":"schema","line_start":1,"line_end":2}]"#,
        ],
        // definition change (lane l1, 2026-10-15): a test file is named after what it tests, never a plan id
        vec!["claim", "test_planned", "--criterion_id", "c1", "--file_path", "tests/test_schema.py", "--will", "create"],
    ] {
        let (code, out, err) = p.run(&args);
        assert_eq!(code, 0, "{args:?}: {out}{err}");
    }
    // the cases card passed (plan cases-card, 2026-10-25) [draft names]
    cases_kept_at(&p.dir, "c1");
    p
}

/// Places before the fix [draft] (the owner's plan of 2026-10-21, step 3): every site the binary
/// listed for a part, classified not affected with a reason, as an agent whose work does not reach
/// them classifies them before its tests-first commit. A site of an older listing is refused and
/// left.
pub fn classify_places(p: &Project) {
    for f in p.facts_of("places_listed") {
        let c = f["criterion_id"].as_str().unwrap_or("").to_string();
        for s in f["sites"].as_array().cloned().unwrap_or_default() {
            let (file, line) = (s["file"].as_str().unwrap_or("").to_string(), s["line"].to_string());
            p.try_claim(&["place_classified", "--criterion_id", &c, "--file", &file, "--line", &line, "--verdict", "not_affected", "--reason", "another use of the name; the behavior this part changes does not reach it"]);
        }
    }
}
