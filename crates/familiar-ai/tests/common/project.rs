//! One fixture for the tests that drive the built binary in a scratch git
//! repository: a builder for the repository's first state, then `git`, `run`,
//! `claim`, `hook`, `commit`, `status` and the record, all against it.
//!
//! The scratch folder is `familiar-ai-test-<file>-<name>-<pid>` under the temp
//! dir and is removed on drop. The repository is its `repo` folder and the
//! binary's home (`FAMILIAR_AI_HOME`) its `home` folder.

use std::cell::RefCell;
use std::ffi::OsString;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use familiar_ai::{FAMILIAR_FOLDER, RECORD_FILE};
use familiar_ai_facts::{Fact, Record};
use serde_json::{json, Value};

/// The script every default project starts with, anchored on `greet`.
pub const APP: &str = "#!/bin/sh\ngreet() { echo hi; }\ngreet\n";

/// The session the binary and the hook payloads run as unless a test names one.
pub const DEFAULT_SESSION: &str = "0c0ffee0-0000-4000-8000-000000000000";

/// A folder under the temp dir named for this test file, `name` and the process.
pub fn temp_dir(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("familiar-ai-test-{}-{name}-{}", env!("CARGO_CRATE_NAME"), std::process::id()))
}

/// Reads a captured payload from `tests/fixtures/<name>.json`.
pub fn fixture(name: &str) -> Value {
    let path = format!("{}/tests/fixtures/{name}.json", env!("CARGO_MANIFEST_DIR"));
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
    serde_json::from_str(&text).expect("fixture is JSON")
}

/// The first state of a project: its files, whether `setup` runs, the base
/// commit and the environment the binary sees.
pub struct Builder {
    dir: PathBuf,
    files: Vec<(String, String)>,
    setup: bool,
    base: bool,
    branch: Option<String>,
    session: Option<String>,
    env: Vec<(String, Option<OsString>)>,
}

impl Builder {
    /// The scratch folder the project will live in, for env values under it.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Adds a file to the base commit, its folders made.
    pub fn file(mut self, path: &str, text: &str) -> Builder {
        self.files.push((path.to_string(), text.to_string()));
        self
    }

    /// Starts from no files instead of `src/app.sh`.
    pub fn no_app(mut self) -> Builder {
        self.files.retain(|(p, _)| p != "src/app.sh");
        self
    }

    /// Skips `familiar-ai setup`.
    pub fn no_setup(mut self) -> Builder {
        self.setup = false;
        self
    }

    /// Skips the base commit: the repository is initialised and its files written.
    pub fn no_base(mut self) -> Builder {
        self.base = false;
        self
    }

    /// Commits the base on main first, then runs `setup` on a new branch.
    pub fn setup_on_branch(mut self, branch: &str) -> Builder {
        self.branch = Some(branch.to_string());
        self
    }

    /// The session id the binary sees and the hook payloads carry.
    pub fn session(mut self, id: &str) -> Builder {
        self.session = Some(id.to_string());
        self
    }

    /// No session id: the binary runs as from the person's terminal.
    pub fn no_session(mut self) -> Builder {
        self.session = None;
        self
    }

    pub fn env(mut self, key: &str, value: impl Into<OsString>) -> Builder {
        self.env.push((key.to_string(), Some(value.into())));
        self
    }

    pub fn env_remove(mut self, key: &str) -> Builder {
        self.env.push((key.to_string(), None));
        self
    }

    /// Writes an executable `bin/<name>` in the scratch folder and puts that
    /// folder first on the binary's PATH.
    pub fn bin(self, name: &str, script: &str) -> Builder {
        let bin = self.dir.join("bin");
        std::fs::create_dir_all(&bin).expect("bin folder");
        let file = bin.join(name);
        std::fs::write(&file, script).expect("write bin");
        assert!(Command::new("chmod").arg("+x").arg(&file).status().expect("chmod").success());
        let path = self.env.iter().rev().find(|(k, _)| k == "PATH").and_then(|(_, v)| v.clone()).unwrap_or_else(|| std::env::var_os("PATH").unwrap_or_default());
        if std::env::split_paths(&path).next().as_deref() == Some(bin.as_path()) {
            return self;
        }
        let mut joined = OsString::from(bin);
        joined.push(":");
        joined.push(path);
        self.env("PATH", joined)
    }

    pub fn create(self) -> Project {
        let p = Project { dir: self.dir, session: self.session, env: RefCell::new(self.env) };
        std::fs::create_dir_all(p.repo()).expect("temp project");
        p.git(&["init", "-q", "-b", "main"]);
        for (path, text) in &self.files {
            p.write(path, text);
        }
        match self.branch {
            Some(branch) => {
                p.git(&["add", "-A"]);
                p.git(&["commit", "-q", "--allow-empty", "-m", "base"]);
                p.git(&["checkout", "-q", "-b", &branch]);
                if self.setup {
                    p.setup();
                }
            }
            None => {
                if self.setup {
                    p.setup();
                }
                if self.base {
                    p.git(&["add", "-A"]);
                    p.git(&["commit", "-q", "--allow-empty", "-m", "base"]);
                }
            }
        }
        p
    }
}

/// A scratch git repository the built binary runs in, removed on drop.
pub struct Project {
    pub dir: PathBuf,
    session: Option<String>,
    env: RefCell<Vec<(String, Option<OsString>)>>,
}

impl Project {
    /// `src/app.sh`, `setup`, and both committed as `base` on main.
    pub fn new(name: &str) -> Project {
        Project::builder(name).create()
    }

    pub fn builder(name: &str) -> Builder {
        let dir = temp_dir(name);
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("temp folder");
        Builder {
            dir,
            files: vec![("src/app.sh".to_string(), APP.to_string())],
            setup: true,
            base: true,
            branch: None,
            session: Some(DEFAULT_SESSION.to_string()),
            env: Vec::new(),
        }
    }

    /// The repository: where git and the binary run.
    pub fn repo(&self) -> PathBuf {
        self.dir.join("repo")
    }

    /// The binary's home (`FAMILIAR_AI_HOME`), where setup leaves its marker.
    pub fn home(&self) -> PathBuf {
        self.dir.join("home")
    }

    /// A path in the repository.
    pub fn path(&self, rel: &str) -> PathBuf {
        self.repo().join(rel)
    }

    /// Writes a file in the repository, its folders made.
    pub fn write(&self, rel: &str, text: &str) {
        let full = self.path(rel);
        std::fs::create_dir_all(full.parent().expect("parent")).expect("mkdir");
        std::fs::write(full, text).expect("write");
    }

    pub fn read(&self, rel: &str) -> String {
        std::fs::read_to_string(self.path(rel)).unwrap_or_default()
    }

    pub fn session(&self) -> Option<&str> {
        self.session.as_deref()
    }

    /// Sets (`Some`) or removes (`None`) a variable the binary sees from now on.
    pub fn set_env(&self, key: &str, value: Option<&str>) {
        self.env.borrow_mut().push((key.to_string(), value.map(OsString::from)));
    }

    /// Runs `git <args>` at `at`, which must succeed; returns its trimmed stdout.
    pub fn git_in(&self, at: &Path, args: &[&str]) -> String {
        let out = Command::new("git")
            .current_dir(at)
            .args(["-c", "user.name=t", "-c", "user.email=t@example.com", "-c", "commit.gpgsign=false"])
            .args(args)
            .output()
            .expect("run git");
        assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    }

    /// Runs `git <args>` in the repository, which must succeed; returns its trimmed stdout.
    pub fn git(&self, args: &[&str]) -> String {
        self.git_in(&self.repo(), args)
    }

    pub fn head(&self) -> String {
        self.git(&["rev-parse", "HEAD"])
    }

    /// Commit subjects, oldest first.
    pub fn log(&self) -> Vec<String> {
        self.git(&["log", "--format=%s", "--reverse"]).lines().map(str::to_string).collect()
    }

    /// The built binary, set to run in the repository as `session`.
    pub fn command_as(&self, session: Option<&str>) -> Command {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_familiar-ai"));
        cmd.current_dir(self.repo())
            .env("FAMILIAR_AI_JUDGE", "off")
            .env("FAMILIAR_AI_INVESTIGATOR", "off")
            .env("FAMILIAR_AI_HOME", self.home())
            .env("GIT_AUTHOR_NAME", "t")
            .env("GIT_AUTHOR_EMAIL", "t@example.com")
            .env("GIT_COMMITTER_NAME", "t")
            .env("GIT_COMMITTER_EMAIL", "t@example.com")
            .env_remove("CLAUDE_PROJECT_DIR")
            .env_remove("FAMILIAR_AI_CONVERSATION_ID")
            // a headless session (`claude -p`) running the tests is not the binary's session here
            .env_remove("CLAUDE_CODE_ENTRYPOINT");
        match session {
            Some(s) => cmd.env("CLAUDE_CODE_SESSION_ID", s),
            None => cmd.env_remove("CLAUDE_CODE_SESSION_ID"),
        };
        for (key, value) in self.env.borrow().iter() {
            match value {
                Some(v) => cmd.env(key, v),
                None => cmd.env_remove(key),
            };
        }
        cmd
    }

    /// The built binary, set to run in the repository as this project's session.
    pub fn command(&self) -> Command {
        self.command_as(self.session.as_deref())
    }

    /// Runs `cmd` with `stdin` on its input; returns (exit code, stdout, stderr).
    pub fn output(mut cmd: Command, stdin: &str) -> (i32, String, String) {
        let mut child = cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().expect("spawn familiar-ai");
        let mut pipe = child.stdin.take().expect("stdin is piped");
        // a command that never reads stdin may have exited already; a broken pipe is fine
        if let Err(e) = pipe.write_all(stdin.as_bytes()) {
            assert_eq!(e.kind(), std::io::ErrorKind::BrokenPipe, "write stdin: {e}");
        }
        drop(pipe);
        let out = child.wait_with_output().expect("wait for familiar-ai");
        (out.status.code().unwrap_or(-1), String::from_utf8_lossy(&out.stdout).into_owned(), String::from_utf8_lossy(&out.stderr).into_owned())
    }

    /// Runs the binary as `session` with `args` and `stdin`; returns (exit code, stdout, stderr).
    pub fn run_as(&self, session: Option<&str>, args: &[&str], stdin: &str) -> (i32, String, String) {
        let mut cmd = self.command_as(session);
        cmd.args(args);
        Project::output(cmd, stdin)
    }

    /// Runs the binary with `args` and `stdin`; returns (exit code, stdout, stderr).
    pub fn run(&self, args: &[&str], stdin: &str) -> (i32, String, String) {
        self.run_as(self.session.as_deref(), args, stdin)
    }

    /// Runs the binary at `at` instead of the repository (a clone, a subfolder).
    pub fn run_in(&self, at: &Path, args: &[&str], stdin: &str) -> (i32, String, String) {
        let mut cmd = self.command();
        cmd.current_dir(at).args(args);
        Project::output(cmd, stdin)
    }

    /// `familiar-ai setup`, which must succeed.
    pub fn setup(&self) {
        let (code, out, err) = self.run(&["setup"], "");
        assert_eq!(code, 0, "setup: {out}{err}");
    }

    /// `familiar-ai claim <args>`, which must succeed; returns its stdout.
    pub fn claim(&self, args: &[&str]) -> String {
        let mut argv = vec!["claim"];
        argv.extend_from_slice(args);
        let (code, out, err) = self.run(&argv, "");
        assert_eq!(code, 0, "claim {args:?}: {out}{err}");
        out
    }

    /// `familiar-ai claim <args>`, which may be refused; returns (exit code, stdout, stderr).
    pub fn try_claim(&self, args: &[&str]) -> (i32, String, String) {
        let mut argv = vec!["claim"];
        argv.extend_from_slice(args);
        self.run(&argv, "")
    }

    /// A hook payload: the event, this project's session and repository, then `fields` over them.
    pub fn payload(&self, event: &str, fields: Value) -> Value {
        self.payload_as(self.session.as_deref().unwrap_or(DEFAULT_SESSION), event, fields)
    }

    pub fn payload_as(&self, session: &str, event: &str, fields: Value) -> Value {
        let mut payload = json!({ "hook_event_name": event, "session_id": session, "cwd": self.repo().to_string_lossy() });
        for (k, v) in fields.as_object().cloned().unwrap_or_default() {
            payload[k] = v;
        }
        payload
    }

    /// A captured payload from `tests/fixtures`, re-pointed at this project's session and repository, then `fields` over it.
    pub fn fixture(&self, name: &str, fields: Value) -> Value {
        let mut payload = fixture(name);
        payload["session_id"] = json!(self.session.as_deref().unwrap_or(DEFAULT_SESSION));
        payload["cwd"] = json!(self.repo().to_string_lossy());
        for (k, v) in fields.as_object().cloned().unwrap_or_default() {
            payload[k] = v;
        }
        payload
    }

    /// `familiar-ai hook <event>` with `payload` on stdin; returns (exit code, stdout, stderr).
    pub fn hook_raw(&self, event: &str, payload: &Value) -> (i32, String, String) {
        self.run(&["hook", event], &payload.to_string())
    }

    /// `familiar-ai hook <event>` with `payload`, which must exit 0 and print one JSON object.
    pub fn hook_with(&self, event: &str, payload: &Value) -> Value {
        let (code, out, err) = self.hook_raw(event, payload);
        assert_eq!(code, 0, "hook {event}: {err}");
        serde_json::from_str(out.trim()).unwrap_or_else(|e| panic!("hook {event} printed no JSON ({e}): {out}"))
    }

    /// `familiar-ai hook <event>` with the payload built from `fields`; what it told.
    pub fn hook(&self, event: &str, fields: Value) -> Value {
        self.hook_with(event, &self.payload(event, fields))
    }

    /// `familiar-ai hook <event>` with a captured payload from `tests/fixtures`, `fields` over it.
    pub fn hook_fixture(&self, event: &str, name: &str, fields: Value) -> Value {
        self.hook_with(event, &self.fixture(name, fields))
    }

    /// The hook around a tool call: `tool_name` and `tool_input`, with an empty
    /// `tool_response` after the call.
    pub fn tool(&self, event: &str, tool_name: &str, tool_input: Value) -> Value {
        let mut fields = json!({ "tool_name": tool_name, "tool_input": tool_input });
        if event == "PostToolUse" {
            fields["tool_response"] = json!({});
        }
        self.hook(event, fields)
    }

    /// The hook around a shell command.
    pub fn bash(&self, event: &str, command: &str) -> Value {
        self.tool(event, "Bash", json!({ "command": command }))
    }

    /// The agent's commit: everything added and committed, then the hook after
    /// the tool call; what the hook told.
    pub fn commit(&self, message: &str) -> Value {
        self.git(&["add", "-A"]);
        self.git(&["commit", "-q", "-m", message]);
        self.bash("PostToolUse", &format!("git commit -m '{message}'"))
    }

    /// Everything added and committed with no hook call, as a person or a
    /// script lands it; returns the new HEAD.
    pub fn land(&self, message: &str) -> String {
        self.git(&["add", "-A"]);
        self.git(&["commit", "-q", "--allow-empty", "-m", message]);
        self.head()
    }

    /// What `familiar-ai status` prints.
    pub fn status(&self) -> String {
        self.run(&["status"], "").1
    }

    /// `familiar-ai status --json`, which must succeed.
    pub fn status_json(&self) -> Value {
        let (code, out, err) = self.run(&["status", "--json"], "");
        assert_eq!(code, 0, "status --json: {err}");
        serde_json::from_str(&out).expect("status --json prints JSON")
    }

    /// One criterion's entry in `status --json`.
    pub fn criterion_json(&self, id: &str) -> Value {
        let status = self.status_json();
        let found = status["criteria"].as_array().and_then(|a| a.iter().find(|c| c["id"] == id)).cloned();
        found.unwrap_or_else(|| panic!("no {id} in {status}"))
    }

    /// The `state_name` of one criterion in `status --json`.
    pub fn state_name(&self, id: &str) -> String {
        self.criterion_json(id)["state_name"].as_str().unwrap_or_default().to_string()
    }

    pub fn record_path(&self) -> PathBuf {
        self.path(FAMILIAR_FOLDER).join(RECORD_FILE)
    }

    /// Every fact in the record, in order.
    pub fn facts(&self) -> Vec<Fact> {
        facts_at(&self.repo())
    }

    /// The fields of every fact of one kind, in record order.
    pub fn facts_of(&self, kind: &str) -> Vec<Value> {
        self.facts().into_iter().filter(|f| f.kind == kind).map(|f| Value::Object(f.fields)).collect()
    }

    /// The cases card passed for `c` [draft names]: see [`cases_kept_at`].
    pub fn cases_kept(&self, c: &str) {
        cases_kept_at(&self.repo(), c);
    }

    /// The person's answers on cards other than the cases card [draft name], in record order: what
    /// `facts_of("hold_answered")` read before every full-path plan passed that card.
    pub fn card_answers(&self) -> Vec<Value> {
        self.facts_of("hold_answered").into_iter().filter(|a| a["hold"] != CASES_CARD).collect()
    }
}

/// The person's card on the case-finder's cases [draft name] (the rules' `cases::CARD`).
pub const CASES_CARD: &str = "cases_unconfirmed";

/// The cases card passed for `c` [draft names] (the owner's plan cases-card, 2026-10-25): every
/// full-path part whose contract is whole waits on the case-finder and then on the person's card
/// before its test. A test about something else states both, as the record would hold them: the
/// binary's record of the finder's answer, none found (`cases_found`, outcome `none`), then the
/// person's `keep_planned` on the card (`hold_answered`), told to the agent by the pick's own
/// reply (`told`), as the picker tells a pick of the main conversation. All are appended straight to the record
/// at `repo`, a fixture as `record_replay_first_use` appends the old binary's facts, since no
/// claim may write them. Findings are read from the part's intent on, so one call after the
/// part's intent stands is enough, and a promise claimed again owes no new call.
pub fn cases_kept_at(repo: &Path, c: &str) {
    let record = Record::open_waiting(&repo.join(FAMILIAR_FOLDER).join(RECORD_FILE), std::time::Duration::from_secs(5)).expect("the record");
    let at = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(1.0);
    let obj = |v: Value| v.as_object().cloned().unwrap_or_default();
    let found = obj(json!({ "criterion_id": c, "outcome": "none", "cases": [], "why": "the planned cases cover it", "model": "sonnet", "tokens": 0 }));
    let kept = obj(json!({ "criterion_id": c, "hold": CASES_CARD, "choice": "keep_planned", "typed": "" }));
    let facts = record.append_all(&[("cases_found", found, at), ("hold_answered", kept, at)], familiar_ai::TENANT).expect("the finder's none and the person's keep_planned");
    // the pick is told by its own reply, as the picker tells the main conversation's picks
    let about = obj(json!({ "about": facts[1].t.to_string() }));
    record.append_raw("told", about, at, familiar_ai::TENANT).expect("the pick told");
}

impl Drop for Project {
    /// Kills the process groups of runs the record names, then removes the scratch folder.
    fn drop(&mut self) {
        if self.record_path().exists() {
            let facts = Record::open_waiting(&self.record_path(), std::time::Duration::from_secs(10)).and_then(|r| r.read_all()).unwrap_or_default();
            for f in facts {
                if let Some(pgid) = f.fields.get("pgid").and_then(Value::as_str) {
                    let _ = Command::new("kill").args(["-9", "--", &format!("-{pgid}")]).stderr(Stdio::null()).status();
                }
            }
        }
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// Every fact in the record of the repository at `repo`, in order.
pub fn facts_at(repo: &Path) -> Vec<Fact> {
    Record::open_waiting(&repo.join(FAMILIAR_FOLDER).join(RECORD_FILE), std::time::Duration::from_secs(5)).expect("the record").read_all().expect("read")
}
