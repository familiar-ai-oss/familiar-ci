//! `familiar-ai launch_lane` starts a lane's `claude -p` with the brief as its
//! prompt, the person's words of the current ask seeded into the lane's record,
//! and the conversation id the binary gave it, so the lane's own prompt (the
//! brief) is never recorded as the person's. One session runs per worktree.
//! A scratch `claude` on PATH records its argv, environment, folder and stdin.
//! The launching checkout's record is let go before the lane is seeded.

mod common;

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, Instant};

use common::Project;
use serde_json::{json, Value};

const BRIEF: &str = "THIS LANE: make greet say hello. The orchestrator's brief, not the person's words.\n";
const ASKED: &str = "make greet say hello, not hi";

/// Records each call as `calls/<n>/{argv,env,cwd,stdin}`, `argv` last, then
/// prints one stream-json line and sleeps `$LANE_SLEEP` seconds.
fn fake_claude(calls: &Path) -> String {
    format!(
        "#!/bin/sh\nn=$(ls '{c}' 2>/dev/null | wc -l | tr -d ' ')\nd='{c}'/$n\nmkdir -p \"$d\"\necho $$ > \"$d/pid\"\nenv > \"$d/env\"\npwd > \"$d/cwd\"\ncat > \"$d/stdin\"\nprintf '%s\\n' \"$@\" > \"$d/argv.tmp\" && mv \"$d/argv.tmp\" \"$d/argv\"\necho '{{\"type\":\"system\",\"subtype\":\"init\"}}'\nsleep \"${{LANE_SLEEP:-0}}\"\n",
        c = calls.display()
    )
}

struct Lane {
    p: Project,
    calls: PathBuf,
    plugin: PathBuf,
    brief: PathBuf,
}

/// A governed project whose person asked `ASKED` (an intent names that prompt),
/// a brief file beside it and the scratch `claude`.
fn lane(name: &str) -> Lane {
    let l = bare(name);
    let t = l.say(None, ASKED);
    l.p.claim(&["intent_submitted", "--for", &t.to_string(), "--text", ASKED]);
    l
}

/// A governed project with no prompt and no intent yet, the brief and the scratch `claude`.
fn bare(name: &str) -> Lane {
    let b = Project::builder(name);
    let calls = b.dir().join("calls");
    let plugin = b.dir().join("plugin");
    let p = b.bin("claude", &fake_claude(&calls)).create();
    std::fs::create_dir_all(&plugin).expect("plugin folder");
    std::fs::create_dir_all(&calls).expect("calls folder");
    let brief = p.dir.join("greet.brief.md");
    std::fs::write(&brief, BRIEF).expect("brief");
    Lane { p, calls, plugin, brief }
}

impl Lane {
    /// The person types `prompt` in `session` (None: the launching session); its `t`.
    fn say(&self, session: Option<&str>, prompt: &str) -> u64 {
        let payload = match session {
            Some(s) => self.p.payload_as(s, "UserPromptSubmit", json!({ "prompt": prompt })),
            None => self.p.payload("UserPromptSubmit", json!({ "prompt": prompt })),
        };
        let said = self.p.hook_with("UserPromptSubmit", &payload);
        assert!(said.get("decision").is_none(), "{said}");
        let typed = |f: &&familiar_ai_facts::Fact| f.kind == "person_said" && f.fields.get("text").and_then(Value::as_str) == Some(prompt);
        self.p.facts().iter().rev().find(typed).map(|f| f.t).expect("the person's prompt is recorded")
    }

    fn launch(&self, extra: &[&str]) -> (i32, String, String) {
        let brief = self.brief.to_string_lossy().into_owned();
        let plugin = self.plugin.to_string_lossy().into_owned();
        let mut args = vec!["launch_lane", "--brief", brief.as_str(), "--plugin-dir", plugin.as_str()];
        args.extend_from_slice(extra);
        self.p.run(&args, "")
    }

    /// The lane's folder: beside the repository, named for it and the brief.
    fn folder(&self) -> PathBuf {
        self.p.dir.join("repo-greet")
    }

    /// The `n`th call of the scratch `claude`, once it has written its argv.
    fn call(&self, n: usize) -> Call {
        let d = self.calls.join(n.to_string());
        let until = Instant::now() + Duration::from_secs(20);
        while !d.join("argv").exists() {
            assert!(Instant::now() < until, "claude was not started a {}th time", n + 1);
            std::thread::sleep(Duration::from_millis(50));
        }
        let read = |f: &str| std::fs::read_to_string(d.join(f)).unwrap_or_default();
        Call { argv: read("argv").lines().map(str::to_string).collect(), env: read("env"), cwd: read("cwd").trim().to_string(), stdin: read("stdin"), pid: read("pid").trim().to_string() }
    }
}

struct Call {
    pid: String,
    argv: Vec<String>,
    env: String,
    cwd: String,
    stdin: String,
}

impl Call {
    fn env_of(&self, key: &str) -> Option<String> {
        self.env.lines().find_map(|l| l.strip_prefix(&format!("{key}=")).map(str::to_string))
    }

    /// The value after `flag` in the argv.
    fn after(&self, flag: &str) -> Option<String> {
        self.argv.iter().position(|a| a == flag).and_then(|i| self.argv.get(i + 1)).cloned()
    }
}

fn canon(p: &Path) -> PathBuf {
    std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf())
}

fn said_in(folder: &Path) -> Vec<String> {
    common::project::facts_at(folder).iter().filter(|f| f.kind == "person_said").filter_map(|f| f.fields.get("text").and_then(Value::as_str).map(str::to_string)).collect()
}

#[test]
fn the_brief_is_the_prompt_and_the_lane_carries_the_persons_words() {
    let l = lane("start");
    let (code, out, err) = l.launch(&[]);
    assert_eq!(code, 0, "{out}{err}");
    let folder = l.folder();
    let c = l.call(0);

    // the brief arrives as the prompt, in the lane's folder, as `claude -p`
    assert_eq!(c.stdin, BRIEF);
    assert_eq!(canon(Path::new(&c.cwd)), canon(&folder));
    assert_eq!(c.argv.first().map(String::as_str), Some("-p"), "{:?}", c.argv);
    assert_eq!(c.after("--plugin-dir").map(PathBuf::from), Some(l.plugin.clone()), "{:?}", c.argv);
    assert_eq!(c.after("--output-format").as_deref(), Some("stream-json"), "{:?}", c.argv);
    assert!(c.argv.iter().any(|a| a == "--verbose"), "{:?}", c.argv);
    let session = c.after("--session-id").expect("a session id of its own");

    // the binary's conversation id, the governing binary first on PATH, no session of the launcher's
    assert_eq!(c.env_of("FAMILIAR_AI_CONVERSATION_ID").as_deref(), Some(session.as_str()));
    assert_eq!(c.env_of("CLAUDE_CODE_SESSION_ID"), None, "the launcher's session is not the lane's");
    let first = c.env_of("PATH").and_then(|p| p.split(':').next().map(PathBuf::from)).expect("PATH");
    assert_eq!(canon(&first), canon(Path::new(env!("CARGO_BIN_EXE_familiar-ai")).parent().expect("bin dir")));

    // a branch of its own from the current HEAD
    let branch = l.p.git_in(&folder, &["rev-parse", "--abbrev-ref", "HEAD"]);
    assert_eq!(branch, "lane-greet");
    assert_eq!(l.p.git_in(&folder, &["rev-parse", "HEAD"]), l.p.head());

    // what it prints: the folder, the branch, the log and how to resume
    let shown = folder.display().to_string();
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines[0], format!("familiar-ai: a lane runs in {shown} on the branch lane-greet, carrying 1 prompt of the person's"), "{out}");
    let log = lines[1].strip_prefix("  its log: ").unwrap_or_else(|| panic!("{out}"));
    assert!(log.starts_with(&format!("{shown}/.familiar/")), "{out}");
    assert_eq!(lines[2], format!("  resume it: familiar-ai launch_lane --brief {} --worktree {shown} --resume", l.brief.display()), "{out}");
    let until = Instant::now() + Duration::from_secs(20);
    while !std::fs::read_to_string(log).unwrap_or_default().contains("\"subtype\":\"init\"") {
        assert!(Instant::now() < until, "claude's output never reached {log}");
        std::thread::sleep(Duration::from_millis(50));
    }
    let ignored = std::process::Command::new("git").current_dir(&folder).args(["check-ignore", "-q", log]).status().expect("git");
    assert!(ignored.success(), "the log is git-ignored");

    // the lane's record holds the person's words, not the brief
    assert_eq!(said_in(&folder), [ASKED]);
    // and the lane's own prompt, the brief, is never recorded as the person's
    let mut cmd = l.p.command();
    cmd.current_dir(&folder).env("FAMILIAR_AI_CONVERSATION_ID", &session).args(["hook", "UserPromptSubmit"]);
    let payload = json!({ "hook_event_name": "UserPromptSubmit", "session_id": session, "cwd": folder.to_string_lossy(), "prompt": BRIEF });
    let (code, _, err) = Project::output(cmd, &payload.to_string());
    assert_eq!(code, 0, "{err}");
    assert_eq!(said_in(&folder), [ASKED], "the brief was recorded as the person's words");
}

#[test]
fn a_second_launch_in_the_same_worktree_is_refused_while_the_first_runs() {
    let l = lane("twice");
    l.p.set_env("LANE_SLEEP", Some("30"));
    assert_eq!(l.launch(&[]).0, 0);
    let first = l.call(0);
    let folder = l.folder().display().to_string();
    let (code, out, err) = l.launch(&["--worktree", &folder]);
    assert_eq!((code, out.as_str()), (1, ""), "{err}");
    assert_eq!(
        err,
        format!("familiar-ai: a lane is already running in {folder}; one session runs per worktree · next: wait for it to end, or launch the lane in another folder with --worktree\n")
    );
    let (code, _, err) = l.launch(&["--worktree", &folder, "--resume"]);
    assert_eq!(code, 1, "a resume waits too: {err}");
    assert!(!l.calls.join("1").exists(), "a second claude was started");
    // the scratch claude leads its own process group: ending it ends the lane
    let _ = std::process::Command::new("kill").args(["-9", "--", &format!("-{}", first.pid)]).stderr(Stdio::null()).status();
}

#[test]
fn resume_continues_the_lanes_own_conversation_with_the_brief() {
    let l = lane("resume");
    assert_eq!(l.launch(&[]).0, 0);
    let first = l.call(0);
    let session = first.after("--session-id").expect("session");
    let folder = l.folder().display().to_string();
    std::fs::write(&l.brief, "carry on: the card was answered\n").expect("next brief");
    // the first claude ends on its own; until then the resume is refused as running
    let until = Instant::now() + Duration::from_secs(20);
    let (code, out, err) = loop {
        let got = l.launch(&["--worktree", &folder, "--resume"]);
        if got.0 == 0 || Instant::now() > until {
            break got;
        }
        std::thread::sleep(Duration::from_millis(100));
    };
    assert_eq!(code, 0, "{out}{err}");
    let second = l.call(1);
    assert_eq!(second.after("--resume").as_deref(), Some(session.as_str()), "{:?}", second.argv);
    assert_eq!(second.after("--session-id"), None, "{:?}", second.argv);
    assert_eq!(second.stdin, "carry on: the card was answered\n");
    assert_eq!(second.env_of("FAMILIAR_AI_CONVERSATION_ID").as_deref(), Some(session.as_str()));
    assert_eq!(said_in(&l.folder()), [ASKED], "the person's words are seeded once");
    assert!(out.starts_with(&format!("familiar-ai: the lane in {folder} resumes its conversation")), "{out}");
}

#[test]
fn a_resume_with_no_lane_and_no_plugin_folder_say_what_is_missing() {
    let l = lane("missing");
    let folder = l.folder().display().to_string();
    let (code, _, err) = l.launch(&["--worktree", &folder, "--resume"]);
    assert_eq!(code, 1);
    assert_eq!(err, format!("familiar-ai: no lane was launched in {folder}, so there is nothing to resume · next: familiar-ai launch_lane --brief {} --worktree {folder}\n", l.brief.display()));
    let brief = l.brief.to_string_lossy().into_owned();
    let mut cmd = l.p.command();
    cmd.env_remove("CLAUDE_PLUGIN_ROOT").args(["launch_lane", "--brief", &brief]);
    let (code, _, err) = Project::output(cmd, "");
    assert_eq!(code, 1);
    assert_eq!(
        err,
        format!("familiar-ai: the plugin's folder is not known, and a lane without it runs ungoverned · next: familiar-ai launch_lane --brief {brief} --plugin-dir <the plugin's folder>\n")
    );
    assert!(!l.folder().exists(), "nothing was made");
    assert!(!l.calls.join("0").exists(), "no claude was started");
}

/// An earlier session's prompt, and that session's id.
const STALE: &str = "well this is concerning, who told you the handoff's rule was that";
const EARLIER: &str = "1d0ffee0-0000-4000-8000-000000000000";

/// `at` as the local day, `YYYY-MM-DD`.
fn day(at: f64) -> String {
    #[allow(clippy::cast_possible_truncation)]
    let secs = at.floor() as libc::time_t;
    // SAFETY: an all-zero `tm` is a valid value; localtime_r reads `secs` and writes only into `tm`.
    let tm = unsafe {
        let mut tm: libc::tm = std::mem::zeroed();
        assert!(!libc::localtime_r(&secs, &mut tm).is_null(), "the local time");
        tm
    };
    format!("{:04}-{:02}-{:02}", tm.tm_year + 1900, tm.tm_mon + 1, tm.tm_mday)
}

#[test]
fn a_prompt_from_before_this_session_is_refused_and_nothing_is_launched() {
    let l = bare("stale");
    let t = l.say(Some(EARLIER), STALE);
    let at = l.p.facts().iter().find(|f| f.t == t).map(|f| f.at).expect("the prompt's time");
    let (code, out, err) = l.launch(&[]);
    assert_eq!((code, out.as_str()), (1, ""), "{err}");
    assert_eq!(
        err,
        format!(
            "familiar-ai: no lane is started: the person's latest words, \"{STALE}\" (t {t}), were typed {}, not in this session, and no intent names them · next: type the ask in this session, or claim the intent naming it first: familiar-ai claim intent_submitted --for {t} --text <the ask>\n",
            day(at)
        )
    );
    assert!(!l.folder().exists(), "no worktree was made");
    assert!(!l.calls.join("0").exists(), "no claude was started");
}

#[test]
fn a_prompt_of_this_session_is_carried_and_another_sessions_later_prompt_is_not() {
    let l = bare("fresh");
    l.say(Some(EARLIER), STALE);
    l.say(None, ASKED);
    l.say(Some(EARLIER), "and another thing, typed in the other session");
    let (code, out, err) = l.launch(&[]);
    assert_eq!(code, 0, "{out}{err}");
    let c = l.call(0);
    assert_eq!(c.stdin, BRIEF);
    let shown = l.folder().display().to_string();
    let first = format!("familiar-ai: a lane runs in {shown} on the branch lane-greet, carrying 1 prompt of the person's");
    assert_eq!(out.lines().next(), Some(first.as_str()), "{out}");
    assert_eq!(said_in(&l.folder()), [ASKED], "only the launching session's words travel");
}

/// The committed test of the seeded project. With `LANE_PROBE` set, each run
/// first asks `familiar-ai status` in the launching checkout (`LANE_LAUNCHER`)
/// with the built binary first on PATH, keeps what it said in `LANE_PROBE`,
/// and fails when the status was refused as busy.
const PROBING_TEST: &str = r#"if [ -n "$LANE_PROBE" ]; then
  out=$(cd "$LANE_LAUNCHER" && PATH="$LANE_BIN_DIR:$PATH" familiar-ai status 2>&1)
  printf '%s\n--\n' "$out" >> "$LANE_PROBE"
  case "$out" in *"busy with another step"*) exit 1 ;; esac
fi
sh src/app.sh | grep -qx hello
"#;

#[test]
fn the_launching_checkouts_record_is_free_while_the_lane_is_seeded() {
    let l = bare("seed-free");
    let p = &l.p;
    let t = l.say(None, ASKED);
    // a committed plan whose history the lane's setup reruns: its test, then the code
    p.claim(&["intent_submitted", "--for", &t.to_string(), "--text", ASKED]);
    p.claim(&["behavior", "--behavior_id", "b1", "--text", "the greeting"]);
    p.claim(&["promise", "--promise_id", "p1", "--behavior_id", "b1", "--accepts", "a name", "--gives_back", "a greeting", "--refuses", "nothing", "--cases", r#"[{"case_id":"n1","kind":"normal","given":"a name","expect":"a greeting"},{"case_id":"o1","kind":"opposite","given":"no name","expect":"no greeting"}]"#]);
    p.claim(&["criterion", "--criterion_id", "c1", "--text", "greet says hello", "--path", "full", "--behavior_id", "b1", "--promise_id", "p1"]);
    p.cases_kept("c1"); // the cases card passed (plan cases-card, 2026-10-25) [draft names]
    p.claim(&["place_chosen", "--criterion_id", "c1", "--places", r#"[{"file_path":"src/app.sh","symbol":"greet","line_start":1,"line_end":3}]"#]);
    p.claim(&["test_planned", "--criterion_id", "c1", "--file_path", "tests/test_c1.sh", "--will", "create"]);
    p.claim(&["test_stack_detected", "--project_root", ".", "--test_stack", "sh", "--test_command", "sh tests/test_c1.sh"]);
    p.claim(&["criterion_head", "--criterion_id", "c1"]);
    p.claim(&["criterion_assigned", "--criterion_id", "c1"]);
    p.claim(&["fix_location_category", "--criterion_id", "c1", "--category", "this_repository", "--where", ""]);
    p.write("tests/test_c1.sh", PROBING_TEST);
    p.commit("c1: tests first");
    p.write("src/app.sh", "#!/bin/sh\ngreet() { echo hello; }\ngreet\n");
    p.commit("c1: code");
    assert_eq!(p.state_name("c1"), "green", "{}", p.status());
    p.commit("c1: the plan");

    // from here on each run of the committed test asks the launching checkout's status
    let probe = p.dir.join("probe.txt");
    let bin_dir = Path::new(env!("CARGO_BIN_EXE_familiar-ai")).parent().expect("bin dir").to_string_lossy().into_owned();
    p.set_env("LANE_PROBE", Some(&probe.to_string_lossy()));
    p.set_env("LANE_LAUNCHER", Some(&p.repo().to_string_lossy()));
    p.set_env("LANE_BIN_DIR", Some(&bin_dir));

    let (code, out, err) = l.launch(&[]);
    assert_eq!(code, 0, "{out}{err}");
    let folder = l.folder();

    // the lane's record was seeded from the committed plan and the history, and carries the person's words
    let facts = common::project::facts_at(&folder);
    assert!(facts.iter().any(|f| f.kind == "criterion"), "the lane's record holds the committed plan");
    assert!(facts.iter().any(|f| f.kind == "commit_seen"), "the lane's record holds the rechecked history");
    assert_eq!(said_in(&folder), [ASKED]);

    // the seed ran the test, and each status it asked in the launching checkout was answered
    let asked = std::fs::read_to_string(&probe).unwrap_or_default();
    assert!(!asked.trim().is_empty(), "the seed never ran the committed test");
    assert!(!asked.contains("busy with another step"), "the launching checkout was refused as busy while the lane was seeded:\n{asked}");
    let c = l.call(0);
    assert_eq!(c.stdin, BRIEF);
}
