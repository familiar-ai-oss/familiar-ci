//! Gap recordings (a probe's output, Familiar's own, never the person's code) queue in
//! the project's `.familiar/runs/gaps/<run_id>/<probe>/` [draft]. They leave the machine
//! only after the person's yes, which is the machine's and not a project's, and only once
//! an endpoint is named, which the owner does: until then the sender does nothing and the
//! recordings stay where they are (plan A-6, 2026-10-19).

use std::path::{Path, PathBuf};

use familiar_ai::door::share::{self, Choice, Sent};
use serde_json::Value;

const ENDPOINT: &str = "https://gaps.example.invalid/in";

/// A scratch folder named for the test and the process, removed on drop.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Scratch {
        let dir = std::env::temp_dir().join(format!("familiar-ai-share-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("scratch folder");
        Scratch(dir)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// One recording in the fixture layout the plan names: cmd, exit, stdout, stderr, version and report/.
fn recording(root: &Path, run: &str, probe: &str) -> PathBuf {
    let dir = root.join(".familiar/runs/gaps").join(run).join(probe);
    std::fs::create_dir_all(dir.join("report")).expect("recording folder");
    for (name, text) in [("cmd", "python -m pytest\n"), ("exit", "1\n"), ("stdout", "collected 1 item\n"), ("stderr", ""), ("version", "pytest 9.1.1\n")] {
        std::fs::write(dir.join(name), text).expect("recording file");
    }
    std::fs::write(dir.join("report/results.jsonl"), "{\"type\":\"session\"}\n").expect("report");
    dir
}

fn ids(root: &Path) -> Vec<String> {
    share::queued(root).iter().map(|r| format!("{}/{}", r.run_id, r.probe)).collect()
}

#[test]
fn the_queue_is_every_recording_not_yet_sent_in_order() {
    let s = Scratch::new("queue");
    assert!(share::queued(s.path()).is_empty(), "no folder, no queue");
    recording(s.path(), "d-2", "P1");
    recording(s.path(), "d-1", "P3");
    recording(s.path(), "d-1", "P2");
    // what is not a recording is not queued: a file among the runs, a run with no probe in it
    std::fs::write(s.path().join(".familiar/runs/gaps/notes.txt"), "x").expect("a stray file");
    std::fs::create_dir_all(s.path().join(".familiar/runs/gaps/d-3")).expect("an empty run");
    assert_eq!(ids(s.path()), ["d-1/P2", "d-1/P3", "d-2/P1"]);
    std::fs::write(s.path().join(".familiar/runs/gaps/d-1/P2/sent"), "sent: 1\n").expect("sent");
    assert_eq!(ids(s.path()), ["d-1/P3", "d-2/P1"]);
}

#[test]
fn only_a_plain_yes_in_the_marker_folder_is_a_yes() {
    let s = Scratch::new("choice");
    assert_eq!(share::choice(None), Choice::Unasked);
    assert_eq!(share::choice(Some(s.path())), Choice::Unasked);
    for (text, want) in [
        ("share: on\nat: 1\n", Choice::On),
        ("share: off\nat: 1\n", Choice::Off),
        ("share: on", Choice::On),
        ("share: ON\n", Choice::Unasked),
        ("share: onward\n", Choice::Unasked),
        ("yes please\n", Choice::Unasked),
        ("", Choice::Unasked),
    ] {
        std::fs::write(s.path().join("share"), text).expect("the choice");
        assert_eq!(share::choice(Some(s.path())), want, "{text:?}");
    }
}

#[test]
fn nothing_leaves_without_the_persons_yes() {
    let s = Scratch::new("noyes");
    recording(s.path(), "d-1", "P2");
    for choice in [Choice::Unasked, Choice::Off] {
        let mut posted = 0;
        let sent = share::send_queued(s.path(), choice, Some(ENDPOINT), 1.0, &mut |_, _| {
            posted += 1;
            Ok(())
        });
        assert_eq!(sent, Sent::NotShared, "{choice:?}");
        assert_eq!(posted, 0, "{choice:?}");
        assert_eq!(ids(s.path()), ["d-1/P2"], "{choice:?}");
    }
}

#[test]
fn a_yes_with_no_endpoint_named_sends_nothing_and_none_is_named_yet() {
    let s = Scratch::new("noendpoint");
    recording(s.path(), "d-1", "P2");
    let mut posted = 0;
    let sent = share::send_queued(s.path(), Choice::On, None, 1.0, &mut |_, _| {
        posted += 1;
        Ok(())
    });
    assert_eq!(sent, Sent::NoEndpoint);
    assert_eq!(posted, 0);
    assert_eq!(ids(s.path()), ["d-1/P2"]);
    // the endpoint is the owner's to name: until the owner does, the binary has none
    assert_eq!(share::ENDPOINT, None);
}

#[test]
fn a_yes_and_an_endpoint_send_each_recording_once_with_its_own_files_only() {
    let s = Scratch::new("send");
    let first = recording(s.path(), "d-1", "P2");
    // a file outside the recording's layout is never sent: nothing of the person's goes
    std::fs::write(first.join("person_code.py"), "secret = 1\n").expect("a stray file");
    recording(s.path(), "d-1", "P3");

    let mut posts: Vec<(String, Value)> = Vec::new();
    let sent = share::send_queued(s.path(), Choice::On, Some(ENDPOINT), 1_759_700_000.0, &mut |to, payload| {
        posts.push((to.to_string(), serde_json::from_str(payload).expect("the payload is JSON")));
        Ok(())
    });
    assert_eq!(sent, Sent::Queue { sent: 2, failed: None, waiting: 0 });
    assert_eq!(posts.len(), 2);
    let (to, payload) = &posts[0];
    assert_eq!(to, ENDPOINT);
    assert_eq!((&payload["run_id"], &payload["probe"]), (&Value::from("d-1"), &Value::from("P2")));
    let files = payload["files"].as_object().expect("files");
    let mut names: Vec<&str> = files.keys().map(String::as_str).collect();
    names.sort_unstable();
    assert_eq!(names, ["cmd", "exit", "report/results.jsonl", "stderr", "stdout", "version"]);
    assert_eq!(files["exit"], "1\n");
    assert_eq!(files["version"], "pytest 9.1.1\n");
    assert!(!payload.to_string().contains("secret"), "{payload}");

    // each is marked as sent, and a second call has nothing to send
    assert_eq!(std::fs::read_to_string(first.join("sent")).expect("marked"), "sent: 1759700000\nto: https://gaps.example.invalid/in\n");
    assert!(first.join("person_code.py").is_file(), "what was not sent is left as it was");
    let again = share::send_queued(s.path(), Choice::On, Some(ENDPOINT), 2.0, &mut |_, _| panic!("a recording was sent twice"));
    assert_eq!(again, Sent::Queue { sent: 0, failed: None, waiting: 0 });
}

#[test]
fn a_failed_post_leaves_the_rest_queued_and_the_next_call_goes_on_from_there() {
    let s = Scratch::new("fail");
    for probe in ["P1", "P2", "P3"] {
        recording(s.path(), "d-1", probe);
    }
    let mut calls = 0;
    let sent = share::send_queued(s.path(), Choice::On, Some(ENDPOINT), 1.0, &mut |_, _| {
        calls += 1;
        if calls == 2 {
            Err("the endpoint is down".to_string())
        } else {
            Ok(())
        }
    });
    assert_eq!(sent, Sent::Queue { sent: 1, failed: Some("the endpoint is down".to_string()), waiting: 2 });
    assert_eq!(calls, 2, "it stops at the first failure");
    assert_eq!(ids(s.path()), ["d-1/P2", "d-1/P3"]);

    let sent = share::send_queued(s.path(), Choice::On, Some(ENDPOINT), 2.0, &mut |_, _| Ok(()));
    assert_eq!(sent, Sent::Queue { sent: 2, failed: None, waiting: 0 });
    assert!(ids(s.path()).is_empty());
}

/// What `send_queued` posted for the one queued recording.
fn posted_once(root: &Path) -> Value {
    let mut got = None;
    let sent = share::send_queued(root, Choice::On, Some(ENDPOINT), 1.0, &mut |_, payload| {
        got = Some(serde_json::from_str::<Value>(payload).expect("the payload is JSON"));
        Ok(())
    });
    assert_eq!(sent, Sent::Queue { sent: 1, failed: None, waiting: 0 });
    got.expect("one post")
}

#[test]
fn a_link_is_never_followed_and_a_big_file_is_cut() {
    let s = Scratch::new("limits");
    let rec = recording(s.path(), "d-1", "P2");
    // a link standing in for a file of the recording points at something of the person's: not sent
    let theirs = s.path().join("person_secret.txt");
    std::fs::write(&theirs, "the person's own file\n").expect("their file");
    std::fs::remove_file(rec.join("stderr")).expect("remove");
    std::os::unix::fs::symlink(&theirs, rec.join("stderr")).expect("link");
    std::os::unix::fs::symlink(&theirs, rec.join("report/link.txt")).expect("link in report");
    // a file past the cap is cut, and says so
    std::fs::write(rec.join("stdout"), "x".repeat(300 * 1024)).expect("big file");

    let payload = posted_once(s.path());
    assert!(!payload.to_string().contains("the person's own file"), "a linked file was sent");
    let files = payload["files"].as_object().expect("files");
    assert!(!files.contains_key("stderr") && !files.contains_key("report/link.txt"), "{:?}", files.keys().collect::<Vec<_>>());
    let stdout = files["stdout"].as_str().expect("stdout");
    assert!(stdout.starts_with("xxxx") && stdout.len() < 300 * 1024, "{}", stdout.len());
    assert!(stdout.ends_with("[cut: the rest of the file is not sent]\n"), "the cut is said");
}

#[test]
fn the_report_folder_is_sent_to_its_depth_and_no_further() {
    let s = Scratch::new("depth");
    let rec = recording(s.path(), "d-1", "P2");
    for (rel, text) in [("report/surefire-reports/TEST-a.xml", "<a/>"), ("report/a/b/c/deep.txt", "deep"), ("report/a/b/c/d/too_deep.txt", "too deep")] {
        let path = rec.join(rel);
        std::fs::create_dir_all(path.parent().expect("parent")).expect("folders");
        std::fs::write(path, text).expect("report file");
    }
    let payload = posted_once(s.path());
    let files = payload["files"].as_object().expect("files");
    let mut names: Vec<&str> = files.keys().map(String::as_str).filter(|n| n.starts_with("report/")).collect();
    names.sort_unstable();
    assert_eq!(names, ["report/a/b/c/deep.txt", "report/results.jsonl", "report/surefire-reports/TEST-a.xml"]);
}
