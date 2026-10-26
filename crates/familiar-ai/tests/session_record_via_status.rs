//! The session context says how the record is read: through
//! `familiar-ai status`, never opened directly. See
//! docs/decisions.md#the-record-is-a-database-file

mod common;

use std::path::{Path, PathBuf};
use std::time::Duration;

use familiar_ai::{run, Surroundings, FAMILIAR_FOLDER};

const LINE: &str = "The record is a database file: read it with familiar-ai status (or familiar-ai status --json), never open it directly. The plan file is its readable mirror.";

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
fn the_session_context_says_the_record_is_read_through_status() {
    let project = scratch_project("context");
    std::fs::create_dir_all(project.join(FAMILIAR_FOLDER)).unwrap();
    let (code, json) = hook("SessionStart", &project, 50);
    assert_eq!(code, 0);
    let context = json["hookSpecificOutput"]["additionalContext"].as_str().unwrap();
    assert!(context.contains("The record is a database file: read it with familiar-ai status"), "{context}");
    assert!(context.contains("never open it directly"), "{context}");
    assert!(context.contains("The plan file is its readable mirror"), "{context}");
    assert!(context.contains(LINE), "{context}");
    let _ = std::fs::remove_dir_all(&project);
}

// the plan skill carries no copy of the rulebook (the rulebook itself says LINE, above).
#[test]
fn the_plan_skill_has_no_discipline_block() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../plugin/skills/plan/SKILL.md");
    let skill = std::fs::read_to_string(path).unwrap();
    assert!(!skill.contains("<!-- discipline-begin -->") && !skill.contains(LINE));
}
