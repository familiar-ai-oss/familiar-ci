//! `python -m unittest`: Familiar's own reporter, its readers, and its probes, as plain data
//! [draft] (lane AKunittest; plan of record 2026-10-19, A-2 "Readers, best first", "The dry
//! run", A0 spike correction 1: unittest has a reader, through `sitecustomize`).
//!
//! ```text
//!   KIT_FILES ──▶ <kit cache>/unittest/sitecustomize.py          (the lane that owns the cache writes it)
//!   READERS[0] ─▶ PYTHONPATH = <kit>:<the person's>              the person's own sitecustomize is
//!                 FAMILIAR_AI_REPORT = <a temp path>             chained to, never shadowed
//!   the run ────▶ python -m unittest …  (the command, untouched; exit, stdout, stderr the person's)
//!   the kit ────▶ $FAMILIAR_AI_REPORT: the per-test result file, JSON Lines v1
//! ```
//!
//! Only std types, so whichever lane owns the reader table can take these without a shared type.
//! Placeholders in env values: `{kit}` is the folder holding [`KIT_FILES`] in the kit cache,
//! `{report}` the run's own result-file path (a temp path outside the work tree).

use crate::door::shell_forms::{args_after, is_python, program_name, shell_words};

/// The runner these data are for.
pub const RUNNER: &str = "unittest";

/// Why the reporter reader is refused for `command`, when it is: python run with `-I` or `-E`
/// ignores every `PYTHON*` variable, so a kit that rides on `PYTHONPATH` cannot load (the run is
/// safe, it just writes no file; the refusal saves the dry-run probe that would find out).
///
/// Only python's own options count: they end at the first word that is not one (a script), at
/// `-c` or `-m` (what follows is the code or the module, then the program's own), at `-`, at
/// `--`, and at the next separator. `-W` and `-X` take a value, attached or the next word. Any
/// python in the command with `-I` or `-E` refuses it.
pub fn refused(command: &str) -> Option<&'static str> {
    let words: Vec<String> = shell_words(command).into_iter().map(|w| w.text).collect();
    for at in 0..words.len() {
        if !is_python(program_name(&words[at])) {
            continue;
        }
        let mut args = args_after(&words, at).iter();
        'options: while let Some(arg) = args.next() {
            let Some(flags) = arg.strip_prefix('-') else { break };
            if flags.is_empty() || flags == "-" {
                break;
            }
            if flags.starts_with('-') {
                continue; // a long option (--version, --check-hash-based-pycs) is neither -I nor -E
            }
            for (i, c) in flags.char_indices() {
                match c {
                    'I' => return Some("python -I ignores PYTHONPATH, so the kit cannot load"),
                    'E' => return Some("python -E ignores PYTHONPATH, so the kit cannot load"),
                    'c' | 'm' => break 'options,
                    'W' | 'X' => {
                        if i + 1 == flags.len() {
                            args.next();
                        }
                        break;
                    }
                    _ => {}
                }
            }
        }
    }
    None
}

/// The kit's files: (path inside the kit's folder, contents).
pub const KIT_FILES: &[(&str, &str)] = &[("sitecustomize.py", include_str!("../../../kit/unittest/sitecustomize.py"))];

/// An environment operation: (variable, op `set|append|prepend|unset`, value, separator). The
/// separator joins a value to the person's own for `append` and `prepend`; `""` otherwise.
pub type Env = (&'static str, &'static str, &'static str, &'static str);

/// A reader: (id, kind `reporter|report|pinned|tables`, env operations, flags added to Familiar's
/// own run only, report-file globs). The id is the only thing `.familiar/adapter` names.
pub type Reader = (&'static str, &'static str, &'static [Env], &'static [&'static str], &'static [&'static str]);

/// The readers, best first. The reporter is env only (the command is never touched), and a run it
/// did not see (no file, an unfinished one) falls to the regex tables.
pub const READERS: &[Reader] = &[
    ("unittest:reporter", "reporter", &[("PYTHONPATH", "prepend", "{kit}", ":"), ("FAMILIAR_AI_REPORT", "set", "{report}", "")], &[], &[]),
    ("unittest:tables", "tables", &[], &[], &[]),
];

/// The flags `re_aim` drops from the claimed command: (flag, takes a value). `-k` picks tests by
/// name; the rest belong to `discover` and name where to look.
pub const SELECTION_FLAGS: &[(&str, bool)] = &[("-k", true), ("-s", true), ("--start-directory", true), ("-p", true), ("--pattern", true), ("-t", true), ("--top-level-directory", true)];

/// The subcommands `re_aim` drops with their options: `discover` takes positionals of its own, so
/// a probe's target after it would be read as the start directory.
pub const DROPPED_SUBCOMMANDS: &[&str] = &["discover"];

/// The files whose change is a change of how the tests run (globs, as the plan's `watched`): the
/// interpreter and its packages, and the startup files the kit chains to.
pub const WATCHED: &[&str] = &[
    "pyproject.toml",
    "requirements*.txt",
    "setup.cfg",
    "setup.py",
    "tox.ini",
    "Pipfile",
    "Pipfile.lock",
    "poetry.lock",
    "uv.lock",
    ".python-version",
    "**/sitecustomize.py",
    "**/usercustomize.py",
];

/// A probe: (id, stage, files, target, label, kind, place).
///
/// * `stage`: 1 is read at the tests-first commit, 2 at the fix.
/// * `files`: (name, contents), placed together in one folder (the project's test location); the
///   names carry `familiar_probe` so they cannot collide with the project's own, and a test file
///   puts its own folder on `sys.path` so it finds its sibling wherever it is placed.
/// * `target`: the file to name on the command, relative to that folder.
/// * `label`, `kind`: the truth, the outcome label and, for `tests_failed`, what kind of error.
/// * `place`: where the deepest project frame must be (`file::function`), for P7 only.
pub type Probe = (&'static str, u8, &'static [(&'static str, &'static str)], &'static str, &'static str, Option<&'static str>, Option<&'static str>);

macro_rules! head {
    () => {
        "import os\nimport sys\nimport unittest\n\nsys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))\n"
    };
}

const CALC: &str = "def add(a, b):\n    return a + b\n\n\ndef parse(s):\n    return int(s)\n";
const CALC_WITH_MUL: &str = "def add(a, b):\n    return a + b\n\n\ndef parse(s):\n    return int(s)\n\n\ndef mul(a, b):\n    return a * b\n";

/// P1 pass, P2 assertion, P3 a missing function, P4 a broken import, P5 a pass beside a skip, P6
/// nothing selected (stage 1); P3b the function now exists, P7 an exception in the code (stage 2).
pub const PROBES: &[Probe] = &[
    (
        "P1",
        1,
        &[("familiar_probe_calc.py", CALC), ("test_familiar_probe_p1.py", concat!(head!(), "from familiar_probe_calc import add\n\n\nclass T(unittest.TestCase):\n    def test_add(self):\n        self.assertEqual(add(2, 2), 4)\n"))],
        "test_familiar_probe_p1.py",
        "passed",
        None,
        None,
    ),
    (
        "P2",
        1,
        &[("familiar_probe_calc.py", CALC), ("test_familiar_probe_p2.py", concat!(head!(), "from familiar_probe_calc import add\n\n\nclass T(unittest.TestCase):\n    def test_add(self):\n        self.assertEqual(add(2, 2), 5)\n"))],
        "test_familiar_probe_p2.py",
        "tests_failed",
        Some("assertion"),
        None,
    ),
    (
        "P3",
        1,
        &[("familiar_probe_calc.py", CALC), ("test_familiar_probe_p3.py", concat!(head!(), "from familiar_probe_calc import mul\n\n\nclass T(unittest.TestCase):\n    def test_mul(self):\n        self.assertEqual(mul(2, 3), 6)\n"))],
        "test_familiar_probe_p3.py",
        "missing_target_symbol",
        None,
        None,
    ),
    (
        "P4",
        1,
        &[
            ("familiar_probe_calc.py", CALC),
            ("test_familiar_probe_p4.py", concat!(head!(), "import familiar_probe_nosuch\nfrom familiar_probe_calc import add\n\n\nclass T(unittest.TestCase):\n    def test_add(self):\n        self.assertEqual(add(2, 2), 4)\n")),
        ],
        "test_familiar_probe_p4.py",
        "collection_error",
        None,
        None,
    ),
    (
        "P5",
        1,
        &[
            ("familiar_probe_calc.py", CALC),
            (
                "test_familiar_probe_p5.py",
                concat!(
                    head!(),
                    "from familiar_probe_calc import add\n\n\nclass T(unittest.TestCase):\n    def test_add(self):\n        self.assertEqual(add(2, 2), 4)\n\n    @unittest.skip(\"later\")\n    def test_later(self):\n        self.assertEqual(add(1, 1), 2)\n"
                ),
            ),
        ],
        "test_familiar_probe_p5.py",
        "passed",
        None,
        None,
    ),
    (
        "P6",
        1,
        &[("familiar_probe_calc.py", CALC), ("test_familiar_probe_p6.py", concat!(head!(), "from familiar_probe_calc import add\n\n\ndef helper():\n    return add(1, 1)\n"))],
        "test_familiar_probe_p6.py",
        "no_tests_collected",
        None,
        None,
    ),
    (
        "P3b",
        2,
        &[("familiar_probe_calc.py", CALC_WITH_MUL), ("test_familiar_probe_p3b.py", concat!(head!(), "from familiar_probe_calc import mul\n\n\nclass T(unittest.TestCase):\n    def test_mul(self):\n        self.assertEqual(mul(2, 3), 6)\n"))],
        "test_familiar_probe_p3b.py",
        "passed",
        None,
        None,
    ),
    (
        "P7",
        2,
        &[("familiar_probe_calc.py", CALC), ("test_familiar_probe_p7.py", concat!(head!(), "from familiar_probe_calc import parse\n\n\nclass T(unittest.TestCase):\n    def test_parse(self):\n        self.assertEqual(parse(\"x1\"), 1)\n"))],
        "test_familiar_probe_p7.py",
        "tests_failed",
        Some("exception"),
        Some("familiar_probe_calc.py::parse"),
    ),
];
