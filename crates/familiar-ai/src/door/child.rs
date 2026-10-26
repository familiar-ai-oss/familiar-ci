//! Run a child program such as the judge or the investigator under the
//! person's own Claude Code login, with the prompt on stdin and a time limit.
//! The hook's environment is handed down unread and is never logged.

use std::io::{Read, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

pub use familiar_ai_run::STRIPPED;

/// How a child ended.
#[derive(Debug, Clone, PartialEq)]
pub enum Ended {
    /// It exited with this code, or `None` when a signal ended it.
    Exited(Option<i32>),
    TimedOut,
    NotStarted(String),
}

/// How a child ended, how long it took and what it printed.
#[derive(Debug, Clone, PartialEq)]
pub struct Ran {
    pub ended: Ended,
    pub wall: Duration,
    pub stdout: String,
    pub stderr: String,
}

impl Ran {
    /// The stdout, only when the child exited 0.
    pub fn answer(&self) -> Option<&str> {
        (self.ended == Ended::Exited(Some(0))).then_some(self.stdout.as_str())
    }

    /// How the child ended, in a few words such as `exit 1`.
    pub fn how(&self) -> String {
        match &self.ended {
            Ended::Exited(Some(c)) => format!("exit {c}"),
            Ended::Exited(None) => "ended by a signal".to_string(),
            Ended::TimedOut => format!("stopped after {} s", self.wall.as_secs()),
            Ended::NotStarted(e) => format!("could not start: {e}"),
        }
    }
}

/// How much of each of a child's streams a log keeps.
pub const LOGGED: usize = 2_000;

/// An argv written for display, quoting empty arguments and ones with spaces.
pub fn shown(argv: &[String]) -> String {
    argv.iter().map(|a| if a.is_empty() || a.contains(' ') { format!("'{a}'") } else { a.clone() }).collect::<Vec<_>>().join(" ")
}

/// Append one call and the start of its stdout and stderr to the log at
/// `path`. The log is a diagnostic only, so a write failure is ignored.
pub fn log(path: &Path, head_line: &str, ran: &Ran, tail: &str) {
    let entry = format!(
        "--- {head_line} · {} · {:.1} s{tail}\nstdout (first {LOGGED} characters):\n{}\nstderr (first {LOGGED} characters):\n{}\n",
        ran.how(),
        ran.wall.as_secs_f64(),
        head(&ran.stdout, LOGGED),
        head(&ran.stderr, LOGGED),
    );
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = f.write_all(entry.as_bytes());
    }
}

pub fn head(s: &str, n: usize) -> &str {
    match s.char_indices().nth(n) {
        Some((i, _)) => &s[..i],
        None => s,
    }
}

/// Run `program args` in `dir` with `prompt` on stdin and `set` added to the
/// inherited environment minus `STRIPPED`, killing it after `wait`.
pub fn run(program: &str, args: &[String], dir: &Path, set: &[(&str, &str)], prompt: &str, wait: Duration) -> Ran {
    let start = Instant::now();
    let not_started = |e: String| Ran { ended: Ended::NotStarted(e), wall: start.elapsed(), stdout: String::new(), stderr: String::new() };
    use std::os::unix::process::CommandExt;
    let mut cmd = Command::new(program);
    // its own process group, so a stop at its time takes its grandchildren too
    cmd.args(args).current_dir(dir).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).process_group(0);
    for name in STRIPPED {
        cmd.env_remove(name);
    }
    for (k, v) in set {
        cmd.env(k, v);
    }
    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => return not_started(e.to_string()),
    };
    if let Some(mut stdin) = child.stdin.take() {
        let prompt = prompt.to_string();
        thread::spawn(move || {
            let _ = stdin.write_all(prompt.as_bytes());
        });
    }
    let read = |pipe: Option<Box<dyn Read + Send>>| {
        let (send, got) = mpsc::channel();
        thread::spawn(move || {
            let mut s = String::new();
            if let Some(mut p) = pipe {
                let mut bytes = Vec::new();
                let _ = p.read_to_end(&mut bytes);
                s = String::from_utf8_lossy(&bytes).into_owned();
            }
            let _ = send.send(s);
        });
        got
    };
    let out = read(child.stdout.take().map(|p| Box::new(p) as Box<dyn Read + Send>));
    let err = read(child.stderr.take().map(|p| Box::new(p) as Box<dyn Read + Send>));
    let ended = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Ended::Exited(status.code()),
            Ok(None) if start.elapsed() < wait => thread::sleep(Duration::from_millis(50)),
            Ok(None) => {
                kill_group(&mut child);
                let _ = child.wait();
                break Ended::TimedOut;
            }
            Err(e) => {
                kill_group(&mut child);
                let _ = child.wait();
                break Ended::NotStarted(e.to_string());
            }
        }
    };
    let wall = start.elapsed();
    // A grandchild still holding a pipe open must not hold the hook.
    let take = |r: mpsc::Receiver<String>| r.recv_timeout(Duration::from_secs(2)).unwrap_or_default();
    Ran { ended, wall, stdout: take(out), stderr: take(err) }
}

/// Kill the child's whole process group (its id is the group's), then the child, and reap it.
fn kill_group(child: &mut std::process::Child) {
    let group = format!("-{}", child.id());
    let _ = Command::new("kill").args(["-9", "--", &group]).stdout(Stdio::null()).stderr(Stdio::null()).status();
    let _ = child.kill();
}

/// Start `program args` in `dir`, detached in a process group of its own,
/// with the file `stdin` on its input and its stdout and stderr appended to
/// the two `logs`; the environment as in [`run`], also without `unset`. Returns the
/// child's id, which is its group's; nothing waits for it.
pub fn detached(program: &Path, args: &[String], dir: &Path, set: &[(&str, &str)], unset: &[&str], stdin: &Path, logs: [&Path; 2]) -> Result<u32, String> {
    use std::os::unix::process::CommandExt;
    let append = |p: &Path| std::fs::OpenOptions::new().create(true).append(true).open(p).map_err(|e| format!("{}: {e}", p.display()));
    let mut cmd = Command::new(program);
    cmd.args(args)
        .current_dir(dir)
        .stdin(std::fs::File::open(stdin).map_err(|e| format!("{}: {e}", stdin.display()))?)
        .stdout(append(logs[0])?)
        .stderr(append(logs[1])?)
        .process_group(0);
    for name in STRIPPED.iter().chain(unset) {
        cmd.env_remove(name);
    }
    for (k, v) in set {
        cmd.env(k, v);
    }
    cmd.spawn().map(|c| c.id()).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn script(dir: &Path, name: &str, body: &str) -> String {
        let p = dir.join(name);
        let _ = std::fs::write(&p, format!("#!/bin/sh\n{body}"));
        let _ = std::process::Command::new("chmod").arg("+x").arg(&p).status();
        p.to_string_lossy().into_owned()
    }

    #[test]
    fn a_child_stopped_at_its_time_takes_its_grandchildren_with_it() {
        // review 3 (S7b): the child is its own process group, and the whole group is killed
        let dir = std::env::temp_dir().join(format!("familiar-ai-child-group-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let pid_file = dir.join("grandchild");
        let slow = script(&dir, "spawns", &format!("sleep 30 &\necho $! > {}\nwait\n", pid_file.display()));
        let ran = run(&slow, &[], &dir, &[], "", Duration::from_millis(500));
        assert_eq!(ran.ended, Ended::TimedOut);
        let pid = std::fs::read_to_string(&pid_file).unwrap_or_default().trim().to_string();
        assert!(!pid.is_empty(), "the grandchild started");
        std::thread::sleep(Duration::from_millis(300));
        let alive = std::process::Command::new("kill").args(["-0", &pid]).stderr(std::process::Stdio::null()).status().is_ok_and(|s| s.success());
        assert!(!alive, "the grandchild {pid} outlived the child");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_environment_is_handed_down_but_two_names_and_the_time_is_kept() {
        let dir = std::env::temp_dir().join(format!("familiar-ai-child-unit-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let env = script(&dir, "env", "cat > /dev/null; echo \"$*|${CLAUDECODE-unset}|${CLAUDE_PROJECT_DIR-unset}|$X_SET|${PATH:+path}|$(pwd)\"; echo err >&2; exit 3\n");
        let ran = run(&env, &["-p".into(), "a b".into()], &dir, &[("X_SET", "yes")], "prompt", Duration::from_secs(10));
        let here = std::fs::canonicalize(&dir).unwrap_or(dir.clone());
        assert_eq!(ran.stdout, format!("-p a b|unset|unset|yes|path|{}\n", here.display()));
        assert_eq!((ran.ended.clone(), ran.stderr.as_str(), ran.how()), (Ended::Exited(Some(3)), "err\n", "exit 3".to_string()));
        assert_eq!(ran.answer(), None, "only exit 0 is an answer");
        let slow = script(&dir, "slow", "sleep 30\n");
        let ran = run(&slow, &[], &dir, &[], "", Duration::from_millis(300));
        assert_eq!(ran.ended, Ended::TimedOut);
        assert!(ran.wall < Duration::from_secs(5) && ran.how().starts_with("stopped after"));
        let none = run("no-such-program-familiar-ai", &[], &dir, &[], "", Duration::from_secs(1));
        assert!(matches!(none.ended, Ended::NotStarted(_)) && none.how().starts_with("could not start: "));
        assert_eq!(head("héllo", 2), "hé");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
