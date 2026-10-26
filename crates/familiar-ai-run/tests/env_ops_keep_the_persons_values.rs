//! A reader's env operations (plan A2): set, append, prepend and unset, each variable with its
//! own separator (`:` for PYTHONPATH, a space for PYTEST_ADDOPTS), so the person's own values
//! survive in Familiar's run.

use std::time::Duration;

use familiar_ai_run::{run_test_command_ops, EnvOp};

#[test]
fn appends_and_prepends_keep_the_persons_own_values() {
    // One test in this binary, so nothing else reads the environment while it is set.
    let names = ["PYTHONPATH", "PYTEST_ADDOPTS", "FAMILIAR_AI_GONE", "FAMILIAR_AI_NEW", "FAMILIAR_AI_EMPTY_ADD"];
    let before = names.map(std::env::var_os);
    // SAFETY: this is the only test in the binary, so no other thread touches the environment.
    unsafe {
        std::env::set_var("PYTHONPATH", "/the/persons/src");
        std::env::set_var("PYTEST_ADDOPTS", "-p pretty --tb=short");
        std::env::set_var("FAMILIAR_AI_GONE", "here");
        std::env::remove_var("FAMILIAR_AI_NEW");
        std::env::remove_var("FAMILIAR_AI_EMPTY_ADD");
    }

    let dir = std::env::current_dir().unwrap();
    let ops = [
        EnvOp::Prepend("PYTHONPATH", "/kit"),
        EnvOp::Append("PYTEST_ADDOPTS", "-p familiar_ai_pytest"),
        EnvOp::Unset("FAMILIAR_AI_GONE"),
        EnvOp::Set("FAMILIAR_AI_NEW", "v"),
        EnvOp::Append("FAMILIAR_AI_EMPTY_ADD", "x"),
        EnvOp::Set("CLAUDECODE", "1"),
    ];
    let out = run_test_command_ops(
        r#"echo "$PYTHONPATH|$PYTEST_ADDOPTS|${FAMILIAR_AI_GONE-unset}|$FAMILIAR_AI_NEW|$FAMILIAR_AI_EMPTY_ADD|${CLAUDECODE-unset}""#,
        &dir,
        Duration::from_secs(30),
        None,
        &ops,
    );

    // SAFETY: as above.
    unsafe {
        for (name, value) in names.into_iter().zip(before) {
            match value {
                Some(v) => std::env::set_var(name, v),
                None => std::env::remove_var(name),
            }
        }
    }

    assert_eq!(out.stdout.trim(), "/kit:/the/persons/src|-p pretty --tb=short -p familiar_ai_pytest|unset|v|x|unset");
    assert_eq!(familiar_ai_run::separator("PYTHONPATH"), ":");
    assert_eq!(familiar_ai_run::separator("PYTEST_ADDOPTS"), " ");
}
