//! The passphrase is typed in a terminal of the person's own, never in the
//! pane (the owner, 2026-10-17, after lane S9e's live check: a typed character
//! flashed before the mask, and typing outside the field reached the model).
//! `familiar-ai approve` at a terminal reads the passphrase from the terminal
//! with echo off; off a terminal it reads the first line of stdin, as before.

mod common;

use std::process::Command;

use common::Project;
use ssh_key::rand_core::OsRng;
use ssh_key::{Algorithm, LineEnding, PrivateKey};

const PASSPHRASE: &str = "correct horse battery staple";

/// Runs `argv` on a pseudo-terminal, types `typed` a second after it starts
/// (with Enter, unless it is a control character), and prints the exit code,
/// whether the terminal echoes after it ended, and everything it showed.
const PTY: &str = r#"
import os, pty, sys, time, select, termios
pid, fd = pty.fork()
if pid == 0:
    os.execv(sys.argv[2], sys.argv[2:])
out = b''
def drain(wait):
    global out
    end = time.time() + wait
    while time.time() < end:
        r, _, _ = select.select([fd], [], [], 0.1)
        if r:
            try:
                chunk = os.read(fd, 4096)
            except OSError:
                return False
            if not chunk:
                return False
            out += chunk
    return True
drain(1.5)
typed = sys.argv[1].encode()
os.write(fd, typed if len(typed) == 1 and typed[0] < 32 else typed + b'\r')
drain(20)
done, status = os.waitpid(pid, os.WNOHANG)
if done == 0:
    os.kill(pid, 9)
    _, status = os.waitpid(pid, 0)
echo = 'echo on' if termios.tcgetattr(fd)[3] & termios.ECHO else 'echo off'
sys.stdout.write(str(os.waitstatus_to_exitcode(status)) + '\n' + echo + '\n' + out.decode('utf-8', 'replace'))
"#;

/// A project with c1 planned and the person's encrypted key; approve run at a
/// pseudo-terminal that types `typed`: (exit code, echo after, what it showed).
fn at_a_terminal(name: &str, typed: &str) -> (Project, String, String, String) {
    let p = Project::builder(name).session("84be6339-0cc7-40ec-91b9-cae44365931b").env("FAMILIAR_AI_BENCH", "0").create();
    p.claim(&["intent_submitted", "--for", "0", "--text", "greet with hello"]);
    p.claim(&["behavior", "--behavior_id", "b1", "--text", "the greeting"]);
    p.claim(&["promise", "--promise_id", "p1", "--behavior_id", "b1", "--accepts", "a name", "--gives_back", "a greeting", "--refuses", "nothing", "--cases", r#"[{"case_id":"n1","kind":"normal","given":"a name","expect":"a greeting"},{"case_id":"o1","kind":"opposite","given":"no name","expect":"no greeting"}]"#]);
    p.claim(&["criterion", "--criterion_id", "c1", "--text", "greet says hello", "--path", "full", "--behavior_id", "b1", "--promise_id", "p1"]);
    let k = PrivateKey::random(&mut OsRng, Algorithm::Ed25519).expect("key");
    let file = p.dir.join("familiar_ai_approve");
    k.encrypt(&mut OsRng, PASSPHRASE).expect("encrypt").write_openssh_file(&file, LineEnding::LF).expect("key file");

    let cmd = p.command_as(None);
    let program = cmd.get_program().to_owned();
    let mut py = Command::new("python3");
    py.current_dir(p.repo()).envs(cmd.get_envs().filter_map(|(k, v)| v.map(|v| (k.to_owned(), v.to_owned()))));
    for (k, v) in cmd.get_envs() {
        if v.is_none() {
            py.env_remove(k);
        }
    }
    py.args(["-I", "-c", PTY, typed]).arg(&program);
    py.args(["approve", "--criterion_id", "c1", "--hold", "two_failures", "--choice", "release", "--key"]).arg(&file);
    let out = py.output().expect("python3");
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    let mut lines = text.splitn(3, '\n');
    let (code, echo, shown) = (lines.next().unwrap_or("?"), lines.next().unwrap_or("?"), lines.next().unwrap_or(""));
    (p, code.to_string(), echo.to_string(), format!("{shown}{}", String::from_utf8_lossy(&out.stderr)))
}

#[test]
fn at_a_terminal_the_passphrase_is_read_with_echo_off_and_signs() {
    let (p, code, echo, shown) = at_a_terminal("approve-tty", PASSPHRASE);
    assert_eq!(code, "0", "{shown}");
    assert_eq!(echo, "echo on", "the terminal echoes again after");
    assert!(shown.contains("passphrase"), "it asks for the passphrase: {shown}");
    assert!(!shown.contains(PASSPHRASE), "the passphrase was echoed: {shown}");
    assert!(shown.contains("approved and signed"), "{shown}");
    assert_eq!(p.facts_of("hold_answered").len(), 1);
}

#[test]
fn ctrl_c_at_the_prompt_cancels_and_gives_the_terminal_its_echo_back() {
    let (p, code, echo, shown) = at_a_terminal("approve-tty-cancel", "\u{3}");
    assert_ne!(code, "0", "{shown}");
    assert_ne!(code, "-9", "it ended by itself: {shown}");
    assert_eq!(echo, "echo on", "{shown}");
    assert!(p.facts_of("hold_answered").is_empty());
}
