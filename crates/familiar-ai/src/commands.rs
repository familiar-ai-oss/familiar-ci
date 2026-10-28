//! The command line: parse argv and hand each command to its module. `hook` goes to `decide`;
//! every other command goes to `door`.

use clap::Parser as _;

use crate::door::cannot;
use crate::input::cli::{Cli, Command};
use crate::input::{Launching, Surroundings};
use crate::teach;

/// What a command did: the exit code and the text `main` prints.
pub struct Outcome {
    pub exit_code: i32,
    pub stdout_text: String,
    pub stderr_text: String,
}

/// The stderr text, with exit 2, of a hook call that panics or cannot print its answer.
pub const STOPPED_WHILE_DECIDING: &str = "familiar-ai: the program stopped while deciding, so nothing goes ahead\n";

/// Run one command line (argv without the program name), with stdin passed as text so tests can drive it.
/// Bad input never panics: bad hook input is a refusal and an unknown command is a usage error, both exit 2.
/// No lane can be launched from here: nothing is known to launch it with.
pub fn run<I, S>(args: I, input: &str, around: &Surroundings) -> Outcome
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    run_with(args, input, around, &Launching::default())
}

/// [`run`], with what `launch_lane` starts a lane with.
pub fn run_with<I, S>(args: I, input: &str, around: &Surroundings, launching: &Launching) -> Outcome
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let argv: Vec<String> = std::iter::once("familiar-ai".to_string())
        .chain(args.into_iter().map(Into::into))
        .collect();
    match Cli::try_parse_from(argv) {
        Err(e) => usage_error_outcome(&e),
        Ok(cli) => match cli.command {
            Command::Hook { event } => crate::decide::run_hook(event, input, around),
            Command::Claim { kind, fields } => crate::door::claim_cmd::run_claim(kind.as_deref(), &fields, input, around),
            Command::Plan => crate::door::claim_cmd::run_plan(input, around),
            Command::Status { json, said, pr } => run_status(around, json, said, pr),
            Command::Verify => crate::door::verify::run_verify(around),
            Command::Try { criterion, list } => crate::door::try_cmd::run_try(criterion.as_deref(), list.as_deref(), around),
            Command::Setup => crate::door::setup::run_setup(around),
            Command::Tidy => crate::door::tidy::run_tidy(around),
            Command::Cancel { run_id } => crate::door::runner::cancel(&run_id, around),
            Command::Run => crate::door::runner::run_command(around),
            Command::AwaitRun => crate::door::told::run_await(input, around),
            Command::RunChild { run_id } => crate::door::runner::run_child(&run_id, around),
            Command::Statusline => crate::door::statusline::run_statusline(input, around),
            Command::Ci { action: crate::input::cli::CiAction::Fetch { sha } } => crate::door::ci::run_fetch(sha.as_deref(), around),
            Command::JudgePrompt { criterion_id } => crate::door::judge::prompt_command(&criterion_id, around),
            Command::Off => crate::door::switch::run_switch("off", around),
            Command::On => crate::door::switch::run_switch("on", around),
            Command::Clear => crate::door::switch::run_clear(around),
            Command::Share { action } => crate::door::share::run_share(matches!(action, crate::input::cli::ShareAction::On), around),
            Command::Approve { criterion_id, hold, choice, run_id, typed, unsigned, key } => {
                let card = crate::door::approval::Card { criterion_id, hold, choice, run_id, typed };
                if unsigned {
                    crate::door::approval::run_unsigned(&card, around)
                } else {
                    // at the person's own terminal the passphrase is asked for with echo off
                    use crate::door::approval::{ask_passphrase, Asked};
                    use std::io::IsTerminal;
                    match if std::io::stdin().is_terminal() { ask_passphrase() } else { Asked::NoTerminal } {
                        Asked::Typed(p) => crate::door::approval::run_approve(&card, key.as_deref(), &p, around),
                        Asked::Cancelled => crate::door::approval::cancelled(),
                        Asked::NoTerminal => crate::door::approval::run_approve(&card, key.as_deref(), input, around),
                    }
                }
            }
            Command::LaunchLane { brief, worktree, resume, plugin_dir, model, max_budget_usd } => {
                let asked = crate::door::lane::Asked { brief, worktree, resume, plugin_dir, model, budget: max_budget_usd };
                crate::door::lane::run_launch(&asked, around, launching)
            }
        },
    }
}

/// Print clap's rendered text: `--help` and `--version` arrive as errors with exit 0 and go to stdout,
/// and a parse failure goes to stderr with exit 2.
fn usage_error_outcome(e: &clap::Error) -> Outcome {
    let exit_code = e.exit_code();
    if exit_code == 0 {
        Outcome {
            exit_code,
            stdout_text: e.to_string(),
            stderr_text: String::new(),
        }
    } else {
        Outcome {
            exit_code,
            stdout_text: String::new(),
            stderr_text: e.to_string(),
        }
    }
}

/// The `status` command prints the picture from a read of the record; nothing is caught up or written.
fn run_status(around: &Surroundings, json: bool, said: bool, pr: bool) -> Outcome {
    let (root, record) = match crate::door::project::open_project(around) {
        Ok(x) => x,
        Err(out) => return out,
    };
    let read = record.read_all();
    // Main's record is read with this one let go.
    drop(record);
    let log = match read {
        Ok(log) => log,
        Err(e) => return crate::door::project::plain(1, String::new(), format!("familiar-ai: {}\n", cannot("read its record", &e))),
    };
    if pr {
        return crate::door::project::plain(0, teach::pr_body(&log), String::new());
    }
    let switch = match crate::door::switch::here(&root, &log, around.record_wait) {
        Ok(s) => s,
        Err(familiar_ai_facts::RecordError::Busy) => return crate::door::project::plain(1, String::new(), format!("familiar-ai: {}\n", crate::decide::hook_output::BUSY_MESSAGE)),
        Err(e) => return crate::door::project::plain(1, String::new(), format!("familiar-ai: {}\n", cannot("read main's record", &e))),
    };
    if json {
        let others = crate::door::project::other_checkouts(&root, around.record_wait);
        let mut v = teach::picture_json(&log, &root, around.now, around.judge_dir.is_some(), &others);
        switch.json_into(&mut v);
        return crate::door::project::plain(0, format!("{v}\n"), String::new());
    }
    let first = switch.status_line().map(|l| format!("{l}\n")).unwrap_or_default();
    let mut out = format!("{first}{}\n", teach::picture(&log, &root, around.now, around.judge_dir.is_some(), teach::ci::line_here(&log, &root).as_deref()));
    // each part's latest try and what it listed [draft]
    out.extend(teach::try_lines(&log).into_iter().map(|l| format!("{l}\n")));
    if said {
        out.extend(teach::said_lines(&log).into_iter().map(|l| format!("{l}\n")));
    }
    crate::door::project::plain(0, out, String::new())
}

#[cfg(test)]
mod tests {
    use super::run;
    use crate::input::hook_input::EVENT_NAMES;
    use crate::input::Surroundings;
    use crate::door::FAMILIAR_FOLDER;
    use std::time::Duration;

    fn run_args(args: &[&str], input: &str) -> super::Outcome {
        let around = Surroundings {
            now: 0.0,
            project_dir: None,
            cwd: Some(std::env::temp_dir()),
            session_id: None,
            data_dir: None,
            record_wait: Duration::ZERO,
            test_run_wait: Duration::from_secs(1),
            run_inline: Duration::ZERO,
            runner: None,
            judge_dir: None,
            judge_wait: Duration::ZERO,
            investigator: None,
            conversation: None,
            bench: false,
        };
        run(args.iter().map(|s| s.to_string()), input, &around)
    }

    #[test]
    fn hook_with_no_project_folder_allows_and_only_the_session_start_says_so() {
        for name in EVENT_NAMES {
            let input = format!("{{\"hook_event_name\": \"{name}\", \"source\": \"startup\"}}");
            let out = run_args(&["hook", name], &input);
            assert_eq!(out.exit_code, 0, "{name}");
            assert!(out.stderr_text.is_empty(), "{name}");
            if name == "SessionStart" {
                let v: serde_json::Value = serde_json::from_str(&out.stdout_text).expect("json");
                assert_eq!(v["hookSpecificOutput"]["additionalContext"], crate::door::setup::NOT_SET_UP, "{name}");
                assert!(v.get("decision").is_none() && !out.stdout_text.contains("permissionDecision"));
            } else {
                assert_eq!(out.stdout_text, "{}\n", "{name}");
            }
        }
    }

    #[test]
    fn hook_with_invalid_json_exits_2_not_1_not_panic() {
        let out = run_args(&["hook", "Stop"], "this is not json");
        assert_eq!(out.exit_code, 2);
        assert!(!out.stderr_text.is_empty());
    }

    #[test]
    fn hook_with_event_mismatch_exits_2() {
        let input = r#"{"hook_event_name": "Stop"}"#;
        let out = run_args(&["hook", "PreToolUse"], input);
        assert_eq!(out.exit_code, 2);
        assert!(out.stderr_text.contains("mismatch"));
    }

    #[test]
    fn unknown_event_is_a_usage_error_exit_2() {
        let out = run_args(&["hook", "NotAnEvent"], "{}");
        assert!(!out.stderr_text.is_empty());
    }

    #[test]
    fn unknown_subcommand_is_a_usage_error_exit_2() {
        let out = run_args(&["frobnicate"], "");
        assert_eq!(out.exit_code, 2);
        assert!(!out.stderr_text.is_empty());
    }

    #[test]
    fn verify_outside_a_repository_could_not_prove_anything_exit_2_one_line() {
        let out = run_args(&["verify"], "");
        assert_eq!(out.exit_code, 2);
        assert!(out.stdout_text.starts_with("familiar-ai: verify could not prove anything: "), "{}", out.stdout_text);
        assert_eq!(out.stdout_text.trim_end().lines().count(), 1);
    }

    #[test]
    fn setup_outside_a_repository_is_refused_with_one_line_and_makes_nothing() {
        // The temp dir is in no repository.
        let out = run_args(&["setup"], "");
        assert_eq!(out.exit_code, 1, "{}", out.stderr_text);
        assert_eq!(out.stdout_text, "");
        assert!(out.stderr_text.contains("not inside a git repository"), "{}", out.stderr_text);
        assert_eq!(out.stderr_text.trim_end().lines().count(), 1);
        assert!(!std::env::temp_dir().join(FAMILIAR_FOLDER).exists());
    }

    #[test]
    fn status_and_claim_outside_a_project_exit_1_naming_the_folder() {
        for args in [vec!["status"], vec!["claim", "new_work_no_spot", "--criterion_id", "c1"]] {
            let out = run_args(&args, "");
            assert_eq!(out.exit_code, 1, "{args:?}");
            assert_eq!(out.stdout_text, "");
            assert!(out.stderr_text.contains(".familiar"), "{args:?}: {}", out.stderr_text);
        }
    }

    #[test]
    fn version_flag_prints_the_version_on_stdout() {
        let out = run_args(&["--version"], "");
        assert_eq!(out.exit_code, 0);
        assert!(out.stdout_text.contains(env!("CARGO_PKG_VERSION")));
    }
}
