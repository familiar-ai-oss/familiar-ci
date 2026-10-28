//! Parse the command line. The doc comments on the commands and their
//! arguments are the `--help` text. A hook's event must be one of Claude
//! Code's event names exactly, or the call is a usage error.

use clap::{Parser, Subcommand};

use crate::input::hook_input::HookEvent;

#[derive(Parser, Debug)]
#[command(
    name = "familiar-ai",
    version,
    about = "Proof that an AI coding agent's work is done."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Run as one of Claude Code's hooks; reads one JSON object from stdin.
    Hook {
        /// Claude Code's event name, exactly.
        #[arg(value_parser = HookEvent::parse_cli)]
        event: HookEvent,
    },
    /// Declare one fact of the plan.
    ///
    /// `claim <kind> --<field> <value> ...`, or the fields as one JSON object
    /// on stdin. An answer, an announcement or a transfer is claimed the same
    /// way. The kinds the binary writes itself are refused. `claim` alone
    /// lists the kinds; `claim <kind>` with no fields prints the kind's form
    /// and an example.
    Claim {
        /// One of the 19 claimable kinds, exactly (none: list them).
        kind: Option<String>,
        /// `--<field> <value>` pairs; a value that reads as JSON is JSON,
        /// anything else is text.
        #[arg(trailing_var_arg = true, allow_hyphen_values = true, num_args = 0..)]
        fields: Vec<String>,
    },
    /// Claim the plan's lines in ONE call, from a JSON array on stdin.
    ///
    /// The array of `{"kind": …, …}` objects comes on stdin as a heredoc: the
    /// plan is not complete, so the hooks refuse the file. Each line is
    /// claimed in order through `claim`'s own path, stopping at the first
    /// refusal, printed prefixed `familiar-ai: line N of M, <kind>: `; the
    /// lines before it stay, nothing after is appended. `plan` with nothing
    /// on stdin prints the array's form. The status comes back with the
    /// answer, so no `familiar-ai status` call is needed after it.
    Plan,
    /// Recheck the committed proof in a clean clone, as CI does.
    ///
    /// Each criterion red at its tests-first commit, the sealed tests
    /// unchanged, green at the head, the teeth run. Exit 0 proven, 1 not,
    /// 2 could not be established.
    Verify,
    /// Try a pytest part's planned test before its place is chosen [draft name].
    ///
    /// Runs the part's test command on the working tree, nothing committed,
    /// with the call recorder on. Red: lists the functions the failing tests
    /// ran, best match to the person's words first, to pick the place from.
    /// A try is a look, never a run that counts.
    Try {
        /// The part to try (default: the one in its turn held by this session).
        #[arg(long)]
        criterion: Option<String>,
        /// `all`: print the latest try's whole list, kept from its run; nothing runs.
        #[arg(long)]
        list: Option<String>,
    },
    /// Show the whole picture.
    ///
    /// Every criterion, its state, its holds, the cards that wait for a
    /// person, and what comes next.
    Status {
        /// Print the same picture as one JSON object.
        #[arg(long)]
        json: bool,
        /// After the picture, every prompt the person typed and what it
        /// became: an ask, an answer or a note.
        #[arg(long)]
        said: bool,
        /// Print the pull request body instead: each part by its own text,
        /// its ask, its tests and the judge, as markdown.
        #[arg(long, conflicts_with_all = ["json", "said"])]
        pr: bool,
    },
    /// Set the project up; safe to run again.
    ///
    /// Make its `.familiar` folder, keep `.familiar/record` out of git, and
    /// commit its own edits as one commit "familiar-ai setup".
    Setup,
    /// Before the first push, fold the unpushed commits into one seal and
    /// one fix per criterion.
    ///
    /// The branch's unpushed commits that the record saw become one seal
    /// commit and one fix commit per criterion, the failed attempts dropped,
    /// re-checked before the branch moves.
    Tidy,
    /// Print the one line for Claude Code's `statusLine` setting.
    ///
    /// Reads the session's JSON on stdin, writes nothing.
    Statusline,
    /// Stop a test run.
    ///
    /// Its process group is ended and confirmed gone, and nothing it ends
    /// with counts as proof. A finished run is "already finished".
    Cancel {
        /// The run, as `familiar-ai run` lists it.
        #[arg(long = "run_id")]
        run_id: String,
    },
    /// One catch-up, then list the runs.
    ///
    /// Record landed commits, recover runs nobody is running, reserve and
    /// launch the runs owed.
    Run,
    /// What hosted CI said.
    ///
    /// `ci fetch` reads the `familiar-verify` runs of a commit with `gh` and
    /// records each finished one; `status` reads the record and never asks
    /// GitHub.
    Ci {
        #[command(subcommand)]
        action: CiAction,
    },
    /// Switch familiar-ai off for this project; the person's switch.
    ///
    /// Every hook lets everything through and says nothing until
    /// `familiar-ai on`. Refused from an agent's shell.
    Off,
    /// Switch familiar-ai on again for this project; the person's switch.
    ///
    /// Refused from an agent's shell.
    On,
    /// Release every part of this project not yet released or proven; the
    /// person's, as a card's release is.
    ///
    /// Their record is kept and the next ask is planned fresh. Refused from an
    /// agent's shell.
    Clear,
    /// Say whether gap recordings may be shared; the person's choice.
    ///
    /// `share on` is the one-time yes to sending Familiar's own probe output
    /// (never your code) once an endpoint is named, `share off` the no. Kept
    /// for this machine, not a project. Refused from an agent's shell.
    Share {
        #[command(subcommand)]
        action: ShareAction,
    },
    /// Answer a card, signed: the person's approval CI can trust.
    ///
    /// Reads the passphrase of the approval key on stdin, never from the
    /// command line or the environment; signs the card's payload (SSHSIG,
    /// namespace familiar-ai-approval), writes .familiar/approvals/<t>.json
    /// and .sig, and records the answer. Refused from an agent's shell in
    /// every form its text shows; the passphrase is what an agent lacks.
    Approve {
        #[arg(long = "criterion_id")]
        criterion_id: String,
        /// The card's hold, e.g. anchor_moved.
        #[arg(long)]
        hold: String,
        #[arg(long)]
        choice: String,
        /// The run behind the card; empty for a card with none.
        #[arg(long = "run_id", default_value = "")]
        run_id: String,
        /// The person's own words, with the choice `typed`.
        #[arg(long, default_value = "")]
        typed: String,
        /// Record the answer with no signature (no key yet): CI reads it as
        /// approval not verified. Needs no key and no passphrase.
        #[arg(long)]
        unsigned: bool,
        /// The encrypted ed25519 key (~/.ssh/familiar_ai_approve when not given).
        #[arg(long)]
        key: Option<std::path::PathBuf>,
    },
    /// Start a lane: a headless `claude -p` in a worktree of its own, with
    /// the brief as its prompt and the person's words of the current ask in
    /// its record, governed by our plugin. One lane runs per worktree.
    #[command(name = "launch_lane")]
    LaunchLane {
        /// The file whose text is the lane's prompt.
        #[arg(long)]
        brief: std::path::PathBuf,
        /// The lane's folder (made beside the repository, named for it and
        /// the brief, when not given).
        #[arg(long)]
        worktree: Option<std::path::PathBuf>,
        /// Continue the lane's own conversation with the brief as its next prompt.
        #[arg(long)]
        resume: bool,
        /// The plugin the lane loads (`CLAUDE_PLUGIN_ROOT` when not given).
        #[arg(long = "plugin-dir")]
        plugin_dir: Option<std::path::PathBuf>,
        /// The lane's model, passed to `claude --model`.
        #[arg(long)]
        model: Option<String>,
        /// The lane's budget, passed to `claude --max-budget-usd`.
        #[arg(long = "max-budget-usd")]
        max_budget_usd: Option<String>,
    },
    /// Internal: the push after a commit (plan §1.2): waits for the runs the
    /// commit reserved and tells each once; reads the hook's JSON on stdin.
    #[command(hide = true, name = "await-run")]
    AwaitRun,
    /// Internal: the detached child that performs one reserved run.
    #[command(hide = true)]
    RunChild {
        #[arg(long = "run_id")]
        run_id: String,
    },
    /// Print the exact prompt the judge is asked for a criterion's seal.
    ///
    /// For the project here. Reads the record; asks no model and writes
    /// nothing.
    JudgePrompt {
        /// The sealed criterion, e.g. c1.
        criterion_id: String,
    },
}

#[derive(Subcommand, Debug)]
pub enum CiAction {
    /// Ask GitHub (`gh`) for the `familiar-verify` runs of a commit and
    /// record each finished one.
    ///
    /// Appends one `ci_report_seen` per finished run not yet recorded, and
    /// prints the CI line for the commit.
    Fetch {
        /// The commit (the project's git HEAD when not given).
        #[arg(long)]
        sha: Option<String>,
    },
}

#[derive(Subcommand, Debug)]
pub enum ShareAction {
    /// Yes: gap recordings are sent, once an endpoint is named.
    On,
    /// No: they stay in each project's `.familiar/runs/gaps`.
    Off,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<Cli, clap::Error> {
        Cli::try_parse_from(std::iter::once("familiar-ai").chain(args.iter().copied()))
    }

    #[test]
    fn hook_parses_each_of_the_six_events() {
        for name in crate::input::hook_input::EVENT_NAMES {
            let cli = parse(&["hook", name]).unwrap_or_else(|e| panic!("{name}: {e}"));
            match cli.command {
                Command::Hook { event } => assert_eq!(event.name(), name),
                _ => panic!("expected hook command"),
            }
        }
    }

    #[test]
    fn hook_rejects_an_unknown_event_name() {
        assert!(parse(&["hook", "BeforeToolUse"]).is_err());
        assert!(parse(&["hook", "pretooluse"]).is_err());
    }

    #[test]
    fn verify_status_setup_parse() {
        for name in ["verify", "status", "setup", "statusline", "tidy", "off", "on"] {
            assert!(parse(&[name]).is_ok(), "{name}");
        }
    }

    #[test]
    fn claim_takes_a_kind_and_keeps_the_field_pairs_as_given() {
        let cli = parse(&["claim", "criterion", "--criterion_id", "c1", "--text", "greet says hello", "--path", "light"])
            .expect("claim parses");
        match cli.command {
            Command::Claim { kind, fields } => {
                assert_eq!(kind.as_deref(), Some("criterion"));
                assert_eq!(fields, ["--criterion_id", "c1", "--text", "greet says hello", "--path", "light"]);
            }
            _ => panic!("expected claim"),
        }
        let cli = parse(&["claim", "new_work_no_spot"]).expect("no fields is fine (stdin, or the form)");
        assert!(matches!(cli.command, Command::Claim { fields, .. } if fields.is_empty()));
        let cli = parse(&["claim"]).expect("no kind lists the kinds");
        assert!(matches!(cli.command, Command::Claim { kind: None, fields } if fields.is_empty()));
    }

    #[test]
    fn plan_takes_no_arguments() {
        let cli = parse(&["plan"]).expect("plan parses");
        assert!(matches!(cli.command, Command::Plan));
    }

    #[test]
    fn unknown_subcommand_is_a_usage_error() {
        assert!(parse(&["fiddle"]).is_err());
    }

    #[test]
    fn version_flag_is_the_crates_version() {
        let err = parse(&["--version"]).expect_err("--version short-circuits");
        assert_eq!(err.kind(), clap::error::ErrorKind::DisplayVersion);
        assert!(err.to_string().contains(env!("CARGO_PKG_VERSION")));
    }
}
