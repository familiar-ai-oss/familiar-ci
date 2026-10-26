//! [draft] The shapes of what a runner's data module holds, as plain data (no behaviour): the
//! readers, the kit files, the probe kit with its truths. Lane A-py's reader table and dry run
//! read the constants of the runner modules beside this one; nothing here runs a command.
//!
//! Placeholders in a reader's env values, added flags and report globs, which the caller fills
//! when it runs: `{KIT}` the folder the kit files were written to (once per version, outside
//! the work tree), `{REPORT_DIR}` a temp folder of this run's own outside the work tree, and
//! `{REPORT}` the file a reporter writes (the env `FAMILIAR_AI_REPORT`, `{REPORT_DIR}/results.jsonl`).

/// What a reader is, best first: Familiar's own reporter injected into its own run, the runner's
/// report files, a console format pinned so the tables read it, or the tables alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Reporter,
    Report,
    Pinned,
    Tables,
}

/// How an env operation changes a variable. `Append`: the old value, then the separator, then the
/// value (just the value when there is no old one, and with an empty separator the value is
/// glued on as written). `Prepend`: the value, the separator, the old value. `Unset`: removed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    Set,
    Append,
    Prepend,
    Unset,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EnvOp {
    pub var: &'static str,
    pub op: Op,
    pub value: &'static str,
    pub separator: &'static str,
}

/// One way to read a runner's run. Only the `id` is ever written into `.familiar/adapter`.
#[derive(Debug, Clone, Copy)]
pub struct Reader {
    /// `<runner>:<name>`, unique.
    pub id: &'static str,
    pub runner: &'static str,
    pub kind: Kind,
    /// Applied to Familiar's own run only, in order; the person's terminal is untouched.
    pub env: &'static [EnvOp],
    /// Added to Familiar's own run, as argv words (the caller quotes), in the segment naming the
    /// runner. Never stored: `run_reserved.command` stays the claimed command.
    pub flags: &'static [&'static str],
    /// The reader applies only when the command names one of these.
    pub only_when_command_has: &'static [&'static str],
    /// The reader is skipped when the command names one of these.
    pub skip_when_command_has: &'static [&'static str],
    /// The files a `Report` reader reads, relative to the project root (a `{...}` one is the run's).
    pub report_globs: &'static [&'static str],
    /// How the files become the per-test result file: `schema` (the kit wrote it), `junit`, `trx`.
    pub converter: &'static str,
    /// Probes this reader cannot read by what it is: a compile error writes no report, so a
    /// report reader asks on those and the tables read them. Asking there is not a wrong reading.
    pub cannot_read: &'static [&'static str],
}

impl Reader {
    /// A reader that changes nothing and reads nothing: the base the runner modules build on
    /// (`Reader { id, runner, kind, .. Reader::NONE }`), and what the tables are.
    pub const NONE: Reader = Reader {
        id: "",
        runner: "",
        kind: Kind::Tables,
        env: &[],
        flags: &[],
        only_when_command_has: &[],
        skip_when_command_has: &[],
        report_globs: &[],
        converter: "",
        cannot_read: &[],
    };
}

/// A file of the kit, written once per version to `<temp>/familiar-ai-kit-<version>/<path>`.
/// A `template` file holds `{KIT}`, replaced by that folder when it is written.
#[derive(Debug, Clone, Copy)]
pub struct KitFile {
    pub path: &'static str,
    pub text: &'static str,
    pub template: bool,
}

/// What a probe truly is: the label its run must read as and, for a failed test, its kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Truth {
    pub label: &'static str,
    pub kind: Option<&'static str>,
}

pub const PASSED: Truth = Truth { label: "passed", kind: None };
pub const ASSERTION: Truth = Truth { label: "tests_failed", kind: Some("assertion") };
pub const MISSING: Truth = Truth { label: "missing_target_symbol", kind: None };
pub const COLLECTION: Truth = Truth { label: "collection_error", kind: None };
pub const NOTHING: Truth = Truth { label: "no_tests_collected", kind: None };
pub const EXCEPTION: Truth = Truth { label: "tests_failed", kind: Some("exception") };

/// The probes' truths (the spike's `kit/SCHEMA.txt`): stage 1 is P1 pass, P2 assertion,
/// P3 missing function, P4 broken import, P5 a pass beside a skip, P6 nothing selected; stage 2 is
/// P3b (the function now exists) pass and P7 an exception in the code.
pub const TRUTHS: [(&str, Truth); 8] = [("P1", PASSED), ("P2", ASSERTION), ("P3", MISSING), ("P4", COLLECTION), ("P5", PASSED), ("P6", NOTHING), ("P3b", PASSED), ("P7", EXCEPTION)];

pub const PLACEHOLDERS: [&str; 3] = ["{KIT}", "{REPORT_DIR}", "{REPORT}"];

/// A file of a probe, at the path the spike calibrated it (relative to the module's root; the
/// dry run places it at the project's own test location).
#[derive(Debug, Clone, Copy)]
pub struct ProbeFile {
    pub path: &'static str,
    pub text: &'static str,
}

/// One probe: its files, its truth, its stage.
#[derive(Debug, Clone, Copy)]
pub struct Probe {
    pub id: &'static str,
    pub stage: u8,
    pub truth: Truth,
    pub files: &'static [ProbeFile],
}

/// `text` with every `{KIT}` replaced by `kit_dir`.
pub fn render(text: &str, kit_dir: &str) -> String {
    text.replace("{KIT}", kit_dir)
}

/// The kit's files as written under `kit_dir`: (path relative to it, text), a template rendered.
pub fn rendered(files: &[KitFile], kit_dir: &str) -> Vec<(&'static str, String)> {
    files.iter().map(|f| (f.path, if f.template { render(f.text, kit_dir) } else { f.text.to_string() })).collect()
}
