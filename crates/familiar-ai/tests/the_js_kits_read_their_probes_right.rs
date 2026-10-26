//! Lane AKjs (plan of record 2026-10-19, A: the adapter): Familiar's own reporters for node --test,
//! jest, vitest and mocha, and the data that says how each is injected and proven.
//!
//! ```text
//!   kit/<runner>/…  ──include_str!──▶ door/kits/<runner>.rs        (plain data: files, readers,
//!   (writes SCHEMA JSON Lines                                       probes, selection flags, watched)
//!    to $FAMILIAR_AI_REPORT)                    │
//!                                               ▼
//!   spike recordings under tests/fixtures/probe_runs/<runner>/<setup>/<probe>/
//!        report/results.jsonl ──read through SCHEMA──▶ equals the probe's truth, or "can't tell"
//! ```
//!
//! Never wrong: a run with no session or no end line has no structured output, whatever else the
//! file holds. Kits never print and never raise.

mod probe_reading;

use familiar_ai::door::kits::{jest, mocha, node, vitest};
use probe_reading::{read, recordings, verdict, Verdict};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

/// (variable, op, value, separator): op is `set`, `append`, `prepend` or `unset`.
type EnvOp = (&'static str, &'static str, &'static str, &'static str);
/// (id, kind, env ops, added flags, report globs).
type Reader = (&'static str, &'static str, &'static [EnvOp], &'static [&'static str], &'static [&'static str]);
/// (probe, truth label, truth kind, files: the first is the test the command names).
type Probe = (&'static str, &'static str, &'static str, &'static [(&'static str, &'static str)]);

struct Kit {
    runner: &'static str,
    fixtures: &'static str,
    kit_files: &'static [(&'static str, &'static str)],
    readers: &'static [Reader],
    probes: &'static [Probe],
    selection: &'static [(&'static str, bool)],
    watched: &'static [&'static str],
}

fn kits() -> Vec<Kit> {
    vec![
        Kit { runner: node::RUNNER, fixtures: "node-test", kit_files: node::KIT_FILES, readers: node::READERS, probes: node::PROBES, selection: node::SELECTION_FLAGS, watched: node::WATCHED },
        Kit { runner: jest::RUNNER, fixtures: "jest", kit_files: jest::KIT_FILES, readers: jest::READERS, probes: jest::PROBES, selection: jest::SELECTION_FLAGS, watched: jest::WATCHED },
        Kit { runner: vitest::RUNNER, fixtures: "vitest", kit_files: vitest::KIT_FILES, readers: vitest::READERS, probes: vitest::PROBES, selection: vitest::SELECTION_FLAGS, watched: vitest::WATCHED },
        Kit { runner: mocha::RUNNER, fixtures: "mocha", kit_files: mocha::KIT_FILES, readers: mocha::READERS, probes: mocha::PROBES, selection: mocha::SELECTION_FLAGS, watched: mocha::WATCHED },
    ]
}

fn kit_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("kit")
}

/// The truths of SCHEMA.txt, by probe.
const TRUTHS: &[(&str, &str, &str)] = &[
    ("P1", "passed", ""),
    ("P2", "tests_failed", "assertion"),
    ("P3", "missing_target_symbol", ""),
    ("P3b", "passed", ""),
    ("P4", "collection_error", ""),
    ("P5", "passed", ""),
    ("P6", "no_tests_collected", ""),
    ("P7", "tests_failed", "exception"),
];

const AGENT_MODE: &[&str] = &["AI_AGENT", "CLAUDECODE", "CLAUDE_CODE", "OPENCODE", "GEMINI_CLI", "CURSOR_AGENT", "CODEX_SANDBOX"];

fn truth_of<'a>(kit: &'a Kit, probe: &str) -> (&'a str, &'a str) {
    let p = kit.probes.iter().find(|p| p.0 == probe).unwrap_or_else(|| panic!("{} has no probe {probe}", kit.runner));
    (p.1, p.2)
}

#[test]
fn every_runner_is_named_and_carries_its_kit() {
    let want: &[(&str, &[&str])] = &[
        ("node-test", &["node/common.cjs", "node/reporter.mjs"]),
        ("jest", &["jest/reporter.cjs", "node/common.cjs"]),
        ("vitest", &["vitest/reporter.mjs", "node/common.cjs"]),
        ("mocha", &["mocha/load.cjs", "mocha/reporter.cjs", "node/common.cjs"]),
    ];
    for (kit, (runner, files)) in kits().iter().zip(want) {
        assert_eq!(kit.runner, *runner);
        let have: BTreeSet<&str> = kit.kit_files.iter().map(|f| f.0).collect();
        assert_eq!(have, files.iter().copied().collect::<BTreeSet<_>>(), "{runner}: the kit's files");
        for (path, text) in kit.kit_files {
            assert!(!text.trim().is_empty(), "{runner}: {path} is empty");
            let on_disk = std::fs::read_to_string(kit_dir().join(path)).unwrap_or_else(|e| panic!("{path}: {e}"));
            assert_eq!(&on_disk, text, "{runner}: {path} is the file under kit/");
        }
    }
}

#[test]
fn every_file_under_kit_is_listed_by_a_runner() {
    let listed: BTreeSet<&str> = kits().iter().flat_map(|k| k.kit_files.iter().map(|f| f.0)).collect();
    let mut on_disk = BTreeSet::new();
    let mut stack = vec![kit_dir()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).into_iter().flatten().flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if let Ok(rel) = p.strip_prefix(kit_dir()) {
                let rel = rel.to_string_lossy().into_owned();
                // another lane's kit directories sit beside ours
                if ["node/", "jest/", "vitest/", "mocha/"].iter().any(|d| rel.starts_with(d)) {
                    on_disk.insert(rel);
                }
            }
        }
    }
    let on_disk: BTreeSet<&str> = on_disk.iter().map(String::as_str).collect();
    assert_eq!(on_disk, listed, "a kit file nobody lists is dead weight, a listed one that is gone does not build");
}

#[test]
fn kits_never_print_to_the_console() {
    for kit in kits() {
        for (path, text) in kit.kit_files {
            for banned in ["console.", "process.stdout", "process.stderr", "print("] {
                assert!(!text.contains(banned), "{}: {path} writes to the console with {banned:?}", kit.runner);
            }
            assert!(text.contains("try") || path.ends_with("common.cjs"), "{}: {path} guards nothing with try", kit.runner);
        }
    }
}

#[test]
fn the_probes_carry_the_truths_of_the_schema() {
    for kit in kits() {
        let ids: Vec<&str> = kit.probes.iter().map(|p| p.0).collect();
        assert_eq!(ids, TRUTHS.iter().map(|t| t.0).collect::<Vec<_>>(), "{}: probes in order", kit.runner);
        for (probe, truth) in kit.probes.iter().zip(TRUTHS) {
            assert_eq!((probe.1, probe.2), (truth.1, truth.2), "{} {}: its truth", kit.runner, probe.0);
            assert!(!probe.3.is_empty(), "{} {}: no files", kit.runner, probe.0);
            for (path, text) in probe.3 {
                assert!(!path.starts_with('/') && !path.contains(".."), "{} {}: {path} is not relative to where the probe is placed", kit.runner, probe.0);
                assert!(!text.trim().is_empty(), "{} {}: {path} is empty", kit.runner, probe.0);
            }
        }
        // the second stage changes only the code, never the test
        let of = |p: &str| kit.probes.iter().find(|x| x.0 == p).unwrap().3;
        assert_eq!(of("P3")[0], of("P3b")[0], "{}: P3 and P3b run the same test", kit.runner);
        assert_ne!(of("P3")[1], of("P3b")[1], "{}: P3b gives the code the function", kit.runner);
    }
}

#[test]
fn every_recording_reads_its_truth_or_cant_tell_and_never_wrong() {
    // runs whose injection could not work: the spike's controls, read as "no structured output"
    let no_file: &[(&str, &str, Option<&str>)] = &[
        ("node-test", "node24_noinject", None),
        ("node-test", "person_reporter", None),
        ("vitest", "plain_noinject", None),
        ("vitest", "agent_env_noinject", None),
        ("mocha", "reporter_only", Some("P4")),
    ];
    let mut right = 0;
    for kit in kits() {
        let recs = recordings(kit.fixtures);
        assert!(recs.len() >= 32, "{}: only {} recordings", kit.runner, recs.len());
        for rec in recs {
            let reading = read(&rec.report(), rec.exit());
            let v = verdict(&reading, truth_of(&kit, &rec.probe));
            let controlled = no_file.iter().any(|(r, s, p)| *r == kit.fixtures && *s == rec.setup && p.is_none_or(|p| p == rec.probe));
            let at = format!("{}/{}/{}: read {} ({})", kit.fixtures, rec.setup, rec.probe, reading.spelled(), reading.why);
            assert_ne!(v, Verdict::Wrong, "{at}: a wrong reading");
            if controlled {
                assert_eq!(reading.label, "next_reader", "{at}: a control has no structured output");
            } else {
                assert_eq!(v, Verdict::Right, "{at}: should be the truth {:?}", truth_of(&kit, &rec.probe));
                right += 1;
            }
        }
    }
    assert!(right >= 112, "only {right} recordings read right");
}

#[test]
fn a_cut_report_has_no_structured_output() {
    let mut cut = 0;
    for kit in kits() {
        for rec in recordings(kit.fixtures) {
            let report = rec.report();
            let lines: Vec<&str> = report.lines().collect();
            if lines.len() < 2 || read(&report, rec.exit()).label == "next_reader" {
                continue;
            }
            let without = |t: &str| lines.iter().filter(|l| !l.contains(&format!("\"type\":\"{t}\""))).copied().collect::<Vec<_>>().join("\n");
            for (what, text) in [
                ("no end line", without("end")),
                ("no session line", without("session")),
                ("the session line alone", lines[0].to_string()),
                ("the last line cut mid-way", format!("{}\n{}", lines[..lines.len() - 1].join("\n"), &lines[lines.len() - 1][..lines[lines.len() - 1].len() / 2])),
                ("junk", "not json at all\n{\"type\":\"end\"".to_string()),
                ("a kit_error among the lines", format!("{}\n{{\"type\":\"kit_error\",\"message\":\"x\"}}\n{}", lines[0], lines[1..].join("\n"))),
            ] {
                let got = read(&text, rec.exit());
                assert_eq!(got.label, "next_reader", "{}/{}/{} with {what}: read {}", kit.fixtures, rec.setup, rec.probe, got.spelled());
                cut += 1;
            }
        }
    }
    assert!(cut > 500, "only {cut} cut reports were tried");
}

/// The reader each recorded setup was injected with: (runner folder, setup) -> (reader id, node's default reporter).
fn reader_of(runner: &str, setup: &str) -> Option<(&'static str, &'static str)> {
    match (runner, setup) {
        ("node-test", "plain" | "person_nodeopts" | "node24_tap") => Some(("node:reporter", "tap")),
        ("node-test", "node24_spec") => Some(("node:reporter", "spec")),
        ("node-test", "person_reporter_flags") => Some(("node:reporter-flags", "")),
        ("jest", _) => Some(("jest:reporter", "")),
        ("vitest", "plain" | "related" | "json_cfg" | "agent_env") => Some(("vitest:reporter", "")),
        ("mocha", "plain" | "json_mocharc" | "json_cli") => Some(("mocha:reporter", "")),
        _ => None, // the spike's controls: no injection, or an injection known not to work
    }
}

#[test]
fn the_readers_make_the_injection_the_spike_recorded() {
    let mut checked = 0;
    for kit in kits() {
        for rec in recordings(kit.fixtures) {
            let Some((id, default)) = reader_of(kit.fixtures, &rec.setup) else { continue };
            let reader = kit.readers.iter().find(|r| r.0 == id).unwrap_or_else(|| panic!("{} has no reader {id}", kit.runner));
            let inj = rec.injection();
            let report_dir = inj["report_dir"].as_str().expect("a report dir").to_string();
            // <scratchpad>/a0-spike/.tmp/rep-x  and  <scratchpad>/a0-spike/kit
            let spike = Path::new(&report_dir).parent().and_then(Path::parent).expect("the spike's folder").to_path_buf();
            let fill = |s: &str| s.replace("{KIT}", &spike.join("kit").to_string_lossy()).replace("{REPORT_DIR}", &report_dir).replace("{NODE_DEFAULT_REPORTER}", default);
            let at = format!("{}/{}/{}", kit.fixtures, rec.setup, rec.probe);
            assert_eq!(fill(&reader.3.join(" ")), inj["flags"].as_str().unwrap_or(""), "{at}: the flags added");
            // every env op the spike recorded is one of the reader's, with the same value
            for op in inj["env"].as_array().expect("env ops") {
                let (kind, var, value) = (op[0].as_str().unwrap(), op[1].as_str().unwrap(), op[2].as_str().unwrap_or(""));
                if kind == "set" {
                    continue; // the person's own environment in the setup (NODE_OPTIONS, CLAUDECODE, AI_AGENT)
                }
                let mine = reader.2.iter().find(|o| o.0 == var && o.1 == kind).unwrap_or_else(|| panic!("{at}: {id} has no {kind} {var}"));
                assert_eq!(fill(mine.2), value, "{at}: {kind} {var}");
            }
            let report = reader.2.iter().find(|o| o.0 == "FAMILIAR_AI_REPORT" && o.1 == "set").unwrap_or_else(|| panic!("{at}: {id} does not say where the report goes"));
            assert_eq!(fill(report.2), inj["final_env"]["FAMILIAR_AI_REPORT"].as_str().unwrap(), "{at}: the report's path");
            checked += 1;
        }
    }
    assert!(checked >= 100, "only {checked} injections compared");
}

#[test]
fn readers_are_plain_data_with_a_closed_vocabulary() {
    for kit in kits() {
        let short = kit.readers[0].0.split(':').next().unwrap();
        let mut ids = BTreeSet::new();
        for (id, kind, env, flags, globs) in kit.readers {
            assert!(ids.insert(*id), "{}: {id} twice", kit.runner);
            assert!(id.starts_with(&format!("{short}:")), "{}: {id} is not {short}:<kind>", kit.runner);
            assert!(["reporter", "report", "pinned", "tables"].contains(kind), "{id}: kind {kind}");
            assert!(id.contains(kind) || (*kind == "reporter" && id.contains("reporter")), "{id} does not name its kind {kind}");
            for (var, op, value, sep) in *env {
                assert!(["set", "append", "prepend", "unset"].contains(op), "{id}: op {op} on {var}");
                assert!(!var.is_empty() && var.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_'), "{id}: variable {var}");
                if matches!(*op, "append" | "prepend") {
                    assert!(!sep.is_empty(), "{id}: {op} {var} has no separator");
                }
                if *op == "unset" {
                    assert!(value.is_empty() && sep.is_empty(), "{id}: unset {var} carries a value");
                }
            }
            // a kit path in a reader is a kit file
            let paths = env.iter().map(|o| o.2).chain(flags.iter().copied()).flat_map(|s| s.split_whitespace().map(str::to_string).collect::<Vec<_>>());
            for word in paths {
                for piece in word.split('=').chain(std::iter::once(word.as_str())) {
                    if let Some(rest) = piece.strip_prefix("{KIT}/") {
                        assert!(kit.kit_files.iter().any(|f| f.0 == rest), "{id}: {piece} is no file of the kit");
                    }
                }
            }
            if *kind == "reporter" {
                assert!(env.iter().any(|o| o.0 == "FAMILIAR_AI_REPORT" && o.1 == "set"), "{id}: a reporter reader says where its file goes");
            }
            assert!(globs.iter().all(|g| !g.starts_with('/')), "{id}: report globs are relative");
        }
        // best first: reporter, report files, pinned console, tables; the tables always close the list
        let kinds: Vec<&str> = kit.readers.iter().map(|r| r.1).collect();
        let rank = |k: &&str| ["reporter", "report", "pinned", "tables"].iter().position(|x| x == k).unwrap();
        assert!(kinds.windows(2).all(|w| rank(&w[0]) <= rank(&w[1])), "{}: readers are listed best first: {kinds:?}", kit.runner);
        assert_eq!(kinds.last(), Some(&"tables"), "{}: the regex tables close the list", kit.runner);
        assert_eq!(kinds.first(), Some(&"reporter"), "{}: Familiar's own reporter is first", kit.runner);
    }
}

#[test]
fn every_reader_unsets_the_agent_mode_variables() {
    for kit in kits() {
        for (id, _, env, _, _) in kit.readers {
            for var in AGENT_MODE {
                assert!(env.iter().any(|o| o.0 == *var && o.1 == "unset"), "{id} leaves {var} set: the laptop and CI would print differently");
            }
        }
    }
}

#[test]
fn selection_flags_and_watched_files_are_named() {
    let has = |kit: &Kit, f: &str| kit.selection.iter().any(|s| s.0 == f);
    let watches = |kit: &Kit, f: &str| kit.watched.contains(&f);
    let all = kits();
    let by = |name: &str| all.iter().find(|k| k.runner == name).unwrap();
    for f in ["-t", "--testNamePattern", "--testPathPattern", "--findRelatedTests", "-o", "--changedSince"] {
        assert!(has(by("jest"), f), "jest drops {f}");
    }
    for f in ["-t", "--testNamePattern", "related", "--changed"] {
        assert!(has(by("vitest"), f), "vitest drops {f}");
    }
    for f in ["--grep", "-g", "--fgrep", "--spec"] {
        assert!(has(by("mocha"), f), "mocha drops {f}");
    }
    for f in ["--test-name-pattern", "--test-only"] {
        assert!(has(by("node-test"), f), "node drops {f}");
    }
    // a flag that takes a value drops it too; the booleans do not
    assert!(by("jest").selection.contains(&("-t", true)) && by("jest").selection.contains(&("-o", false)));
    assert!(by("node-test").selection.contains(&("--test-only", false)));
    for (kit, files) in [
        ("jest", &["package.json", "jest.config.*"][..]),
        ("vitest", &["package.json", "vitest.config.*", "vite.config.*"][..]),
        ("mocha", &["package.json", ".mocharc*"][..]),
        ("node-test", &["package.json", ".nvmrc"][..]),
    ] {
        for f in files {
            assert!(watches(by(kit), f) || watches(by(kit), &format!("**/{f}")), "{kit} watches {f}");
        }
        assert!(by(kit).watched.iter().any(|w| w.contains("lock")), "{kit} watches a lockfile: a version change changes a reading");
    }
}

#[test]
fn node_defaults_to_tap_before_23_and_spec_after() {
    for (major, want) in [(18, "tap"), (20, "tap"), (22, "tap"), (23, "spec"), (24, "spec"), (26, "spec")] {
        assert_eq!(node::default_reporter(major), want, "node {major}");
    }
}

// ---- the kits for real: node is on this machine, the others' packages are not ----

fn node_major() -> Option<u32> {
    let out = Command::new("node").arg("--version").output().ok()?;
    String::from_utf8(out.stdout).ok()?.trim().trim_start_matches('v').split('.').next()?.parse().ok()
}

fn scratch(name: &str) -> PathBuf {
    let d = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("akjs-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// The reader's env ops applied to an empty environment with a PATH, placeholders filled.
fn apply(ops: &[EnvOp], kit: &Path, report_dir: &Path, default: &str) -> Vec<(String, String)> {
    let fill = |s: &str| s.replace("{KIT}", &kit.to_string_lossy()).replace("{REPORT_DIR}", &report_dir.to_string_lossy()).replace("{NODE_DEFAULT_REPORTER}", default);
    let mut env: Vec<(String, String)> = vec![("PATH".into(), std::env::var("PATH").unwrap_or_default()), ("HOME".into(), std::env::var("HOME").unwrap_or_default())];
    for (var, op, value, sep) in ops {
        let at = env.iter().position(|(k, _)| k == var);
        match (*op, at) {
            ("set", Some(i)) => env[i].1 = fill(value),
            ("set", None) => env.push((var.to_string(), fill(value))),
            ("append", Some(i)) => env[i].1 = format!("{}{sep}{}", env[i].1, fill(value)),
            ("append", None) => env.push((var.to_string(), fill(value))),
            ("prepend", Some(i)) => env[i].1 = format!("{}{sep}{}", fill(value), env[i].1),
            ("prepend", None) => env.push((var.to_string(), fill(value))),
            ("unset", Some(i)) => {
                env.remove(i);
            }
            _ => {}
        }
    }
    env
}

fn write_all(base: &Path, files: &[(&str, &str)]) {
    for (path, text) in files {
        let p = base.join(path);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, text).unwrap();
    }
}

#[test]
fn the_node_kit_reads_every_probe_right_when_run_for_real() {
    let Some(major) = node_major().filter(|m| *m >= 20) else {
        eprintln!("skipped: no node 20 or later on PATH");
        return;
    };
    let reader = node::READERS.iter().find(|r| r.0 == "node:reporter").unwrap();
    let default = node::default_reporter(major);
    let tmp = scratch("live");
    write_all(&tmp.join("kit"), node::KIT_FILES);
    for (probe, label, kind, files) in node::PROBES {
        let work = tmp.join(format!("work-{probe}"));
        let rep = tmp.join(format!("rep-{probe}"));
        std::fs::create_dir_all(&rep).unwrap();
        write_all(&work, files);
        let mut env = apply(reader.2, &tmp.join("kit"), &rep, default);
        env.push(("FAMILIAR_AI_REPORT".into(), rep.join("results.jsonl").to_string_lossy().into_owned()));
        let out = Command::new("node").args(["--test", files[0].0]).current_dir(&work).env_clear().envs(env).output().expect("node runs");
        let code = out.status.code().expect("an exit code");
        let report = std::fs::read_to_string(rep.join("results.jsonl")).unwrap_or_default();
        let got = read(&report, Some(code));
        assert_eq!(verdict(&got, (label, kind)), Verdict::Right, "node {major} {probe}: read {} ({}) from\n{report}", got.spelled(), got.why);
        // the person's console is node's own default for this version
        let console = String::from_utf8_lossy(&out.stdout);
        let want = if default == "tap" { "TAP version" } else { "▶" };
        assert!(console.contains(want) || *probe == "P4", "node {major} {probe}: the console is {default}: {console}");
    }
    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn the_node_kit_never_raises_and_never_prints() {
    let Some(major) = node_major().filter(|m| *m >= 20) else {
        eprintln!("skipped: no node 20 or later on PATH");
        return;
    };
    let reader = node::READERS.iter().find(|r| r.0 == "node:reporter").unwrap();
    let tmp = scratch("quiet");
    write_all(&tmp.join("kit"), node::KIT_FILES);
    let (_, _, _, files) = node::PROBES.iter().find(|p| p.0 == "P1").unwrap();
    let work = tmp.join("work");
    write_all(&work, files);
    let rep = tmp.join("rep");
    std::fs::create_dir_all(&rep).unwrap();
    let plain = Command::new("node").args(["--test", files[0].0]).current_dir(&work).env_clear().envs(vec![("PATH", std::env::var("PATH").unwrap_or_default())]).output().unwrap();
    // a destination that cannot be written, and none at all
    for dest in [Some(tmp.join("no/such/dir/results.jsonl")), None] {
        let mut env = apply(reader.2, &tmp.join("kit"), &rep, node::default_reporter(major));
        if let Some(d) = dest {
            env.push(("FAMILIAR_AI_REPORT".into(), d.to_string_lossy().into_owned()));
        }
        let out = Command::new("node").args(["--test", files[0].0]).current_dir(&work).env_clear().envs(env).output().unwrap();
        assert_eq!(out.status.code(), plain.status.code(), "the exit is the person's own");
        assert_eq!(out.stderr, plain.stderr, "nothing of the kit on stderr");
        let strip = |b: &[u8]| String::from_utf8_lossy(b).lines().filter(|l| !l.contains("duration_ms")).collect::<Vec<_>>().join("\n");
        assert_eq!(strip(&out.stdout), strip(&plain.stdout), "nothing of the kit on stdout");
    }
    let _ = std::fs::remove_dir_all(&tmp);
}

/// jest, vitest and mocha are not packages of this repository. With `FAMILIAR_AI_JS_MODULES` naming a
/// `node_modules` that holds them, each runner runs every probe as its `<runner>:reporter` reader says
/// (the flags added to the command, the env ops applied) and the kit's file reads the probe's truth.
#[cfg(unix)]
#[test]
fn the_other_runners_read_every_probe_right_when_their_packages_are_installed() {
    let Some(modules) = std::env::var_os("FAMILIAR_AI_JS_MODULES").map(PathBuf::from).filter(|p| p.is_dir()) else {
        eprintln!("skipped: FAMILIAR_AI_JS_MODULES names no node_modules with jest, vitest and mocha");
        return;
    };
    let tmp = scratch("others");
    let mut right = 0;
    for kit in kits().iter().filter(|k| k.runner != "node-test") {
        let (script, base): (&str, &[&str]) = match kit.runner {
            "jest" => ("jest/bin/jest.js", &[]),
            "vitest" => ("vitest/vitest.mjs", &["run"]),
            _ => ("mocha/bin/mocha.js", &[]),
        };
        let reader = kit.readers.iter().find(|r| r.0 == format!("{}:reporter", kit.runner)).unwrap();
        let kit_at = tmp.join(format!("kit-{}", kit.runner));
        write_all(&kit_at, kit.kit_files);
        for (probe, label, kind, files) in kit.probes {
            let work = tmp.join(format!("work-{}-{probe}", kit.runner));
            let rep = tmp.join(format!("rep-{}-{probe}", kit.runner));
            std::fs::create_dir_all(&rep).unwrap();
            write_all(&work, files);
            // the person's project: a manifest that names nothing, so no module type is assumed
            std::fs::write(work.join("package.json"), "{}\n").unwrap();
            std::os::unix::fs::symlink(&modules, work.join("node_modules")).unwrap();
            let fill = |s: &str| s.replace("{KIT}", &kit_at.to_string_lossy()).replace("{REPORT_DIR}", &rep.to_string_lossy());
            let mut env = apply(reader.2, &kit_at, &rep, "");
            env.push(("TMPDIR".into(), tmp.to_string_lossy().into_owned()));
            let args: Vec<String> = std::iter::once(modules.join(script).to_string_lossy().into_owned())
                .chain(base.iter().map(|s| s.to_string()))
                .chain(std::iter::once(files[0].0.to_string()))
                .chain(reader.3.iter().map(|f| fill(f)))
                .collect();
            let out = Command::new("node").args(&args).current_dir(&work).env_clear().envs(env).output().expect("node runs");
            let code = out.status.code().expect("an exit code");
            let report = std::fs::read_to_string(rep.join("results.jsonl")).unwrap_or_default();
            let got = read(&report, Some(code));
            assert_eq!(
                verdict(&got, (label, kind)),
                Verdict::Right,
                "{} {probe}: read {} ({}) from\n{report}\nstdout:\n{}\nstderr:\n{}",
                kit.runner,
                got.spelled(),
                got.why,
                String::from_utf8_lossy(&out.stdout),
                String::from_utf8_lossy(&out.stderr)
            );
            right += 1;
        }
    }
    assert_eq!(right, 24);
    let _ = std::fs::remove_dir_all(&tmp);
}

/// node reports a failing hook as `failureType: 'hookFailed'` (the spike's kit looked for
/// `'hookFailure'`, which node never writes): the test whose `beforeEach` threw is a setup error,
/// not an assertion or an exception at some line.
#[test]
fn a_hook_that_throws_in_node_is_a_setup_error() {
    let Some(major) = node_major().filter(|m| *m >= 20) else {
        eprintln!("skipped: no node 20 or later on PATH");
        return;
    };
    let reader = node::READERS.iter().find(|r| r.0 == "node:reporter").unwrap();
    let tmp = scratch("hook");
    write_all(&tmp.join("kit"), node::KIT_FILES);
    let work = tmp.join("work");
    write_all(
        &work,
        &[(
            "hooked.test.cjs",
            "'use strict';\nconst { test, beforeEach } = require('node:test');\nbeforeEach(() => {\n  throw new Error('no database');\n});\ntest('reads', () => {});\n",
        )],
    );
    let rep = tmp.join("rep");
    std::fs::create_dir_all(&rep).unwrap();
    let mut env = apply(reader.2, &tmp.join("kit"), &rep, node::default_reporter(major));
    env.push(("FAMILIAR_AI_REPORT".into(), rep.join("results.jsonl").to_string_lossy().into_owned()));
    let out = Command::new("node").args(["--test", "hooked.test.cjs"]).current_dir(&work).env_clear().envs(env).output().unwrap();
    let report = std::fs::read_to_string(rep.join("results.jsonl")).unwrap_or_default();
    let got = read(&report, out.status.code());
    assert_eq!(got.label, "setup_error", "read {} ({}) from\n{report}", got.spelled(), got.why);
    let _ = std::fs::remove_dir_all(&tmp);
}

/// What the person's terminal shows must not change because a kit is added: the same exit and the
/// same console, numbers aside, with the report's destination unwritable or not named at all.
#[cfg(unix)]
#[test]
fn the_other_kits_never_raise_and_never_print_when_their_packages_are_installed() {
    let Some(modules) = std::env::var_os("FAMILIAR_AI_JS_MODULES").map(PathBuf::from).filter(|p| p.is_dir()) else {
        eprintln!("skipped: FAMILIAR_AI_JS_MODULES names no node_modules with jest, vitest and mocha");
        return;
    };
    let digits = regex::Regex::new(r"[0-9][0-9.,]*").unwrap();
    let timings = regex::Regex::new(r"\((?:[a-z]+ N%(?:, )?)+\)").unwrap();
    let units = regex::Regex::new(r"N ?(?:ms|s)\b").unwrap();
    let tmp = scratch("quiet-others");
    for kit in kits().iter().filter(|k| k.runner != "node-test") {
        let (script, base): (&str, &[&str]) = match kit.runner {
            "jest" => ("jest/bin/jest.js", &[]),
            "vitest" => ("vitest/vitest.mjs", &["run"]),
            _ => ("mocha/bin/mocha.js", &[]),
        };
        let reader = kit.readers.iter().find(|r| r.0 == format!("{}:reporter", kit.runner)).unwrap();
        let kit_at = tmp.join(format!("kit-{}", kit.runner));
        write_all(&kit_at, kit.kit_files);
        // a failing probe: the console has the most to say
        let (_, _, _, files) = kit.probes.iter().find(|p| p.0 == "P2").unwrap();
        let work = tmp.join(format!("work-{}", kit.runner));
        write_all(&work, files);
        std::fs::write(work.join("package.json"), "{}\n").unwrap();
        std::os::unix::fs::symlink(&modules, work.join("node_modules")).unwrap();
        let rep = tmp.join(format!("rep-{}", kit.runner));
        std::fs::create_dir_all(&rep).unwrap();
        let fill = |s: &str| s.replace("{KIT}", &kit_at.to_string_lossy()).replace("{REPORT_DIR}", &rep.to_string_lossy());
        let run = |with_kit: bool, dest: Option<PathBuf>| {
            let mut args: Vec<String> = vec![modules.join(script).to_string_lossy().into_owned()];
            args.extend(base.iter().map(|s| s.to_string()));
            args.push(files[0].0.to_string());
            let mut env = vec![("PATH".to_string(), std::env::var("PATH").unwrap_or_default()), ("TMPDIR".to_string(), tmp.to_string_lossy().into_owned())];
            if with_kit {
                args.extend(reader.3.iter().map(|f| fill(f)));
                env.extend(apply(reader.2, &kit_at, &rep, "").into_iter().filter(|(k, _)| k != "PATH" && k != "HOME" && k != "FAMILIAR_AI_REPORT"));
                if let Some(d) = dest {
                    env.push(("FAMILIAR_AI_REPORT".into(), d.to_string_lossy().into_owned()));
                }
            }
            let out = Command::new("node").args(&args).current_dir(&work).env_clear().envs(env).output().expect("node runs");
            // jest adds ", estimated N s" once an earlier run left its timing in the cache; vitest lists
            // where the time went in the order of the time: (transform N%, import N%, tests N%, …)
            // and prints a duration in ms or in s by how long it took
            let text = |b: &[u8]| {
                let counted = digits.replace_all(&String::from_utf8_lossy(b), "N").replace(", estimated N s", "");
                let timed = timings.replace_all(&counted, "(timings)");
                units.replace_all(&timed, "N time").into_owned()
            };
            (out.status.code(), text(&out.stdout), text(&out.stderr))
        };
        let person = run(false, None);
        assert_eq!(person.0, Some(1), "{}: the probe fails", kit.runner);
        for dest in [Some(tmp.join("no/such/dir/results.jsonl")), None] {
            let with = run(true, dest.clone());
            assert_eq!(with, person, "{}: the kit changed what the person sees (report to {dest:?})", kit.runner);
        }
    }
    let _ = std::fs::remove_dir_all(&tmp);
}

/// A hook that throws in a suite fails the suite, and node reports the suite's own failure
/// (`type: 'suite'`, `failureType: 'hookFailed'`); its tests are only cancelled. The spike's kit skipped
/// every suite event, so the setup error was never said.
#[test]
fn a_hook_that_throws_in_a_node_suite_is_a_setup_error() {
    let Some(major) = node_major().filter(|m| *m >= 20) else {
        eprintln!("skipped: no node 20 or later on PATH");
        return;
    };
    let reader = node::READERS.iter().find(|r| r.0 == "node:reporter").unwrap();
    let tmp = scratch("suite-hook");
    write_all(&tmp.join("kit"), node::KIT_FILES);
    let work = tmp.join("work");
    write_all(
        &work,
        &[(
            "suite.test.cjs",
            "'use strict';\nconst { describe, it, before } = require('node:test');\ndescribe('suite', () => {\n  before(() => {\n    throw new Error('no database');\n  });\n  it('inner', () => {});\n});\n",
        )],
    );
    let rep = tmp.join("rep");
    std::fs::create_dir_all(&rep).unwrap();
    let mut env = apply(reader.2, &tmp.join("kit"), &rep, node::default_reporter(major));
    env.push(("FAMILIAR_AI_REPORT".into(), rep.join("results.jsonl").to_string_lossy().into_owned()));
    let out = Command::new("node").args(["--test", "suite.test.cjs"]).current_dir(&work).env_clear().envs(env).output().unwrap();
    let report = std::fs::read_to_string(rep.join("results.jsonl")).unwrap_or_default();
    let got = read(&report, out.status.code());
    assert_eq!(got.label, "setup_error", "read {} ({}) from\n{report}", got.spelled(), got.why);
    let _ = std::fs::remove_dir_all(&tmp);
}

/// vitest attaches a `beforeAll` failure to the suite and leaves its tests skipped: with nothing else
/// failing, the kit's file said "one skipped, exit 1", which reads as nothing ran.
#[cfg(unix)]
#[test]
fn a_vitest_suite_whose_hook_throws_is_a_failure_and_not_nothing_ran_when_installed() {
    let Some(modules) = std::env::var_os("FAMILIAR_AI_JS_MODULES").map(PathBuf::from).filter(|p| p.is_dir()) else {
        eprintln!("skipped: FAMILIAR_AI_JS_MODULES names no node_modules with vitest");
        return;
    };
    let tmp = scratch("vitest-hook");
    let kit_at = tmp.join("kit");
    write_all(&kit_at, vitest::KIT_FILES);
    let work = tmp.join("work");
    write_all(
        &work,
        &[(
            "hooked.test.mjs",
            "import { describe, test, expect, beforeAll } from 'vitest';\ndescribe('suite', () => {\n  beforeAll(() => {\n    throw new Error('no database');\n  });\n  test('reads', () => {\n    expect(1).toBe(1);\n  });\n});\n",
        )],
    );
    std::fs::write(work.join("package.json"), "{}\n").unwrap();
    std::os::unix::fs::symlink(&modules, work.join("node_modules")).unwrap();
    let rep = tmp.join("rep");
    std::fs::create_dir_all(&rep).unwrap();
    let reader = vitest::READERS.iter().find(|r| r.0 == "vitest:reporter").unwrap();
    let fill = |s: &str| s.replace("{KIT}", &kit_at.to_string_lossy()).replace("{REPORT_DIR}", &rep.to_string_lossy());
    let mut args: Vec<String> = vec![modules.join("vitest/vitest.mjs").to_string_lossy().into_owned(), "run".into(), "hooked.test.mjs".into()];
    args.extend(reader.3.iter().map(|f| fill(f)));
    let env = apply(reader.2, &kit_at, &rep, "");
    let out = Command::new("node").args(&args).current_dir(&work).env_clear().envs(env).output().unwrap();
    let report = std::fs::read_to_string(rep.join("results.jsonl")).unwrap_or_default();
    let got = read(&report, out.status.code());
    assert_eq!(got.label, "tests_failed", "read {} ({}) from\n{report}", got.spelled(), got.why);
    let _ = std::fs::remove_dir_all(&tmp);
}
