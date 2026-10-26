//! The dry run (plan A; lane a-py for pytest, lane A-wire for the other runners). Probes with
//! known truths are placed, one at a time, at the project's test location in a copy of the commit
//! and run by the planned command re-aimed at the probe, under each reader best first
//! ([`crate::door::readers::READERS`]); a probe is read as a real run is read (the reader's result
//! file, else the tables). The first reader that reads every probe right is chosen, else the first
//! that reads none wrong (its can't-tell probes are the gaps), else `none`. When no reader reads P1
//! (a plain pass) right, the probe kit could not run here: the tables stay, every probe a gap. The
//! choice is a `reader_chosen` fact.
//!
//! ```text
//!   runner   probes from         placed at                      re-aimed at
//!   pytest   this module         the planned test's folder      the probe's file
//!   unittest kits::unittest      the planned test's folder      the probe's file
//!   JS ×4    kits::<runner>      the planned test's folder      the probe's file
//!   go       kits::go_test       a package per probe            ./familiar_probe_<k>
//!   cargo    kits::cargo_test    tests/familiar_probe_<k>.rs    --test familiar_probe_<k>
//!   PHPUnit  kits::phpunit       tests/, src/ (the kit's)       the probe's file
//!   Maven    kits::maven         src/test/java, src/main/java   -Dtest=ProbeTest
//!   Gradle   kits::maven         the same                       --tests calc.ProbeTest
//!   dotnet   kits::dotnet        tests/, src/ (the kit's)       --filter …ProbeTests
//!   C/C++    none: not calibrated, no dry run (the tables read every run)
//! ```
//!
//! The trigger here is the binary's own simple reading ([`owed`]); TODO(lane A5): switch to the
//! rules' `dry_run_owed` and `reader_at` when they land.

use std::path::Path;
use std::time::SystemTime;

use familiar_ai_facts::{Fact, ReaderChosen};
use serde_json::Value;

use crate::door::aim::{self, Aim};
use crate::door::kits;
use crate::door::readers::{self, Left};
use crate::door::verify_run::{run_in_copy, with_copy, Place};

/// The files whose change owes pytest a new dry run (plan A's `watched`), matched by name at any
/// depth.
pub const WATCHED: &[&str] = &["pyproject.toml", "requirements*.txt", "setup.cfg", "tox.ini", "pytest.ini", "**/conftest.py"];

/// One probe: its name, the stage it stands for, the reading that is its truth (and, for a failed
/// test, its kind), its test file's text, every file it places (relative to the copy's project
/// folder, the test first), and the words the command is re-aimed at.
#[derive(Debug, Clone)]
pub struct Probe {
    pub name: &'static str,
    pub stage: u8,
    pub truth: &'static str,
    pub kind: Option<&'static str>,
    pub test: String,
    pub files: Vec<(String, String)>,
    pub target: Vec<String>,
}

/// The probes' own module, beside them; `mul` is missing from it (P3).
const CALC: &str = "def add(a, b):\n    return a + b\n\n\ndef parse(s):\n    return int(s)\n";
/// The stage-2 module, where `mul` exists (P3b).
const CALC_B: &str = "def mul(a, b):\n    return a * b\n";
const IMPORTS: &str = "import os\nimport sys\n\nsys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))\n";

fn stage_of(id: &str) -> u8 {
    if matches!(id, "P3b" | "P7") {
        2
    } else {
        1
    }
}

fn kind_of(k: &'static str) -> Option<&'static str> {
    (!k.is_empty()).then_some(k)
}

/// The pytest probes, in order: stage 1 P1–P6, stage 2 P3b and P7, placed in `tests`.
pub fn probes() -> Vec<Probe> {
    pytest_probes("tests")
}

fn pytest_probes(folder: &str) -> Vec<Probe> {
    let p = |name: &'static str, stage: u8, truth: &'static str, kind: Option<&'static str>, body: &str| {
        let test = format!("# familiar-ai probe {name}\n{IMPORTS}{body}");
        let rel = under(folder, &format!("test_familiar_probe_{}.py", name.to_lowercase()));
        let files = vec![(rel.clone(), test.clone()), (under(folder, "familiar_probe_calc.py"), CALC.to_string()), (under(folder, "familiar_probe_calc_b.py"), CALC_B.to_string())];
        Probe { name, stage, truth, kind, test, files, target: vec![rel] }
    };
    vec![
        p("P1", 1, "passed", None, "from familiar_probe_calc import add\n\n\ndef test_familiar_probe():\n    assert add(2, 2) == 4\n"),
        p("P2", 1, "tests_failed", Some("assertion"), "from familiar_probe_calc import add\n\n\ndef test_familiar_probe():\n    assert add(2, 2) == 5\n"),
        p("P3", 1, "missing_target_symbol", None, "from familiar_probe_calc import mul\n\n\ndef test_familiar_probe():\n    assert mul(2, 3) == 6\n"),
        p("P4", 1, "collection_error", None, "import familiar_probe_nosuch\n\n\ndef test_familiar_probe():\n    assert familiar_probe_nosuch\n"),
        p("P5", 1, "passed", None, "import pytest\nfrom familiar_probe_calc import add\n\n\ndef test_familiar_probe():\n    assert add(2, 2) == 4\n\n\n@pytest.mark.skip(reason=\"later\")\ndef test_familiar_probe_later():\n    assert add(1, 1) == 2\n"),
        p("P6", 1, "no_tests_collected", None, "from familiar_probe_calc import add\n\n\ndef familiar_helper():\n    return add(1, 1)\n"),
        p("P3b", 2, "passed", None, "from familiar_probe_calc_b import mul\n\n\ndef test_familiar_probe():\n    assert mul(2, 3) == 6\n"),
        p("P7", 2, "tests_failed", Some("exception"), "from familiar_probe_calc import parse\n\n\ndef test_familiar_probe():\n    assert parse(\"x1\") == 1\n"),
    ]
}

/// Where a path-named probe goes: `folder/name`.
fn under(folder: &str, name: &str) -> String {
    if folder.is_empty() || folder == "." {
        name.to_string()
    } else {
        format!("{folder}/{name}")
    }
}

/// A tuple kit's probes (id, truth label, truth kind, files: the test first).
type TupleProbe = (&'static str, &'static str, &'static str, &'static [(&'static str, &'static str)]);

/// A tuple kit's probes (the JS four, go, cargo) as [`Probe`]s.
fn from_tuples(list: &[TupleProbe], folder: Option<&str>, target: impl Fn(&str) -> Vec<String>) -> Vec<Probe> {
    list.iter()
        .map(|(id, label, kind, files)| {
            let placed: Vec<(String, String)> = files.iter().map(|(p, t)| (folder.map_or_else(|| p.to_string(), |f| under(f, p)), t.to_string())).collect();
            let test_path = placed.first().map(|f| f.0.clone()).unwrap_or_default();
            Probe { name: id, stage: stage_of(id), truth: label, kind: kind_of(kind), test: files.first().map(|f| f.1.to_string()).unwrap_or_default(), target: target(&test_path), files: placed }
        })
        .collect()
}

/// A `spec` kit's probes (PHPUnit, Maven, Gradle, dotnet) as [`Probe`]s, placed at the kit's own
/// paths.
fn from_spec(list: &[kits::spec::Probe], target: impl Fn(&str) -> Vec<String>) -> Vec<Probe> {
    list.iter()
        .map(|p| Probe {
            name: p.id,
            stage: p.stage,
            truth: p.truth.label,
            kind: p.truth.kind,
            test: p.files.first().map(|f| f.text.to_string()).unwrap_or_default(),
            files: p.files.iter().map(|f| (f.path.to_string(), f.text.to_string())).collect(),
            target: target(p.files.first().map(|f| f.path).unwrap_or("")),
        })
        .collect()
}

/// The probes of `runner`, the path-named ones placed in `folder`; empty for a runner with no
/// probe kit (C/C++: not calibrated).
pub fn probes_for(runner: &str, folder: &str) -> Vec<Probe> {
    let path = |test: &str| vec![test.to_string()];
    match runner {
        "pytest" => pytest_probes(folder),
        "unittest" => kits::unittest::PROBES
            .iter()
            .map(|(id, stage, files, target, label, kind, _)| Probe {
                name: id,
                stage: *stage,
                truth: label,
                kind: *kind,
                test: files.iter().find(|f| f.0 == *target).map(|f| f.1.to_string()).unwrap_or_default(),
                // the test first
                files: files.iter().filter(|f| f.0 == *target).chain(files.iter().filter(|f| f.0 != *target)).map(|(p, t)| (under(folder, p), t.to_string())).collect(),
                target: vec![under(folder, target)],
            })
            .collect(),
        "jest" => from_tuples(kits::jest::PROBES, Some(folder), path),
        "vitest" => from_tuples(kits::vitest::PROBES, Some(folder), path),
        "mocha" => from_tuples(kits::mocha::PROBES, Some(folder), path),
        "node-test" => from_tuples(kits::node::PROBES, Some(folder), path),
        "go-test" => from_tuples(kits::go_test::PROBES, None, |test| vec![format!("./{}", Path::new(test).parent().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default())]),
        "cargo-test" | "cargo-nextest" => from_tuples(kits::cargo_test::PROBES, None, |test| vec!["--test".to_string(), Path::new(test).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default()]),
        "phpunit" => from_spec(kits::phpunit::PROBES, |test| vec![test.to_string()]),
        "maven" => from_spec(kits::maven::PROBES, |_| vec!["-Dtest=ProbeTest".into(), "-Dsurefire.failIfNoSpecifiedTests=false".into()]),
        "gradle" => from_spec(kits::gradle::PROBES, |_| vec!["--tests".into(), "calc.ProbeTest".into()]),
        // P4's `using Nosuch.Helpers;` is csc's CS0246, which reads missing_target_symbol (the
        // owner, 2026-10-14, all three dotnet tables; the plan's "for .NET the CS0246-style missing
        // name"): that is its truth here, whatever the kit's shared table of truths says
        "dotnet" => from_spec(kits::dotnet::PROBES, |_| vec!["--filter".into(), "FullyQualifiedName~Tests.ProbeTests".into()])
            .into_iter()
            .map(|p| if p.name == "P4" { Probe { truth: "missing_target_symbol", ..p } } else { p })
            .collect(),
        _ => Vec::new(),
    }
}

/// The folder a runner's path-named probes go in when no planned test names one.
fn default_folder(runner: &str) -> &'static str {
    match runner {
        "mocha" => "test",
        _ => "tests",
    }
}

/// How `runner`'s command is re-aimed: its selection flags (from its kit data), the kept flags
/// that take a value, whether positional words are its own (goals, tasks, a project).
pub fn aim_of(runner: &str) -> Option<Aim> {
    let a = |selection: &'static [(&'static str, bool)], with_value: &'static [&'static str]| Aim { selection, with_value, keep_positionals: false, dropped_subcommands: &[] };
    const CARGO: &[&str] = &["-p", "--package", "--features", "-F", "--target", "--target-dir", "--manifest-path", "-j", "--jobs", "--profile", "--exclude", "--config", "-Z", "--color", "--message-format"];
    Some(match runner {
        "unittest" => Aim { dropped_subcommands: kits::unittest::DROPPED_SUBCOMMANDS, ..a(kits::unittest::SELECTION_FLAGS, &[]) },
        "jest" => a(
            kits::jest::SELECTION_FLAGS,
            &["--config", "-c", "--rootDir", "--roots", "--maxWorkers", "-w", "--testTimeout", "--reporters", "--setupFiles", "--setupFilesAfterEnv", "--testEnvironment", "--env", "--coverageDirectory", "--coverageProvider", "--testRunner", "--outputFile", "--seed"],
        ),
        "vitest" => a(kits::vitest::SELECTION_FLAGS, &["--config", "-c", "--root", "-r", "--environment", "--pool", "--reporter", "--outputFile", "--mode", "--maxWorkers", "--minWorkers", "--testTimeout", "--hookTimeout", "--retry"]),
        "mocha" => a(
            kits::mocha::SELECTION_FLAGS,
            &["--config", "--require", "-r", "--reporter", "-R", "--reporter-option", "--reporter-options", "-O", "--ui", "-u", "--timeout", "-t", "--slow", "-s", "--file", "--extension", "--package", "--node-option", "-n", "--jobs", "-j", "--retries"],
        ),
        "node-test" => a(kits::node::SELECTION_FLAGS, &["--import", "--require", "-r", "--loader", "--experimental-loader", "--test-reporter", "--test-reporter-destination", "--test-concurrency", "--test-timeout", "--env-file", "--conditions", "-C", "--input-type"]),
        "go-test" => a(kits::go_test::SELECTION_FLAGS, &["-count", "-timeout", "-p", "-parallel", "-tags", "-coverprofile", "-cpu", "-bench", "-benchtime", "-o", "-exec", "-ldflags", "-gcflags", "-mod", "-modfile", "-covermode", "-coverpkg", "-vet", "-shuffle"]),
        "cargo-test" => a(kits::cargo_test::SELECTION_FLAGS, CARGO),
        "cargo-nextest" => a(
            kits::cargo_nextest::SELECTION_FLAGS,
            &[
                "-p", "--package", "--features", "-F", "--target", "--target-dir", "--manifest-path", "-j", "--jobs", "--exclude", "--config", "-Z", "--color", "--profile", "-P", "--retries", "--cargo-profile", "--tool-config-file", "--test-threads", "--status-level", "--final-status-level", "--failure-output", "--success-output", "--message-format", "--message-format-version", "--max-fail",
            ],
        ),
        "phpunit" => a(
            &[("--filter", true), ("--group", true), ("--testsuite", true), ("--exclude-group", true)],
            &["--configuration", "-c", "--bootstrap", "--log-junit", "--cache-directory", "--coverage-clover", "--coverage-html", "--coverage-text", "--extension", "--order-by", "--random-order-seed", "--testdox-html", "--testdox-text", "--test-suffix", "--log-teamcity"],
        ),
        "maven" => Aim { keep_positionals: true, ..a(&[("-Dtest", true)], &["-f", "--file", "-s", "--settings", "-P", "--activate-profiles", "-pl", "--projects", "-T", "--threads", "-rf", "--resume-from", "-gs", "-l", "--log-file"]) },
        "gradle" => Aim { keep_positionals: true, ..a(&[("--tests", true)], &["-p", "--project-dir", "-b", "--build-file", "-c", "--settings-file", "-x", "--exclude-task", "--max-workers"]) },
        "dotnet" => Aim {
            keep_positionals: true,
            ..a(
                &[("--filter", true)],
                &["-c", "--configuration", "-f", "--framework", "-r", "--runtime", "-s", "--settings", "-l", "--logger", "-a", "--test-adapter-path", "-v", "--verbosity", "--results-directory", "-o", "--output", "--arch", "--os", "--blame-hang-timeout", "--collect", "-d", "--diag"],
            )
        },
        _ => return None,
    })
}

/// The setup files whose change owes `runner` a new dry run (its kit's `WATCHED`).
pub fn watched_for(runner: &str) -> &'static [&'static str] {
    match runner {
        "pytest" => WATCHED,
        "unittest" => kits::unittest::WATCHED,
        "jest" => kits::jest::WATCHED,
        "vitest" => kits::vitest::WATCHED,
        "mocha" => kits::mocha::WATCHED,
        "node-test" => kits::node::WATCHED,
        "go-test" => kits::go_test::WATCHED,
        "cargo-test" => kits::cargo_test::WATCHED,
        "cargo-nextest" => kits::cargo_nextest::WATCHED,
        "phpunit" => kits::phpunit::WATCHED,
        "maven" => kits::maven::WATCHED,
        "gradle" => kits::gradle::WATCHED,
        "dotnet" => kits::dotnet::WATCHED,
        "ctest" => kits::ctest::WATCHED,
        _ => &[],
    }
}

/// Each reader tried, with how it read each probe.
type Readings = Vec<(&'static str, Vec<(&'static str, Verdict)>)>;

/// How a reader read a probe: its truth, can't tell (`unclassified` or nothing), or another label.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    Right,
    CantTell,
    Wrong,
}

/// A reading of a probe against its truth (`label`, and the kind of a failed test): the truth is
/// right; `unclassified` or no reading is can't tell; another label, or the truth's label pointing
/// at another kind of failure (an exception read as an assertion would seal), is wrong.
pub fn verdict(truth: (&str, Option<&str>), label: Option<&str>, kind: Option<&str>) -> Verdict {
    match label {
        None | Some("unclassified") => Verdict::CantTell,
        Some(l) if l != truth.0 => Verdict::Wrong,
        Some(_) => match (truth.1, kind) {
            (Some(want), Some(got)) if want != got => Verdict::Wrong,
            _ => Verdict::Right,
        },
    }
}

/// The choice (plan A): the first reader right on every probe; else the first with no wrong
/// reading, its can't-tell probes the gaps; else `none`, the why naming the probes read wrong.
pub fn choose(results: &[(&str, Vec<(&str, Verdict)>)]) -> (String, Vec<String>, String) {
    if let Some((id, _)) = results.iter().find(|(_, v)| v.iter().all(|(_, x)| *x == Verdict::Right)) {
        return (id.to_string(), Vec::new(), format!("{id} read every probe right"));
    }
    if let Some((id, v)) = results.iter().find(|(_, v)| v.iter().all(|(_, x)| *x != Verdict::Wrong)) {
        let gaps: Vec<String> = v.iter().filter(|(_, x)| *x == Verdict::CantTell).map(|(p, _)| p.to_string()).collect();
        return (id.to_string(), gaps.clone(), format!("{id} read no probe wrong; it cannot tell {}", gaps.join(", ")));
    }
    let wrong: Vec<String> = results.iter().flat_map(|(id, v)| v.iter().filter(|(_, x)| *x == Verdict::Wrong).map(move |(p, _)| format!("{p} under {id}"))).collect();
    // under `none` no run is read, so every probe is a gap and the setup card is owed (plan A-4)
    let gaps: Vec<String> = results.first().map(|(_, v)| v.iter().map(|(p, _)| p.to_string()).collect()).unwrap_or_default();
    ("none".to_string(), gaps, format!("every reader read a probe wrong ({}): a plugin or conftest changes outcomes", wrong.join(", ")))
}

/// [`choose`], with the kit's own failure first: when no reader reads P1 (a plain pass) right, the
/// probes could not run in this project, which says nothing of the readers; the tables stay
/// (`tables`), every probe a gap.
pub fn choose_or_kit_failure(results: &[(&str, Vec<(&str, Verdict)>)], tables: &str) -> (String, Vec<String>, String) {
    let p1_right = results.iter().any(|(_, v)| v.iter().any(|(p, x)| *p == "P1" && *x == Verdict::Right));
    if !results.is_empty() && !p1_right {
        let gaps: Vec<String> = results.first().map(|(_, v)| v.iter().map(|(p, _)| p.to_string()).collect()).unwrap_or_default();
        return (tables.to_string(), gaps, "the probe kit did not run here (P1, a plain pass, read right under no reader): the tables read this runner's runs".to_string());
    }
    choose(results)
}

fn basename(word: &str) -> &str {
    word.rsplit('/').next().unwrap_or(word)
}

/// pytest's selection flags, dropped from a dry run's command; those of the first list take a value.
const SELECTING_WITH_VALUE: &[&str] = &["-k", "-m", "--deselect"];
const SELECTING: &[&str] = &["--lf", "--last-failed", "--ff", "--failed-first", "--sw", "--stepwise", "--sw-skip", "--stepwise-skip", "--nf", "--new-first"];
/// pytest options whose value is the next word, kept with it.
const WITH_VALUE: &[&str] = &[
    "-p", "-c", "-o", "-W", "-r", "-n", "--tb", "--rootdir", "--confcutdir", "--basetemp", "--maxfail", "--durations", "--junitxml", "--junit-xml", "--import-mode", "--log-level", "--log-cli-level", "--color", "--capture", "--dist", "--numprocesses", "--override-ini", "--ignore", "--ignore-glob", "--cov", "--cov-report", "--timeout", "--reruns", "--pythonwarnings", "--config-file",
];

/// One segment of a command re-aimed, when it names pytest: wrappers, the runner and its flags
/// kept; positional targets and the selection flags dropped; `targets` appended.
fn re_aim_segment(segment: &str, targets: &[String]) -> Option<String> {
    let words: Vec<&str> = segment.split_whitespace().collect();
    let at = words.iter().position(|w| matches!(basename(w), "pytest" | "py.test"))?;
    let mut out: Vec<&str> = words[..=at].to_vec();
    let mut i = at + 1;
    while i < words.len() {
        let w = words[i];
        if SELECTING_WITH_VALUE.contains(&w) {
            i += 2;
            continue;
        }
        if SELECTING.contains(&w) || w.starts_with("--deselect=") || ((w.starts_with("-k") || w.starts_with("-m")) && !w.starts_with("--") && w.len() > 2) {
            i += 1;
            continue;
        }
        if WITH_VALUE.contains(&w) {
            out.push(w);
            if let Some(v) = words.get(i + 1) {
                out.push(v);
            }
            i += 2;
            continue;
        }
        if !w.starts_with('-') {
            i += 1; // a positional target
            continue;
        }
        out.push(w);
        i += 1;
    }
    let mut s = out.join(" ");
    for t in targets {
        s.push(' ');
        s.push_str(t);
    }
    Some(s)
}

/// `command` re-aimed at `targets` (plan A's `re_aim`): the last `&&`/`;` segment naming pytest
/// is re-aimed, the others kept; `None` when no segment names pytest (no dry run).
pub fn re_aim(command: &str, targets: &[String]) -> Option<String> {
    // split keeping each separator
    let mut parts: Vec<(String, &str)> = Vec::new();
    let mut rest = command;
    loop {
        let and = rest.find("&&");
        let semi = rest.find(';');
        let (cut, sep, len) = match (and, semi) {
            (Some(a), Some(s)) if s < a => (s, ";", 1),
            (Some(a), _) => (a, "&&", 2),
            (None, Some(s)) => (s, ";", 1),
            (None, None) => {
                parts.push((rest.trim().to_string(), ""));
                break;
            }
        };
        parts.push((rest[..cut].trim().to_string(), sep));
        rest = &rest[cut + len..];
    }
    let which = parts.iter().rposition(|(seg, _)| familiar_ai_run::details_for(seg).is_some_and(|d| d.runner == "pytest"))?;
    parts[which].0 = re_aim_segment(&parts[which].0, targets)?;
    let mut s = String::new();
    for (seg, sep) in parts {
        s.push_str(&seg);
        if !sep.is_empty() {
            s.push_str(if sep == ";" { "; " } else { " && " });
        }
    }
    Some(s)
}

/// `command` re-aimed at `targets` for `runner`: pytest's own re-aim, the others' through
/// [`aim::re_aim`] with the runner's [`aim_of`]. `None` when the command names no such runner.
pub fn re_aim_for(runner: &str, command: &str, targets: &[String]) -> Option<String> {
    if runner == "pytest" {
        return re_aim(command, targets);
    }
    aim::re_aim(command, runner, &aim_of(runner)?, targets)
}

fn text<'a>(f: &'a Fact, k: &str) -> &'a str {
    f.fields.get(k).and_then(Value::as_str).unwrap_or("")
}

/// Whether `name` matches `pattern`, `*` any run of characters.
fn wildcard(pattern: &str, name: &str) -> bool {
    match pattern.split_once('*') {
        None => pattern == name,
        Some((head, tail)) => name.starts_with(head) && (head.len()..=name.len()).filter(|i| name.is_char_boundary(*i)).any(|i| wildcard(tail, &name[i..])),
    }
}

/// Whether `path` is one of `globs` (a glob without a folder, or under `**/`, by its name at any
/// depth; else the whole path).
pub fn watched_by(globs: &[&str], path: &str) -> bool {
    globs.iter().any(|g| match g.strip_prefix("**/") {
        Some(name) if !name.contains('/') => wildcard(name, basename(path)),
        _ if !g.contains('/') => wildcard(g, basename(path)),
        _ => wildcard(g, path),
    })
}

/// Whether `path` is one of [`WATCHED`] (by its name, at any depth).
pub fn watched(path: &str) -> bool {
    watched_by(WATCHED, path)
}

/// The names a claimed stack goes by for each runner a dry run reads (a stack name's words, split
/// at `_`, `-` and spaces, any of them).
fn stack_names(runner: &str) -> &'static [&'static str] {
    match runner {
        "pytest" => &["pytest"],
        "unittest" => &["unittest"],
        "jest" => &["jest"],
        "vitest" => &["vitest"],
        "mocha" => &["mocha"],
        "node-test" => &["node", "nodejs"],
        "go-test" => &["go", "golang", "gotest"],
        "cargo-test" => &["cargo", "rust"],
        "cargo-nextest" => &["nextest", "cargo", "rust"],
        "phpunit" => &["phpunit", "php"],
        "maven" => &["maven", "mvn", "java", "junit"],
        "gradle" => &["gradle", "java", "kotlin", "junit"],
        "dotnet" => &["dotnet", "csharp", "xunit", "nunit", "mstest", "net"],
        _ => &[],
    }
}

/// The runner a dry run is owed for under `command` claimed as stack `name`: the command names a
/// runner with a probe kit, and the stack is claimed as that runner (a shell script named
/// `pytest` under an `sh` stack is not).
pub fn runner_for(name: &str, command: &str) -> Option<&'static str> {
    let details = familiar_ai_run::details_for(command)?;
    let lower = name.to_lowercase();
    let words: Vec<&str> = lower.split(|c: char| c == '_' || c == '-' || c == '.' || c.is_whitespace()).filter(|w| !w.is_empty()).collect();
    let claimed = stack_names(details.runner).iter().any(|n| words.contains(n));
    (claimed && !probes_for(details.runner, "tests").is_empty()).then_some(details.runner)
}

/// The command a dry run is owed for (the binary's simple trigger, TODO(lane A5)): the latest
/// claimed stack command naming a runner with a probe kit, when no dry run was reserved for it
/// since it was claimed or since the latest commit changing a file its runner watches. Once per
/// trigger, never a loop.
pub fn owed(log: &[Fact]) -> Option<String> {
    let stack = log.iter().rev().find(|f| f.kind == "test_stack_detected")?;
    let command = text(stack, "test_command");
    let runner = runner_for(text(stack, "test_stack"), command)?;
    let globs = watched_for(runner);
    let changed = log
        .iter()
        .filter(|f| f.kind == "commit_seen")
        .filter(|f| f.fields.get("files").and_then(Value::as_array).is_some_and(|a| a.iter().filter_map(Value::as_str).any(|p| watched_by(globs, p))))
        .map(|f| f.t)
        .max();
    let since = changed.map_or(stack.t, |c| c.max(stack.t));
    let reserved = log.iter().any(|f| f.kind == "run_reserved" && f.t > since && text(f, "by") == crate::door::runs::DRY_RUN && text(f, "command") == command);
    (!reserved).then(|| command.to_string())
}

/// The folder the path-named probes go in: the first planned test's, else the runner's default.
fn probe_folder(runner: &str, plan_files: &[String]) -> String {
    plan_files.first().and_then(|f| Path::new(f).parent()).map(|p| p.to_string_lossy().into_owned()).filter(|p| !p.is_empty()).unwrap_or_else(|| default_folder(runner).to_string())
}

/// What one probe read as: its label, the kind of the place it points, the versions it named.
type ProbeReading = (Option<String>, Option<String>, Option<String>);

/// How one probe read under one reader, as a real run under it is read: the reader's result file
/// (`reported::read`), else the tables on the console.
#[allow(clippy::too_many_arguments)]
fn read_probe(run_dir: &Path, report_dir: &Path, reader: &readers::Reader, out: &familiar_ai_run::RunOutcome, started: SystemTime, before: &[(String, SystemTime)], details: &familiar_ai_run::RunnerDetails, plan_files: &[String]) -> ProbeReading {
    let exit = match out.end {
        familiar_ai_run::End::Exited(c) => Some(c),
        _ => None,
    };
    let left = Left { stdout: (out.stdout_dropped_bytes == 0).then_some(out.stdout.as_str()), run_dir, report_dir, started, exit, before };
    let kind = |l: &Option<familiar_ai_facts::Location>| l.as_ref().and_then(|l| l.kind).and_then(|k| serde_json::to_value(k).ok()).and_then(|v| v.as_str().map(str::to_string));
    if let Some(r) = readers::result_file(reader.id, &left).and_then(|t| crate::door::reported::read(&t, exit, plan_files)) {
        let k = kind(&r.translation.location);
        return (Some(r.translation.label), k, Some(r.versions));
    }
    use crate::door::kept::{classify, clean, Streams};
    let (stdout, stderr) = (clean(&out.stdout), clean(&out.stderr));
    let report = clean(&crate::door::verify_run::read_reports_since(run_dir, details.reports, Some(started)));
    match classify(out.end, &Streams { stdout: &stdout, stderr: &stderr, report: &report }, Some(details)) {
        Some(t) => {
            let k = kind(&t.location);
            (Some(t.label), k, None)
        }
        None => (None, None, None),
    }
}

/// Run the dry run `run_id` for `command` at `commit` (plan A): every probe under each of the
/// runner's readers, best first, until one reads every probe right; the choice as a
/// `reader_chosen`. `on_start` is told each probe run's process group; `false` cancels the dry run.
pub fn dry_run(p: &Place, run_id: &str, command: &str, commit: &str, plan_files: &[String], on_start: &mut dyn FnMut(i32) -> bool) -> Result<ReaderChosen, String> {
    let details = familiar_ai_run::details_for(command).ok_or("the command names no runner a dry run reads")?;
    let runner = details.runner;
    let folder = probe_folder(runner, plan_files);
    let all = probes_for(runner, &folder);
    if all.is_empty() {
        return Err(format!("{runner} has no probe kit: it is not calibrated, the tables read its runs"));
    }
    let kit = readers::kit_dir()?;
    let wait = p.wait.min(std::time::Duration::from_secs(300));
    let mut versions: Option<String> = None;
    let mut cancelled = false;
    let candidates: Vec<&readers::Reader> = readers::of_runner(runner).into_iter().filter(|r| r.calibrated).collect();
    let tables = candidates.iter().rev().find(|r| r.kind == "tables").map_or("none", |r| r.id);
    let results = with_copy(p, commit, |run_dir, imports| -> Result<Readings, String> {
        let mut results = Vec::new();
        for reader in &candidates {
            let mut verdicts = Vec::new();
            let mut applies = true;
            for probe in &all {
                let aimed = re_aim_for(runner, command, &probe.target).ok_or("the command could not be re-aimed")?;
                let reports = std::env::temp_dir().join(format!("familiar-ai-report-{}-{run_id}-{}", std::process::id(), probe.name));
                let _ = std::fs::remove_dir_all(&reports);
                // a reader that cannot read this command (python -I, a person's own reporter) is
                // not tried, so never chosen
                let Ok(applied) = readers::apply(reader.id, &aimed, &kit, &reports) else {
                    applies = false;
                    break;
                };
                // the probe's files in, the project's own kept aside and put back after
                let mut kept: Vec<(String, Option<Vec<u8>>)> = Vec::new();
                for (rel, text) in &probe.files {
                    let at = run_dir.join(rel);
                    if kept.iter().all(|(k, _)| k != rel) {
                        kept.push((rel.clone(), std::fs::read(&at).ok()));
                    }
                    if let Some(parent) = at.parent() {
                        std::fs::create_dir_all(parent).map_err(|e| format!("the probes' folder could not be made: {e}"))?;
                    }
                    std::fs::write(&at, text).map_err(|e| e.to_string())?;
                }
                let _ = std::fs::create_dir_all(&reports);
                let before = readers::reports_before(reader.id, run_dir);
                let started = SystemTime::now();
                let out = run_in_copy(run_dir, &applied.command, wait, Some(details), imports, &applied.ops, on_start);
                if out.end == familiar_ai_run::End::Cancelled {
                    cancelled = true;
                }
                let test: Vec<String> = probe.files.first().map(|f| f.0.clone()).into_iter().collect();
                let (label, kind, v) = read_probe(run_dir, &reports, reader, &out, started, &before, details, &test);
                if let Some(v) = v {
                    versions.get_or_insert(v);
                }
                let _ = std::fs::remove_dir_all(&reports);
                for (rel, was) in kept {
                    let at = run_dir.join(&rel);
                    let _ = match was {
                        Some(bytes) => std::fs::write(&at, bytes),
                        None => std::fs::remove_file(&at),
                    };
                }
                if cancelled {
                    return Ok(results);
                }
                verdicts.push((probe.name, verdict((probe.truth, probe.kind), label.as_deref(), kind.as_deref())));
            }
            if !applies {
                continue;
            }
            let done = verdicts.iter().all(|(_, v)| *v == Verdict::Right);
            results.push((reader.id, verdicts));
            if done {
                break; // readers are tried lazily, best first
            }
        }
        Ok(results)
    })??;
    if cancelled {
        return Err("the dry run was cancelled".into());
    }
    let (reader, gaps, why) = choose_or_kit_failure(&results, tables);
    Ok(ReaderChosen {
        run_id: run_id.to_string(),
        root: crate::door::runner::root_in_repo(p.root),
        runner: runner.to_string(),
        reader,
        command: command.to_string(),
        commit: commit.to_string(),
        setup: None,
        versions,
        familiar: env!("CARGO_PKG_VERSION").to_string(),
        watched: watched_for(runner).iter().map(|w| w.to_string()).collect(),
        gaps,
        why,
        ran_in: Some("laptop".to_string()),
    })
}
