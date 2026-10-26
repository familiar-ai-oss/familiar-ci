//! What comes in from outside the process: the command line, the hook's JSON
//! and the surroundings, passed in so nothing below reads a clock or the environment.

pub mod cli;
pub mod hook_input;
pub mod transcript;

use std::path::PathBuf;
use std::time::Duration;

/// What the program is told from outside.
#[derive(Debug, Clone)]
pub struct Surroundings {
    /// The time now in seconds, the `at` of any fact written.
    pub now: f64,
    /// The project's root as the agent was started in it; the hook input's
    /// `cwd` is used when it is not known.
    pub project_dir: Option<PathBuf>,
    /// Where `claim` and `status` look for the project when `project_dir` is
    /// not known.
    pub cwd: Option<PathBuf>,
    /// This conversation's id from `CLAUDE_CODE_SESSION_ID`, which
    /// `criterion_assigned` carries when the claim leaves it out.
    pub session_id: Option<String>,
    /// Where this laptop keeps what is not the repository's, such as the setup
    /// marker; `None` when no home is known, and then no marker is written or read.
    pub data_dir: Option<PathBuf>,
    /// How long a hook waits its turn for the record.
    pub record_wait: Duration,
    /// How long the announced test command may run before it counts as failed.
    pub test_run_wait: Duration,
    /// How long a call that launched a test run waits for it with the record
    /// let go; a slower run is left running.
    pub run_inline: Duration,
    /// This binary, which runs a reserved run detached as `run-child`; `None`
    /// runs it in the calling process.
    pub runner: Option<PathBuf>,
    /// The folder the judge runs in, outside the project so its hooks stay
    /// quiet. `None` means no judge is asked and the gate on code does not apply.
    pub judge_dir: Option<PathBuf>,
    /// How long the judge may take before it counts as no answer.
    pub judge_wait: Duration,
    /// What the binary needs to run the investigator itself; `None` leaves
    /// only the spawn rewrite.
    pub investigator: Option<Investigating>,
    /// The conversation this session is when the binary launched it as an
    /// investigator, from `FAMILIAR_AI_CONVERSATION_ID`, which the agent cannot change.
    pub conversation: Option<String>,
    /// A bench sandbox or a headless session (`claude -p`), with no person
    /// here to answer a card, so the call to action and the stop held on an
    /// open card are left out; the card waits in the record for the person's
    /// session.
    pub bench: bool,
}

/// What `launch_lane` starts a lane with, read from the environment by `main`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Launching {
    /// `claude` as found on PATH.
    pub claude: Option<PathBuf>,
    /// `CLAUDE_PLUGIN_ROOT`: the plugin in use.
    pub plugin_root: Option<PathBuf>,
    /// The PATH the lane is given after the governing binary's folder.
    pub path: Option<std::ffi::OsString>,
    /// This binary, the one that governs the lane.
    pub exe: Option<PathBuf>,
}

/// What the binary-run investigator is launched with.
#[derive(Debug, Clone, PartialEq)]
pub struct Investigating {
    /// `claude` as found on the hook's PATH; without it no investigator is due,
    /// so no transfer is written to a child that cannot start.
    pub program: PathBuf,
    /// The plugin's folder, which the child loads so the same hooks govern it.
    pub plugin_dir: PathBuf,
    /// The child's `--max-budget-usd`.
    pub budget: String,
    pub wait: Duration,
    /// Where Claude Code keeps the child's sessions; its transcript is copied
    /// from `projects/` there.
    pub claude_dir: Option<PathBuf>,
}

impl Surroundings {
    /// How long a hook waits for the record, well inside a hook's time limit.
    pub const RECORD_WAIT: Duration = Duration::from_secs(10);
    /// How long a test run may take by default; a run is detached from the
    /// hook, so no hook's limit applies.
    pub const TEST_RUN_WAIT: Duration = Duration::from_secs(1_800);
    /// How long a call that launched a run waits for it before returning.
    pub const RUN_INLINE: Duration = Duration::from_secs(5);
    /// How long one test run may take inside `verify`, where the CI job's own
    /// limit applies.
    pub const VERIFY_RUN_WAIT: Duration = Duration::from_secs(1_800);
    /// The judge's time limit: answered calls were measured at most 43 s and
    /// failing ones at 50 to 70 s, so 60 s keeps every answer and stops a
    /// failing call before it costs more.
    pub const JUDGE_WAIT: Duration = Duration::from_secs(60);
    /// How long the binary waits for an investigator. The same hook call may
    /// also ask the judge, run tests and wait for the record, and the total must
    /// stay under the plugin's 1800 s hook limit.
    pub const INVESTIGATOR_WAIT: Duration = Duration::from_secs(1_600);
    /// The investigator's `--max-budget-usd` when none is given.
    pub const INVESTIGATOR_BUDGET: &'static str = "3.00";

    /// The binary-run investigator, or `None` when it is switched off, no
    /// plugin folder is known or no `claude` was found. A budget that is not a
    /// positive number of dollars is the default.
    pub fn investigator_from(switch: Option<&str>, program: Option<PathBuf>, plugin_dir: Option<&str>, budget: Option<&str>) -> Option<Investigating> {
        if switch == Some("off") {
            return None;
        }
        let program = program?;
        let plugin_dir = PathBuf::from(plugin_dir.filter(|d| !d.is_empty())?);
        let budget = budget
            .map(str::trim)
            .filter(|b| b.parse::<f64>().is_ok_and(|x| x.is_finite() && x > 0.0))
            .unwrap_or(Self::INVESTIGATOR_BUDGET)
            .to_string();
        Some(Investigating { program, plugin_dir, budget, wait: Self::INVESTIGATOR_WAIT, claude_dir: None })
    }

    /// Where Claude Code keeps its sessions: `CLAUDE_CONFIG_DIR`, else
    /// `$HOME/.claude`.
    pub fn claude_dir_from(config_dir: Option<&str>, home: Option<&str>) -> Option<PathBuf> {
        match (config_dir.filter(|d| !d.is_empty()), home.filter(|h| !h.is_empty())) {
            (Some(d), _) => Some(PathBuf::from(d)),
            (None, Some(h)) => Some(PathBuf::from(h).join(".claude")),
            (None, None) => None,
        }
    }

    /// The first file called `name` in the folders of a PATH value.
    pub fn found_on(path: Option<&str>, name: &str) -> Option<PathBuf> {
        std::env::split_paths(path?).map(|d| d.join(name)).find(|p| p.is_file())
    }

    /// Where the judge runs, or `None` when `FAMILIAR_AI_JUDGE` is `off`.
    pub fn judge_dir_from(judge: Option<&str>, data_dir: Option<&std::path::Path>, temp_dir: PathBuf) -> Option<PathBuf> {
        if judge == Some("off") {
            return None;
        }
        Some(data_dir.map(|d| d.join("judge")).unwrap_or_else(|| temp_dir.join("familiar-ai-judge")))
    }

    /// The data folder from `FAMILIAR_AI_HOME`, else `XDG_DATA_HOME`, else
    /// `HOME`; an empty value is no value.
    pub fn data_dir_from(
        familiar_ai_home: Option<&str>,
        xdg_data_home: Option<&str>,
        home: Option<&str>,
    ) -> Option<PathBuf> {
        let set = |v: Option<&str>| v.filter(|s| !s.is_empty()).map(PathBuf::from);
        set(familiar_ai_home)
            .or_else(|| set(xdg_data_home).map(|d| d.join("familiar-ai")))
            .or_else(|| set(home).map(|h| h.join(".local").join("share").join("familiar-ai")))
    }
}

/// Whether a command line reads stdin. Off a terminal only, a person typing
/// `claim` or `plan` is shown the form rather than left waiting.
pub fn wants_stdin(args: &[String], at_terminal: bool) -> bool {
    // Help and the version are printed at once; nothing is waited for.
    if args.iter().any(|a| matches!(a.as_str(), "--help" | "-h" | "--version" | "-V")) {
        return false;
    }
    match args.first().map(String::as_str) {
        Some("hook" | "await-run") => true,
        // Claude Code pipes the session's JSON; a person at a terminal is not left waiting.
        Some("statusline") => !at_terminal,
        Some("claim") => args.len() >= 2 && !at_terminal && !args.iter().skip(2).any(|a| a.starts_with("--")),
        // The plan's lines are a JSON array on stdin; a person at a terminal is shown the form.
        Some("plan") => !at_terminal,
        // Off a terminal the approval key's passphrase is stdin's first line; at a
        // terminal approve asks for it with echo off.
        Some("approve") => !at_terminal,
        _ => false,
    }
}

/// How long a `claim` or a `plan` waits for stdin before taking it as empty.
/// Only an open pipe nobody writes, as the Bash tool's stdin can be, pays the
/// whole wait instead of hanging forever.
pub const STDIN_WAIT: std::time::Duration = std::time::Duration::from_millis(500);

/// Whether stdin has data or its end within `wait`. Asked only for a `claim`
/// or a `plan`, since Claude Code always writes and closes the hook's stdin.
pub fn stdin_has_something(wait: std::time::Duration) -> bool {
    let mut fd = libc::pollfd { fd: 0, events: libc::POLLIN, revents: 0 };
    let ms = libc::c_int::try_from(wait.as_millis()).unwrap_or(libc::c_int::MAX);
    // SAFETY: one pollfd, owned here and alive for the call.
    let ready = unsafe { libc::poll(&mut fd, 1, ms) };
    ready > 0 && fd.revents & (libc::POLLIN | libc::POLLHUP) != 0
}

#[cfg(test)]
mod tests {
    use super::{wants_stdin, Surroundings};

    #[test]
    fn the_investigator_is_off_without_a_plugin_folder_and_the_budget_is_a_number() {
        let claude = || Some(std::path::PathBuf::from("/bin/claude"));
        assert_eq!(Surroundings::investigator_from(Some("off"), claude(), Some("/p"), None), None);
        assert_eq!(Surroundings::investigator_from(None, claude(), None, None), None);
        assert_eq!(Surroundings::investigator_from(None, claude(), Some(""), None), None);
        assert_eq!(Surroundings::investigator_from(None, None, Some("/p"), None), None, "no claude found");
        let on = Surroundings::investigator_from(None, claude(), Some("/p"), Some("0.50")).unwrap_or_else(|| panic!("on"));
        assert_eq!((on.plugin_dir.to_string_lossy().as_ref(), on.budget.as_str()), ("/p", "0.50"));
        for bad in ["", "x", "-1", "0", "inf", "NaN"] {
            let b = Surroundings::investigator_from(Some("on"), claude(), Some("/p"), Some(bad)).map(|i| i.budget);
            assert_eq!(b.as_deref(), Some("3.00"), "{bad}");
        }
        assert_eq!(Surroundings::claude_dir_from(Some("/c"), Some("/h")), Some(std::path::PathBuf::from("/c")));
        assert_eq!(Surroundings::claude_dir_from(Some(""), Some("/h")), Some(std::path::PathBuf::from("/h/.claude")));
        assert_eq!(Surroundings::claude_dir_from(None, None), None);
        assert_eq!(Surroundings::found_on(Some("/no-such-dir:/bin"), "sh"), Some(std::path::PathBuf::from("/bin/sh")));
        assert_eq!(Surroundings::found_on(Some("/no-such-dir"), "sh"), None);
        assert_eq!(Surroundings::found_on(None, "sh"), None);
    }

    #[test]
    fn stdin_is_read_for_a_plan_only_off_a_terminal() {
        let a = |xs: &[&str]| xs.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert!(wants_stdin(&a(&["plan"]), false), "the lines come as a JSON array on stdin");
        assert!(!wants_stdin(&a(&["plan"]), true), "a person at a terminal is shown the form");
    }

    #[test]
    fn stdin_is_read_for_the_hook_and_a_fieldless_claim_only() {
        let a = |xs: &[&str]| xs.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert!(wants_stdin(&a(&["hook", "Stop"]), false));
        assert!(wants_stdin(&a(&["hook", "Stop"]), true));
        assert!(wants_stdin(&a(&["claim", "new_work_no_spot"]), false));
        assert!(!wants_stdin(&a(&["claim", "new_work_no_spot"]), true), "a person at a terminal is shown the form");
        assert!(!wants_stdin(&a(&["claim"]), false));
        assert!(!wants_stdin(&a(&["claim", "criterion", "--criterion_id", "c1"]), false));
        assert!(!wants_stdin(&a(&["status"]), false));
        assert!(wants_stdin(&a(&["statusline"]), false));
        assert!(!wants_stdin(&a(&["statusline"]), true));
        assert!(!wants_stdin(&a(&["--version"]), false));
        assert!(!wants_stdin(&a(&[]), false));
    }

    #[test]
    fn a_command_line_asking_for_help_or_the_version_reads_no_stdin() {
        let a = |xs: &[&str]| xs.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        for line in [
            &["plan", "--help"][..],
            &["plan", "-h"],
            &["claim", "criterion", "--help"],
            &["hook", "--help"],
            &["statusline", "-V"],
            &["plan", "--version"],
        ] {
            assert!(!wants_stdin(&a(line), false), "{line:?}: help is printed at once, nothing is waited for");
        }
    }
}
