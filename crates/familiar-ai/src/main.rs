//! The thin binary: read argv, stdin and the environment, run the library, print, exit.
//! Claude Code lets an action through on any exit code but 0 or 2, so a panic under `hook` exits 2.

use std::io::{IsTerminal, Read, Write};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let is_hook = args.first().map(String::as_str) == Some("hook");
    // stdin may carry the approval key's passphrase: wiped when dropped
    let mut stdin_text = zeroize::Zeroizing::new(String::new());
    // A claim or a plan takes an open pipe nobody writes as empty, so the form is shown.
    let mut nothing_came = false;
    if familiar_ai::wants_stdin(&args, std::io::stdin().is_terminal()) {
        let waits = matches!(args.first().map(String::as_str), Some("claim" | "plan"));
        if waits && !familiar_ai::stdin_has_something(familiar_ai::STDIN_WAIT) {
            nothing_came = true;
        } else {
            let _ = std::io::stdin().read_to_string(&mut stdin_text);
        }
    }

    // The only place a clock or the environment is read.
    let data_dir = familiar_ai::Surroundings::data_dir_from(
        std::env::var("FAMILIAR_AI_HOME").ok().as_deref(),
        std::env::var("XDG_DATA_HOME").ok().as_deref(),
        std::env::var("HOME").ok().as_deref(),
    );
    let conversation = std::env::var("FAMILIAR_AI_CONVERSATION_ID").ok().filter(|v| !v.is_empty());
    // The judge and the investigator ask the `claude` found on the hook's PATH.
    let claude = familiar_ai::Surroundings::found_on(std::env::var("PATH").ok().as_deref(), "claude");
    let launching = familiar_ai::Launching {
        claude: claude.clone(),
        plugin_root: std::env::var_os("CLAUDE_PLUGIN_ROOT").filter(|v| !v.is_empty()).map(std::path::PathBuf::from),
        path: std::env::var_os("PATH"),
        exe: std::env::current_exe().ok(),
    };
    let around = familiar_ai::Surroundings {
        now: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs_f64())
            .unwrap_or(0.0),
        project_dir: std::env::var_os("CLAUDE_PROJECT_DIR")
            .filter(|v| !v.is_empty())
            .map(std::path::PathBuf::from),
        cwd: std::env::current_dir().ok(),
        // An investigator's session, its hooks and its shell's claims alike, uses the id the binary gave it.
        session_id: conversation
            .clone()
            .or_else(|| std::env::var("CLAUDE_CODE_SESSION_ID").ok().filter(|v| !v.is_empty())),
        conversation,
        // A headless session (`claude -p`) has no person to ask either.
        bench: std::env::var("FAMILIAR_AI_BENCH").ok().as_deref() == Some("1") || std::env::var("CLAUDE_CODE_ENTRYPOINT").ok().as_deref() == Some("sdk-cli"),
        // With no `claude` to ask there is no judge, the same as when it is off.
        judge_dir: claude.as_ref().and_then(|_| {
            familiar_ai::Surroundings::judge_dir_from(std::env::var("FAMILIAR_AI_JUDGE").ok().as_deref(), data_dir.as_deref(), std::env::temp_dir())
        }),
        data_dir,
        record_wait: familiar_ai::Surroundings::RECORD_WAIT,
        // A run is detached from the hook, so only the stack's wait applies; the variable is in seconds.
        test_run_wait: std::env::var("FAMILIAR_AI_TEST_RUN_WAIT")
            .ok()
            .and_then(|v| v.trim().parse::<u64>().ok())
            .filter(|s| *s > 0)
            .map(std::time::Duration::from_secs)
            .unwrap_or(if args.first().map(String::as_str) == Some("verify") {
                familiar_ai::Surroundings::VERIFY_RUN_WAIT
            } else {
                familiar_ai::Surroundings::TEST_RUN_WAIT
            }),
        run_inline: std::env::var("FAMILIAR_AI_RUN_INLINE_MS")
            .ok()
            .and_then(|v| v.trim().parse::<u64>().ok())
            .map(std::time::Duration::from_millis)
            .unwrap_or(familiar_ai::Surroundings::RUN_INLINE),
        runner: std::env::current_exe().ok(),
        judge_wait: familiar_ai::Surroundings::JUDGE_WAIT,
        investigator: familiar_ai::Surroundings::investigator_from(
            std::env::var("FAMILIAR_AI_INVESTIGATOR").ok().as_deref(),
            claude,
            std::env::var("CLAUDE_PLUGIN_ROOT").ok().as_deref(),
            std::env::var("FAMILIAR_AI_INVESTIGATOR_BUDGET").ok().as_deref(),
        )
        .map(|i| familiar_ai::input::Investigating {
            claude_dir: familiar_ai::Surroundings::claude_dir_from(
                std::env::var("CLAUDE_CONFIG_DIR").ok().as_deref(),
                std::env::var("HOME").ok().as_deref(),
            ),
            ..i
        }),
    };

    let outcome = match std::panic::catch_unwind(|| familiar_ai::run_with(args, &stdin_text, &around, &launching)) {
        Ok(outcome) => outcome,
        Err(_) => familiar_ai::Outcome {
            exit_code: if is_hook { 2 } else { 1 },
            stdout_text: String::new(),
            stderr_text: familiar_ai::STOPPED_WHILE_DECIDING.to_string(),
        },
    };
    let _ = std::io::stdout().write_all(outcome.stdout_text.as_bytes());
    if nothing_came {
        let _ = writeln!(
            std::io::stderr(),
            "familiar-ai: nothing came on stdin in {} ms; pass the fields on the command line or pipe the JSON",
            familiar_ai::STDIN_WAIT.as_millis()
        );
    }
    let _ = std::io::stderr().write_all(outcome.stderr_text.as_bytes());
    std::process::exit(outcome.exit_code);
}
