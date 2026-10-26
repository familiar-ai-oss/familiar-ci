//! A `claim` with no fields and a `plan` with nothing piped never block on an
//! open stdin pipe that nothing writes, as a Claude Code Bash tool's stdin is;
//! they answer as at a terminal, and JSON piped and closed is still read.

mod common;

use std::io::Write;
use std::process::{Child, Stdio};
use std::time::{Duration, Instant};

use common::Project;

const SESSION: &str = "11111111-2222-4333-8444-555555555555";

fn project(name: &str) -> Project {
    Project::builder(name).session(SESSION).env("PATH", "/usr/bin:/bin").create()
}

impl Project {
    /// The binary with stdin from `/dev/null`: what a terminal is shown.
    fn at_dev_null(&self, args: &[&str]) -> (i32, String, String) {
        let out = self.command().args(args).stdin(Stdio::null()).output().expect("run familiar-ai");
        (out.status.code().unwrap_or(-1), String::from_utf8_lossy(&out.stdout).into_owned(), String::from_utf8_lossy(&out.stderr).into_owned())
    }

    /// The binary with stdin a pipe held open, given `text` if any; fails after 5 s.
    fn on_a_pipe(&self, args: &[&str], text: Option<&str>) -> (i32, String, String) {
        let mut child = self.command().args(args).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().expect("run familiar-ai");
        let held = child.stdin.take().expect("stdin");
        let held = match text {
            Some(t) => {
                let mut h = held;
                h.write_all(t.as_bytes()).expect("write stdin");
                drop(h);
                None
            }
            None => Some(held),
        };
        let out = within_5s(child, args);
        drop(held);
        (out.status.code().unwrap_or(-1), String::from_utf8_lossy(&out.stdout).into_owned(), String::from_utf8_lossy(&out.stderr).into_owned())
    }
}

fn within_5s(mut child: Child, args: &[&str]) -> std::process::Output {
    let start = Instant::now();
    loop {
        if child.try_wait().expect("try_wait").is_some() {
            return child.wait_with_output().expect("wait");
        }
        if start.elapsed() > Duration::from_secs(5) {
            let _ = child.kill();
            let _ = child.wait();
            panic!("familiar-ai {args:?} was still waiting on stdin after 5 s");
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// An unwritten open pipe gives the terminal's output plus one stderr line.
fn shows_the_form_on_an_open_pipe(p: &Project, args: &[&str], code: i32) {
    let (want_code, want_out, _) = p.at_dev_null(args);
    assert_eq!(want_code, code, "familiar-ai {args:?} < /dev/null");
    let (got_code, got_out, got_err) = p.on_a_pipe(args, None);
    assert_eq!(got_code, want_code, "familiar-ai {args:?} on an open pipe: {got_err}");
    assert_eq!(got_out, want_out, "familiar-ai {args:?} on an open pipe shows the form");
    assert!(
        got_err.contains("familiar-ai: nothing came on stdin in 500 ms; pass the fields on the command line or pipe the JSON"),
        "familiar-ai {args:?} says nothing came: {got_err}"
    );
}

#[test]
fn a_claim_of_an_unknown_kind_lists_the_kinds_instead_of_waiting() {
    let p = project("unknown");
    shows_the_form_on_an_open_pipe(&p, &["claim", "intent"], 1);
}

#[test]
fn a_claim_of_a_kind_alone_shows_its_form_instead_of_waiting() {
    let p = project("kind");
    shows_the_form_on_an_open_pipe(&p, &["claim", "intent_submitted"], 0);
}

#[test]
fn a_bare_plan_shows_the_arrays_form_instead_of_waiting() {
    let p = project("plan");
    shows_the_form_on_an_open_pipe(&p, &["plan"], 0);
}

#[test]
fn a_plan_written_to_the_pipe_and_closed_is_read_as_before() {
    let p = project("written");
    let (code, out, err) = p.on_a_pipe(&["plan"], Some("[]\n"));
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("familiar-ai: 0 lines claimed"), "the empty array is read: {out}{err}");
    assert!(!err.contains("nothing came on stdin"), "{err}");
}
