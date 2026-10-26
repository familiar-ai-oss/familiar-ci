//! The session context tells the agent that t, at and tenant are stamped by
//! the binary and never go in a claim. See
//! docs/decisions.md#the-binary-stamps-position-time-kind-and-tenant

mod common;

use std::path::{Path, PathBuf};
use std::time::Duration;

use familiar_ai::{run, Surroundings, FAMILIAR_FOLDER};

const LINE: &str = "t, at and tenant are stamped by the binary; never put them in a claim.";

fn around(project: &Path) -> Surroundings {
    Surroundings {
        now: 9.0,
        project_dir: Some(project.to_path_buf()),
        cwd: None,
        session_id: None,
        data_dir: None,
        record_wait: Duration::from_millis(50),
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

#[test]
fn the_session_context_says_t_at_and_tenant_are_stamped_by_the_binary() {
    let project: PathBuf = common::temp_dir("stamped");
    let _ = std::fs::remove_dir_all(&project);
    std::fs::create_dir_all(project.join(FAMILIAR_FOLDER)).unwrap();
    let input = format!(
        r#"{{"hook_event_name":"SessionStart","session_id":"s1","cwd":"{}"}}"#,
        project.display()
    );
    let out = run(["hook", "SessionStart"], &input, &around(&project));
    assert_eq!(out.exit_code, 0);
    let json: serde_json::Value = serde_json::from_str(&out.stdout_text).unwrap();
    let context = json["hookSpecificOutput"]["additionalContext"].as_str().unwrap();
    assert!(context.contains(LINE), "{context}");
    let _ = std::fs::remove_dir_all(&project);
}
