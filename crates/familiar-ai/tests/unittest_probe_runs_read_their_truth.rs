//! Lane AKunittest (plan of record 2026-10-19, A-2 and "The per-test result file"): the
//! `unittest` kit's output, as the A0 spike recorded it, reads each probe's truth through the
//! per-test result file's schema, and never reads wrong.
//!
//! ```text
//!   python -m unittest ──▶ sitecustomize (kit on PYTHONPATH) ──▶ $FAMILIAR_AI_REPORT
//!        │                                                            │
//!        └─ exit, stdout, stderr: the person's, untouched             ▼
//!                                                      one reading, or "next reader"
//! ```
//!
//! The recordings are `tests/fixtures/probe_runs/unittest/<setup>/<probe>/` (the spike's own
//! layout: cmd, person_cmd, injection.json, exit, version, stdout, stderr, report/), copied
//! as they were recorded (A0 spike, CPython 3.14.4, 2026-10-19). Setups: `plain`, `q` (-q), `b`
//! (-b), `user_sitecustomize` (the person's own sitecustomize on PYTHONPATH after the kit's),
//! `isolated` (python -I: the kit does not load) and `no_reporter` (no kit at all).
//!
//! Probes (the plan's dry run, P1–P7): P1 pass · P2 assertion · P3 missing function · P4 broken
//! import · P5 a pass beside a skip · P6 nothing selected · P3b the function now exists · P7 an
//! exception in the code. The truths are the data module's (`door::kits::unittest::PROBES`).

mod schema_truth;

use familiar_ai::door::kits::unittest as kit;
use schema_truth::{fixtures, read, recorded, Reading};

/// The setups where the kit loads and writes the file.
const LOADS: &[&str] = &["plain", "q", "b", "user_sitecustomize"];
/// The setups where it never loads: nothing is written and the person's run is untouched.
const NEVER_LOADS: &[&str] = &["isolated", "no_reporter"];

fn truth(probe: &str) -> Reading {
    let (_, _, _, _, label, kind, _) = kit::PROBES.iter().find(|p| p.0 == probe).unwrap_or_else(|| panic!("{probe} is not a probe of the data module"));
    Reading::is(label, *kind)
}

fn probe_ids() -> Vec<&'static str> {
    kit::PROBES.iter().map(|p| p.0).collect()
}

#[test]
fn every_probe_reads_its_truth_in_every_setup_where_the_kit_loads() {
    for setup in LOADS {
        for probe in probe_ids() {
            let r = recorded("unittest", setup, probe);
            assert!(r.report.is_some(), "{setup}/{probe}: the kit wrote no file");
            assert_eq!(read(r.report.as_deref(), r.exit), truth(probe), "{setup}/{probe} (exit {}) reads wrong", r.exit);
        }
    }
}

#[test]
fn where_the_kit_does_not_load_nothing_is_written_and_the_run_is_the_persons() {
    for setup in NEVER_LOADS {
        for probe in probe_ids() {
            let r = recorded("unittest", setup, probe);
            let plain = recorded("unittest", "plain", probe);
            assert_eq!(r.report, None, "{setup}/{probe}: a file where the kit cannot have loaded");
            assert!(matches!(read(r.report.as_deref(), r.exit), Reading::Next(_)), "{setup}/{probe}: read something from no file");
            assert_eq!(r.exit, plain.exit, "{setup}/{probe}: the person's own exit changed");
        }
    }
}

#[test]
fn the_persons_command_is_what_ran_in_every_setup() {
    // the kit is env only: the command that ran is the command claimed, word for word
    for setup in LOADS.iter().chain(NEVER_LOADS) {
        for probe in probe_ids() {
            let r = recorded("unittest", setup, probe);
            assert_eq!(r.cmd, r.person_cmd, "{setup}/{probe}: Familiar's run changed the command");
        }
    }
}

#[test]
fn the_persons_own_sitecustomize_ran_beside_the_kit() {
    for probe in probe_ids() {
        let r = recorded("unittest", "user_sitecustomize", probe);
        assert_eq!(r.person_site.as_deref().map(str::trim), Some("person sitecustomize ran"), "{probe}: the person's sitecustomize did not run");
    }
}

#[test]
fn the_readings_are_the_spikes_own_summary_row_for_row() {
    let summary = std::fs::read_to_string(fixtures("unittest").join("SUMMARY.tsv")).expect("the spike's summary");
    let mut rows = 0;
    for row in summary.lines().skip(1).map(|l| l.split('\t').collect::<Vec<_>>()) {
        let (setup, probe, exit, file, read_word, truth_word, verdict) = (row[0], row[1], row[2], row[3], row[4], row[5], row[6]);
        let r = recorded("unittest", setup, probe);
        assert_eq!(r.exit.to_string(), exit, "{setup}/{probe}");
        let reading = read(r.report.as_deref(), r.exit);
        assert_eq!(reading.word(), read_word, "{setup}/{probe}: the spike read it differently");
        assert_eq!(file == "-", r.report.is_none(), "{setup}/{probe}");
        assert_eq!(truth(probe).word(), truth_word, "{setup}/{probe}: the data module's truth is not the spike's");
        assert_eq!(verdict, if LOADS.contains(&setup) { "right" } else { "cant_tell" }, "{setup}/{probe}");
        rows += 1;
    }
    assert_eq!(rows, (LOADS.len() + NEVER_LOADS.len()) * kit::PROBES.len(), "a recording has no row, or a row no recording");
}

#[test]
fn a_file_missing_any_one_line_is_no_structured_output() {
    // the session line, a test or collect line, the end line: drop any one and the reader must not read it
    for probe in probe_ids() {
        let r = recorded("unittest", "plain", probe);
        let text = r.report.expect("a recorded file");
        let lines: Vec<&str> = text.lines().collect();
        for dropped in 0..lines.len() {
            let kept: Vec<&str> = lines.iter().enumerate().filter(|(i, _)| *i != dropped).map(|(_, l)| *l).collect();
            let got = read(Some(&kept.join("\n")), r.exit);
            assert!(matches!(got, Reading::Next(_)), "{probe}: read {got:?} without its line {dropped}: {}", lines[dropped]);
        }
    }
}

#[test]
fn a_file_cut_anywhere_is_no_structured_output() {
    // a process killed mid-write: every proper prefix of the file, cut at a line or inside one
    for probe in probe_ids() {
        let r = recorded("unittest", "plain", probe);
        let text = r.report.expect("a recorded file");
        let mut ends: Vec<usize> = text.char_indices().filter(|(_, c)| *c == '\n').map(|(i, _)| i).collect();
        ends.extend((1..text.len().saturating_sub(1)).step_by(37));
        for at in ends {
            // a cut that still holds every line's JSON is the whole file, not a cut one
            if !text.is_char_boundary(at) || at >= text.trim_end().len() {
                continue;
            }
            let got = read(Some(&text[..at]), r.exit);
            assert!(matches!(got, Reading::Next(_)), "{probe}: read {got:?} from the first {at} bytes");
        }
        assert!(matches!(read(Some(""), r.exit), Reading::Next(_)), "{probe}: read an empty file");
    }
}

#[test]
fn a_kit_error_hands_the_run_to_the_next_reader() {
    for probe in probe_ids() {
        let r = recorded("unittest", "plain", probe);
        let text = r.report.expect("a recorded file");
        let mut lines: Vec<String> = text.lines().map(str::to_string).collect();
        let at = lines.len() - 1; // before the end line
        lines.insert(at, r#"{"type":"kit_error","where":"on_test","message":"KeyError: 'x'"}"#.to_string());
        let got = read(Some(&lines.join("\n")), r.exit);
        assert!(matches!(got, Reading::Next(_)), "{probe}: read {got:?} beside a kit_error");
    }
}

#[test]
fn a_second_run_in_the_same_file_is_not_read() {
    // two python processes of one command both ran unittest: two sessions, two ends
    let r = recorded("unittest", "plain", "P1");
    let text = r.report.expect("a recorded file");
    let twice = format!("{text}{text}");
    assert!(matches!(read(Some(&twice), r.exit), Reading::Next(_)));
}

#[test]
fn an_end_that_contradicts_the_exit_is_not_read() {
    // (probe, the exit the process would have had instead): the run's own lines say otherwise
    for (probe, exit) in [("P1", 1), ("P1", 5), ("P2", 0), ("P3", 0), ("P4", 0), ("P5", 1), ("P7", 0), ("P6", 1), ("P6", 2), ("P3b", 3)] {
        let r = recorded("unittest", "plain", probe);
        assert_eq!(read(r.report.as_deref(), exit), Reading::is("unclassified", None), "{probe} read with exit {exit}");
    }
}

#[test]
fn a_run_of_skipped_tests_only_is_not_called_nothing_ran() {
    let file = [
        r#"{"type":"session","v":1,"runner":"unittest","runner_version":"3.14.4","language":"python","language_version":"3.14.4","plugins":[]}"#,
        r#"{"type":"test","id":"tests.test_x.T.test_a","file":"tests/test_x.py","line":null,"outcome":"skipped","skip_reason":"later","error":null}"#,
        r#"{"type":"end","exit":0,"counts":{"passed":0,"failed":0,"skipped":1,"deselected":0,"errors":0}}"#,
    ]
    .join("\n");
    assert_eq!(read(Some(&file), 0), Reading::is("unclassified", None));
}

#[test]
fn the_kit_folder_holds_exactly_the_files_the_data_module_lists() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("kit/unittest");
    let mut on_disk: Vec<String> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
        .flatten()
        .map(|e| e.file_name().to_string_lossy().to_string())
        .filter(|n| n != "__pycache__")
        .collect();
    on_disk.sort();
    let mut listed: Vec<String> = kit::KIT_FILES.iter().map(|(path, _)| path.to_string()).collect();
    listed.sort();
    assert_eq!(on_disk, listed, "the kit folder and KIT_FILES disagree");
    for (path, contents) in kit::KIT_FILES {
        assert_eq!(&std::fs::read_to_string(dir.join(path)).expect("a kit file"), contents, "{path}");
        assert!(!contents.is_empty(), "{path} is empty");
    }
    assert_eq!(listed, ["sitecustomize.py"]);
}

#[test]
fn the_readers_are_the_kit_then_the_tables() {
    let ids: Vec<&str> = kit::READERS.iter().map(|r| r.0).collect();
    assert_eq!(ids, ["unittest:reporter", "unittest:tables"], "best first, the regex tables last");
    for (id, kind, env, _flags, _globs) in kit::READERS {
        assert!(["reporter", "report", "pinned", "tables"].contains(kind), "{id}: kind {kind}");
        for (var, op, _value, sep) in *env {
            assert!(!var.is_empty());
            assert!(["set", "append", "prepend", "unset"].contains(op), "{id}: {var} op {op}");
            assert_eq!(!sep.is_empty(), ["append", "prepend"].contains(op), "{id}: {var} has a separator exactly when it joins");
        }
    }
    let (_, kind, env, flags, globs) = kit::READERS[0];
    assert_eq!(kind, "reporter");
    assert_eq!(env, [("PYTHONPATH", "prepend", "{kit}", ":"), ("FAMILIAR_AI_REPORT", "set", "{report}", "")], "env only: the kit first on PYTHONPATH after the person's, the report path set");
    assert!(flags.is_empty() && globs.is_empty(), "the command is never touched, no report files are read");
    let (_, kind, env, flags, globs) = kit::READERS[1];
    assert_eq!(kind, "tables");
    assert!(env.is_empty() && flags.is_empty() && globs.is_empty());
}

#[test]
fn the_probes_are_the_dry_runs_and_each_carries_its_files_and_target() {
    assert_eq!(probe_ids(), ["P1", "P2", "P3", "P4", "P5", "P6", "P3b", "P7"]);
    for (id, stage, files, target, label, kind, place) in kit::PROBES {
        assert_eq!(*stage, if ["P3b", "P7"].contains(id) { 2 } else { 1 }, "{id}: stage 1 is the tests-first commit, stage 2 the fix");
        assert!(files.iter().any(|(path, _)| path == target), "{id}: the target {target} is not one of its files");
        assert!(files.iter().all(|(path, contents)| path.ends_with(".py") && !path.contains('/') && !contents.is_empty()), "{id}: files sit beside each other");
        assert!(files.iter().all(|(path, _)| path.contains("familiar_probe")), "{id}: a probe's file names cannot collide with the project's");
        assert!(["passed", "tests_failed", "missing_target_symbol", "collection_error", "no_tests_collected"].contains(label), "{id}: {label}");
        assert_eq!(kind.is_some(), *label == "tests_failed", "{id}: a kind exactly for a failed test");
        assert_eq!(place.is_some(), *id == "P7", "{id}: only P7 is located inside the probe's function");
    }
    let place = kit::PROBES.iter().find(|p| p.0 == "P7").and_then(|p| p.6);
    assert_eq!(place, Some("familiar_probe_calc.py::parse"));
}

#[test]
fn what_re_aim_drops_and_what_a_setup_commit_watches() {
    assert!(kit::SELECTION_FLAGS.contains(&("-k", true)), "-k takes a pattern");
    for flag in ["-s", "--start-directory", "-p", "--pattern", "-t", "--top-level-directory"] {
        assert!(kit::SELECTION_FLAGS.contains(&(flag, true)), "{flag} (discover) names where to look and takes a value");
    }
    assert!(kit::SELECTION_FLAGS.iter().all(|(f, _)| f.starts_with('-')));
    assert_eq!(kit::DROPPED_SUBCOMMANDS, ["discover"], "discover's positionals would swallow the probe's target");
    for file in ["pyproject.toml", "requirements*.txt", "setup.cfg", "tox.ini", "**/sitecustomize.py", "**/usercustomize.py"] {
        assert!(kit::WATCHED.contains(&file), "{file} changes how the tests run");
    }
}
