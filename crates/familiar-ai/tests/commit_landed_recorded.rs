//! Commits are recorded after they land, against real repositories built in
//! a temporary folder with the `git` program.

mod common;

use std::path::{Path, PathBuf};
use std::time::Duration;

use common::Project;
use familiar_ai::commits::{
    last_commit_seen, record_new_commits, CommitsError, CriterionTests, WhatToRead, MOST_COMMITS_AT_ONCE,
};
use familiar_ai::{run, Surroundings, FAMILIAR_FOLDER, RECORD_FILE, TENANT};
use familiar_ai_facts::{CommitSeen, IntentSubmitted, Record};
use familiar_ai_git::Anchor;

impl Project {
    /// A record outside the repository's files, so `git add -A` never sees it.
    fn record(&self) -> Record {
        let path = self.repo().join(".git").join("test-record");
        Record::open_waiting(&path, std::time::Duration::from_secs(5)).unwrap()
    }
}

const TEST_FILE: &str = "tests/test_c1.py";
const CODE_FILE: &str = "src/login.py";

/// The first criterion: its test file and its anchor on lines 2..=3 of the
/// code file at `base`.
fn what(base: &str) -> impl Fn(&familiar_ai_facts::Log) -> WhatToRead + '_ {
    move |_log| WhatToRead {
        plan_test_files: vec![TEST_FILE.to_string()],
        all_tests: vec![CriterionTests { criterion_id: "c1".to_string(), files: vec![TEST_FILE.to_string()], criterion_rev: "r1".to_string() }],
        anchors: vec![Anchor {
            criterion_id: "c1".to_string(),
            file_path: CODE_FILE.to_string(),
            line_start: 2,
            line_end: 3,
            sha: base.to_string(),
        }],
    }
}

fn seen(record: &Record) -> Vec<CommitSeen> {
    record
        .read_by_kind("commit_seen")
        .unwrap()
        .iter()
        .map(|f| f.fields_as::<CommitSeen>().unwrap())
        .collect()
}

/// Returns c1's fingerprint as observed at a commit.
fn observed(record: &Record, commit: &str) -> Option<String> {
    record
        .read_by_kind("tests_observed")
        .unwrap()
        .iter()
        .rev()
        .find(|f| f.fields.get("commit").and_then(|v| v.as_str()) == Some(commit) && f.criterion_id() == Some("c1"))
        .and_then(|f| f.fields.get("tests_hash").and_then(|v| v.as_str()).map(str::to_string))
}

fn base_repo(name: &str) -> (Project, String) {
    let repo = Project::builder(name)
        .no_app()
        .no_setup()
        .file(CODE_FILE, "def check_password(p):\n    ok = False\n    return ok\nprint(1)\n")
        .create();
    let base = repo.head();
    (repo, base)
}

fn submit_intent(record: &Record, sha: &str) {
    let intent = IntentSubmitted {
        text: "let a person log in".to_string(),
        sha: sha.to_string(),
        for_t: None,
    };
    record.append(&intent, 1.0, TENANT).unwrap();
}

#[test]
fn before_an_intent_no_commit_is_recorded() {
    let (repo, base) = base_repo("no-intent");
    repo.write(CODE_FILE, "changed\n");
    repo.land("a change with no run");
    let record = repo.record();
    assert_eq!(last_commit_seen(&record.read_all().unwrap()), None);
    let written = record_new_commits(&record, &repo.repo(), &what(&base), 5.0, TENANT).unwrap();
    assert!(written.is_empty());
    assert_eq!(record.len().unwrap(), 0);
}

#[test]
fn each_commit_that_lands_becomes_one_fact_oldest_first() {
    let (repo, base) = base_repo("lands");
    let record = repo.record();
    submit_intent(&record, &base);
    assert_eq!(
        last_commit_seen(&record.read_all().unwrap()).as_deref(),
        Some(base.as_str())
    );

    // nothing has landed yet
    assert!(
        record_new_commits(&record, &repo.repo(), &what(&base), 5.0, TENANT)
            .unwrap()
            .is_empty()
    );

    repo.write(TEST_FILE, "def t1():\n    assert check_password('x')\n");
    let t1 = repo.land("T1: the tests alone");
    repo.write(
        CODE_FILE,
        "def check_password(p):\n    ok = p == 'x'\n    return ok\nprint(1)\n",
    );
    let k1 = repo.land("K1: the code");

    let written = record_new_commits(&record, &repo.repo(), &what(&base), 7.0, TENANT).unwrap();
    assert_eq!(written.len(), 2);
    // an observation lands beside each commit
    assert_eq!(written.iter().map(|f| f.t).collect::<Vec<_>>(), vec![1, 3]);
    assert!(written
        .iter()
        .all(|f| f.at == 7.0 && f.tenant == TENANT && f.kind == "commit_seen"));

    let facts = seen(&record);
    assert_eq!(
        facts.iter().map(|f| f.commit.as_str()).collect::<Vec<_>>(),
        vec![t1.as_str(), k1.as_str()]
    );
    // the tests-first commit: tests only, no anchor touched
    assert_eq!(facts[0].files, vec![TEST_FILE.to_string()]);
    assert!(facts[0].touches_tests && !facts[0].touches_code);
    assert!(facts[0].anchor_overlap.is_empty());
    // the fingerprint is in c1's observation, not in commit_seen
    assert!(facts[0].tests_hash.is_none());
    assert!(observed(&record, &t1).is_some());
    // the code commit: code only, the anchor's lines changed, the tests as sealed
    assert_eq!(facts[1].files, vec![CODE_FILE.to_string()]);
    assert!(!facts[1].touches_tests && facts[1].touches_code);
    assert_eq!(facts[1].anchor_overlap, vec!["c1".to_string()]);
    assert_eq!(observed(&record, &k1), observed(&record, &t1));

    // asked again with nothing new: nothing is written
    assert!(
        record_new_commits(&record, &repo.repo(), &what(&base), 8.0, TENANT)
            .unwrap()
            .is_empty()
    );
    assert_eq!(record.len().unwrap(), 5, "the intent, two commits, an observation beside each");
    assert_eq!(
        last_commit_seen(&record.read_all().unwrap()).as_deref(),
        Some(k1.as_str())
    );
}

#[test]
fn a_commit_made_outside_the_agent_is_noticed_at_the_next_call() {
    let (repo, base) = base_repo("outside");
    let record = repo.record();
    submit_intent(&record, &base);
    repo.write("notes.txt", "one\n");
    let first = repo.land("first");
    assert_eq!(
        record_new_commits(&record, &repo.repo(), &what(&base), 2.0, TENANT)
            .unwrap()
            .len(),
        1
    );

    // somebody commits from their own terminal; no hook ran for it
    repo.write("notes.txt", "one\ntwo\n");
    let outside = repo.land("made outside the agent");

    let written = record_new_commits(&record, &repo.repo(), &what(&base), 3.0, TENANT).unwrap();
    assert_eq!(written.len(), 1);
    let facts = seen(&record);
    assert_eq!(
        facts.iter().map(|f| f.commit.as_str()).collect::<Vec<_>>(),
        vec![first.as_str(), outside.as_str()]
    );
    // a file that is neither a test nor inside an anchor: code touched, no anchor
    assert!(facts[1].touches_code && !facts[1].touches_tests && facts[1].anchor_overlap.is_empty());
}

#[test]
fn a_commit_rewritten_in_place_is_recorded_under_its_new_id() {
    let (repo, base) = base_repo("amend");
    let record = repo.record();
    submit_intent(&record, &base);
    repo.write(TEST_FILE, "def t1():\n    assert False\n");
    let before = repo.land("tests");
    record_new_commits(&record, &repo.repo(), &what(&base), 2.0, TENANT).unwrap();

    repo.git(&["commit", "-q", "--amend", "-m", "tests, reworded"]);
    let after = repo.head();
    assert_ne!(before, after);

    let written = record_new_commits(&record, &repo.repo(), &what(&base), 3.0, TENANT).unwrap();
    assert_eq!(written.len(), 1);
    let facts = seen(&record);
    assert_eq!(facts[1].commit, after);
    // the id changed, the content did not: same fingerprint observed, found by content
    assert_eq!(observed(&record, &before), observed(&record, &after));
    assert!(observed(&record, &after).is_some());
}

#[test]
fn a_head_moved_back_writes_nothing_and_the_next_commit_is_still_seen() {
    let (repo, base) = base_repo("reset");
    let record = repo.record();
    submit_intent(&record, &base);
    repo.write("a.txt", "1\n");
    repo.land("one");
    record_new_commits(&record, &repo.repo(), &what(&base), 2.0, TENANT).unwrap();

    repo.git(&["reset", "-q", "--hard", &base]);
    assert!(
        record_new_commits(&record, &repo.repo(), &what(&base), 3.0, TENANT)
            .unwrap()
            .is_empty()
    );

    repo.write("b.txt", "2\n");
    let next = repo.land("two, from the base again");
    let written = record_new_commits(&record, &repo.repo(), &what(&base), 4.0, TENANT).unwrap();
    assert_eq!(written.len(), 1);
    assert_eq!(seen(&record).last().unwrap().commit, next);
}

#[test]
fn a_project_that_is_a_folder_inside_the_repository_works() {
    let (repo, base) = base_repo("inside");
    let record = repo.record();
    submit_intent(&record, &base);
    repo.write("service/app.py", "x = 1\n");
    let c = repo.land("in a folder");
    let project = repo.path("service");
    let written = record_new_commits(&record, &project, &what(&base), 2.0, TENANT).unwrap();
    assert_eq!(written.len(), 1);
    assert_eq!(seen(&record)[0].commit, c);
}

#[test]
fn more_commits_than_can_be_read_at_once_is_an_error_and_writes_nothing() {
    let (repo, base) = base_repo("many");
    let record = repo.record();
    submit_intent(&record, &base);
    for i in 0..=MOST_COMMITS_AT_ONCE {
        repo.git(&["commit", "-q", "--allow-empty", "-m", &format!("empty {i}")]);
    }
    let got = record_new_commits(&record, &repo.repo(), &what(&base), 2.0, TENANT);
    assert!(
        matches!(got, Err(CommitsError::TooMany(n)) if n == MOST_COMMITS_AT_ONCE + 1),
        "{got:?}"
    );
    assert_eq!(record.len().unwrap(), 1);
}

#[test]
fn a_folder_that_is_no_repository_is_an_error_not_a_guess() {
    let dir = common::temp_dir("norepo");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let record = Record::open_waiting(&dir.join("record"), std::time::Duration::from_secs(5)).unwrap();
    submit_intent(&record, "0123456789012345678901234567890123456789");
    let no_reads = |_: &familiar_ai_facts::Log| WhatToRead::default();
    assert!(matches!(
        record_new_commits(&record, &dir, &no_reads, 2.0, TENANT),
        Err(CommitsError::Git(_))
    ));
    assert_eq!(record.len().unwrap(), 1);
    drop(record);
    let _ = std::fs::remove_dir_all(&dir);
}

// ---- the hook opening the record ----

fn around(project: &Path, wait_ms: u64) -> Surroundings {
    Surroundings {
        now: 9.0,
        project_dir: Some(project.to_path_buf()),
        cwd: None,
        session_id: None,
        data_dir: None,
        record_wait: Duration::from_millis(wait_ms),
        test_run_wait: Duration::from_secs(1),
        run_inline: Duration::ZERO,
        runner: None,
        judge_dir: None,
        judge_wait: Duration::ZERO,
        investigator: None,
        conversation: None,
        bench: false,
    }
}

fn hook(event: &str, project: &Path, wait_ms: u64) -> (i32, serde_json::Value) {
    let input = format!(
        r#"{{"hook_event_name":"{event}","session_id":"s1","cwd":"{}","tool_name":"Edit","tool_input":{{"file_path":"a.txt"}}}}"#,
        project.display()
    );
    let out = run(["hook", event], &input, &around(project, wait_ms));
    let json = serde_json::from_str(&out.stdout_text).unwrap_or(serde_json::Value::Null);
    (out.exit_code, json)
}

fn scratch_project(name: &str) -> PathBuf {
    let dir = common::temp_dir(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn a_project_familiar_was_never_used_in_gets_nothing_created() {
    let project = scratch_project("untouched");
    let (code, json) = hook("PreToolUse", &project, 50);
    assert_eq!(code, 0);
    assert!(json["hookSpecificOutput"]["permissionDecision"].is_null());
    assert!(!project.join(FAMILIAR_FOLDER).exists());
    assert_eq!(std::fs::read_dir(&project).unwrap().count(), 0);
    let _ = std::fs::remove_dir_all(&project);
}

#[test]
fn with_the_folder_there_the_hook_opens_the_record_and_lets_go_of_it() {
    let project = scratch_project("opens");
    std::fs::create_dir_all(project.join(FAMILIAR_FOLDER)).unwrap();
    // an empty record holds every edit on `planning_incomplete`; a stop goes
    let (code, json) = hook("SessionStart", &project, 50);
    assert_eq!(code, 0);
    assert!(json["hookSpecificOutput"]["permissionDecision"].is_null());
    // the mode line first, then the discipline, then the whole picture
    let context = json["hookSpecificOutput"]["additionalContext"].as_str().unwrap();
    assert!(context.starts_with("familiar-ai: idle"), "{context}");
    assert!(context.contains("familiar-ai: proof that the work is done"), "{context}");
    assert!(context.contains("\nfamiliar-ai: the plan is still being written"), "{context}");
    assert!(context.contains("claim it: familiar-ai claim intent_submitted"), "{context}");
    let (code, json) = hook("PreToolUse", &project, 50);
    assert_eq!(code, 0);
    assert_eq!(json["hookSpecificOutput"]["permissionDecision"], "deny");
    assert!(json["hookSpecificOutput"]["permissionDecisionReason"]
        .as_str()
        .unwrap()
        .starts_with("familiar-ai: the plan comes first"));
    // no run is open, so a stop goes
    let (code, json) = hook("Stop", &project, 50);
    assert_eq!(code, 0);
    assert!(json.get("decision").is_none(), "{json}");
    let path = project.join(FAMILIAR_FOLDER).join(RECORD_FILE);
    assert!(path.is_file());
    // the hook holds nothing afterwards, and wrote no fact
    assert_eq!(Record::open_waiting(&path, std::time::Duration::from_secs(5)).unwrap().len().unwrap(), 0);
    let _ = std::fs::remove_dir_all(&project);
}

#[test]
fn a_record_held_by_another_step_refuses_the_tool_call_and_passes_the_rest() {
    let project = scratch_project("busy");
    std::fs::create_dir_all(project.join(FAMILIAR_FOLDER)).unwrap();
    let holder = Record::open_waiting(&project.join(FAMILIAR_FOLDER).join(RECORD_FILE), std::time::Duration::from_secs(5)).unwrap();

    let (code, json) = hook("PreToolUse", &project, 100);
    assert_eq!(code, 0);
    assert_eq!(json["hookSpecificOutput"]["permissionDecision"], "deny");
    assert!(json["hookSpecificOutput"]["permissionDecisionReason"]
        .as_str()
        .unwrap()
        .contains("busy with another step"));

    for event in [
        "UserPromptSubmit",
        "Stop",
        "SubagentStop",
        "SessionStart",
        "PostToolUse",
    ] {
        let (code, json) = hook(event, &project, 100);
        assert_eq!(code, 0, "{event}");
        assert!(
            json.get("decision").is_none(),
            "{event} must not be blocked"
        );
        if matches!(event, "Stop" | "SubagentStop") {
            // Claude Code continues the turn on a stop's additionalContext (lane bench-stop F14)
            assert!(json.get("hookSpecificOutput").is_none(), "{event}: {json}");
        }
        assert!(
            json["systemMessage"]
                .as_str()
                .unwrap()
                .contains("busy with another step"),
            "{event}"
        );
    }
    drop(holder);
    let (_, json) = hook("PreToolUse", &project, 100);
    assert!(
        !json["hookSpecificOutput"]["permissionDecisionReason"]
            .as_str()
            .unwrap()
            .contains("busy with another step"),
        "free again once the holder lets go: the rules answer, not the wait"
    );
    let _ = std::fs::remove_dir_all(&project);
}

#[test]
fn a_record_that_cannot_be_read_refuses_the_tool_call_and_says_why() {
    let project = scratch_project("unreadable");
    std::fs::create_dir_all(project.join(FAMILIAR_FOLDER)).unwrap();
    std::fs::write(
        project.join(FAMILIAR_FOLDER).join(RECORD_FILE),
        b"this is not a record",
    )
    .unwrap();

    let (code, json) = hook("PreToolUse", &project, 50);
    assert_eq!(code, 0);
    assert_eq!(json["hookSpecificOutput"]["permissionDecision"], "deny");
    assert!(json["hookSpecificOutput"]["permissionDecisionReason"]
        .as_str()
        .unwrap()
        .contains("cannot read its record"));
    let (code, json) = hook("Stop", &project, 50);
    assert_eq!(code, 0);
    assert!(json.get("decision").is_none());
    assert!(json["systemMessage"]
        .as_str()
        .unwrap()
        .contains("cannot read its record"));
    // the file is left exactly as it was
    assert_eq!(
        std::fs::read(project.join(FAMILIAR_FOLDER).join(RECORD_FILE)).unwrap(),
        b"this is not a record"
    );
    let _ = std::fs::remove_dir_all(&project);
}
