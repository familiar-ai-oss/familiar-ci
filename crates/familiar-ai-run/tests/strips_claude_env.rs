//! A project's test command never sees the hook's `CLAUDE_PROJECT_DIR` or `CLAUDECODE`.

use std::time::Duration;

use familiar_ai_run::run_test_command;

#[test]
fn the_test_command_does_not_see_the_hooks_claude_variables() {
    // One test in this binary, so nothing else reads the environment while it is set.
    let before = ["CLAUDE_PROJECT_DIR", "CLAUDECODE"].map(std::env::var_os);
    // SAFETY: this is the only test in the binary, so no other thread touches the environment.
    unsafe {
        std::env::set_var("CLAUDE_PROJECT_DIR", "/the/hooks/worktree");
        std::env::set_var("CLAUDECODE", "1");
    }

    let dir = std::env::current_dir().unwrap();
    let out = run_test_command(
        r#"echo "${CLAUDE_PROJECT_DIR-unset}|${CLAUDECODE-unset}""#,
        &dir,
        Duration::from_secs(30),
    );

    // SAFETY: as above.
    unsafe {
        for (name, value) in ["CLAUDE_PROJECT_DIR", "CLAUDECODE"].into_iter().zip(before) {
            match value {
                Some(v) => std::env::set_var(name, v),
                None => std::env::remove_var(name),
            }
        }
    }

    assert_eq!(out.stdout.trim(), "unset|unset");
}
