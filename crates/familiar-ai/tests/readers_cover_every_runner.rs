//! Lane A-wire (plan of record 2026-10-19, A-2, A-8): the reader table holds every runner's readers
//! from its kit data module, best first, and a reader applies to Familiar's own run only: its env
//! ops (set, append, prepend, unset, each with its separator) and its flags inserted into the
//! runner's own segment of the command, after the runner and its subcommand. A reader that cannot
//! read a command (the A0 spike's gotchas) is refused, so the dry run never tries it there.

use std::path::Path;

use familiar_ai::door::kits;
use familiar_ai::door::readers::{apply, kit_dir_in, of_runner, reader, Op, READERS};

/// A tuple kit's reader: (id, kind, env ops, flags, report globs).
type Tuple = (&'static str, &'static str, &'static [(&'static str, &'static str, &'static str, &'static str)], &'static [&'static str], &'static [&'static str]);

fn ids(runner: &str) -> Vec<&'static str> {
    of_runner(runner).iter().map(|r| r.id).collect()
}

#[test]
fn every_runner_has_its_kits_readers_best_first() {
    assert_eq!(ids("pytest"), ["pytest:reporter", "pytest:tables"]);
    let tuple = |list: &[Tuple]| list.iter().map(|r| r.0).collect::<Vec<_>>();
    assert_eq!(ids("unittest"), tuple(kits::unittest::READERS));
    assert_eq!(ids("jest"), tuple(kits::jest::READERS));
    assert_eq!(ids("vitest"), tuple(kits::vitest::READERS));
    assert_eq!(ids("mocha"), tuple(kits::mocha::READERS));
    assert_eq!(ids("node-test"), tuple(kits::node::READERS));
    assert_eq!(ids("go-test"), tuple(kits::go_test::READERS));
    assert_eq!(ids("cargo-test"), tuple(kits::cargo_test::READERS));
    assert_eq!(ids("cargo-nextest"), tuple(kits::cargo_nextest::READERS));
    let spec = |list: &[kits::spec::Reader]| list.iter().map(|r| r.id).collect::<Vec<_>>();
    assert_eq!(ids("phpunit"), spec(kits::phpunit::READERS));
    assert_eq!(ids("maven"), spec(kits::maven::READERS));
    assert_eq!(ids("gradle"), spec(kits::gradle::READERS));
    assert_eq!(ids("dotnet"), spec(kits::dotnet::READERS));
    assert_eq!(ids("ctest"), spec(kits::ctest::READERS));
    // every id once; every runner of a calibrated reader is one the run crate names
    let mut all: Vec<&str> = READERS.iter().map(|r| r.id).collect();
    let n = all.len();
    all.sort();
    all.dedup();
    assert_eq!(all.len(), n, "an id twice");
    for r in READERS.iter().filter(|r| r.calibrated) {
        assert!(familiar_ai_run::RUNNER_DETAILS.iter().any(|d| d.runner == r.runner), "{} names no runner the run crate knows", r.id);
    }
}

#[test]
fn the_cpp_readers_are_report_readers_marked_not_calibrated() {
    for id in ["ctest:report", "gtest:report", "catch2:report"] {
        let r = reader(id).unwrap_or_else(|| panic!("{id}"));
        assert_eq!((r.kind, r.converter, r.calibrated), ("report", "junit", false), "{id}");
    }
    for r in READERS.iter().filter(|r| ["ctest", "gtest", "catch2"].contains(&r.runner)) {
        assert!(!r.calibrated, "{}: C/C++ has no probe kit", r.id);
    }
}

#[test]
fn every_reader_names_how_its_run_is_read() {
    for r in READERS.iter() {
        let want: &[&str] = match r.kind {
            "tables" => &[""],
            "pinned" => &["", "tap"],
            "reporter" => &["schema", "test2json"],
            _ => &["libtest_json", "junit", "trx"],
        };
        assert!(want.contains(&r.converter), "{}: {} reads by {:?}", r.id, r.kind, r.converter);
    }
    assert_eq!(reader("go-test:reporter").unwrap().converter, "test2json");
    assert_eq!(reader("cargo-nextest:report").unwrap().converter, "libtest_json");
    assert_eq!(reader("node:pinned").unwrap().converter, "tap");
    assert_eq!(reader("maven:report").unwrap().converter, "junit");
    assert_eq!(reader("dotnet:trx").unwrap().converter, "trx");
}

const KIT: &str = "/k";
const REP: &str = "/r";

fn applied(id: &str, command: &str) -> (Vec<Op>, String) {
    let a = apply(id, command, Path::new(KIT), Path::new(REP)).unwrap_or_else(|e| panic!("{id} on {command}: {e}"));
    (a.ops, a.command)
}

fn set(n: &str, v: &str) -> Op {
    Op::Set(n.into(), v.into())
}

#[test]
fn a_js_reporter_adds_its_flags_at_the_end_of_its_segment_and_unsets_the_agent_mode() {
    let (ops, command) = applied("jest:reporter", "cd web && npx jest --ci src/a.test.js");
    assert_eq!(command, "cd web && npx jest --ci src/a.test.js --reporters=default --reporters=/k/jest/reporter.cjs", "at the end: a list option swallows the files after it");
    assert_eq!(ops[0], set("FAMILIAR_AI_REPORT", "/r/results.jsonl"));
    assert!(ops.contains(&Op::Unset("CLAUDECODE".into())) && ops.contains(&Op::Unset("AI_AGENT".into())), "{ops:?}");
    let (_, command) = applied("vitest:reporter", "npx vitest run");
    assert_eq!(command, "npx vitest run --reporter=default --reporter=/k/vitest/reporter.mjs");
    let (_, command) = applied("mocha:reporter", "npx mocha 'test/**/*.spec.js'");
    assert_eq!(command, "npx mocha 'test/**/*.spec.js' --require /k/mocha/load.cjs --reporter /k/mocha/reporter.cjs", "mocha needs --require to see a file that fails to load");
    let (ops, command) = applied("jest:tables", "npx jest");
    assert_eq!(command, "npx jest", "the tables add no flag");
    assert!(ops.iter().all(|o| matches!(o, Op::Unset(_))), "the tables only unset the agent mode: {ops:?}");
}

#[test]
fn node_takes_its_reporter_by_env_unless_the_command_names_its_own() {
    let (ops, command) = applied("node:reporter", "node --test");
    assert_eq!(command, "node --test", "the env route leaves the command alone");
    let options = ops.iter().find_map(|o| match o {
        Op::Append(n, v) if n == "NODE_OPTIONS" => Some(v.clone()),
        _ => None,
    });
    let options = options.expect("NODE_OPTIONS appended, the person's own kept");
    assert!(options.starts_with("--test-reporter=/k/node/reporter.mjs --test-reporter-destination=/r/node.out --test-reporter="), "{options}");
    assert!(options.ends_with(" --test-reporter-destination=stdout") && !options.contains("{NODE_DEFAULT_REPORTER}"), "{options}");
    // the person's own reporter: the env form would pair ours with the wrong destination
    assert!(apply("node:reporter", "node --test --test-reporter=dot", Path::new(KIT), Path::new(REP)).is_err());
    assert!(apply("node:reporter-flags", "node --test", Path::new(KIT), Path::new(REP)).is_err(), "only for a command naming its own reporter");
    let (_, command) = applied("node:reporter-flags", "node --test --test-reporter dot test/");
    assert_eq!(command, "node --test-reporter-destination=stdout --test-reporter=/k/node/reporter.mjs --test-reporter-destination=/r/node.out --test --test-reporter dot test/");
}

#[test]
fn go_takes_json_by_goflags_only_when_go_test_is_the_only_go_call() {
    let (ops, command) = applied("go-test:reporter", "go test ./...");
    assert_eq!((ops, command.as_str()), (vec![Op::Append("GOFLAGS".into(), "-json".into())], "go test ./..."));
    for c in ["go vet ./... && go test ./...", "go test ./... | tee out.log", "go build ./cmd && go test ./..."] {
        assert!(apply("go-test:reporter", c, Path::new(KIT), Path::new(REP)).is_err(), "{c}: GOFLAGS would reach another go call");
    }
    let (ops, command) = applied("go-test:reporter-flag", "go vet ./... && go test -race ./...");
    assert!(ops.is_empty());
    assert_eq!(command, "go vet ./... && go test -json -race ./...", "-json inserted in the go test part only");
}

#[test]
fn nextest_needs_both_of_its_variables_and_yields_to_the_persons_format() {
    let (ops, command) = applied("cargo-nextest:report", "cargo nextest run --workspace");
    assert_eq!(ops, [set("NEXTEST_EXPERIMENTAL_LIBTEST_JSON", "1"), set("NEXTEST_MESSAGE_FORMAT", "libtest-json-plus")]);
    assert_eq!(command, "cargo nextest run --workspace");
    assert!(apply("cargo-nextest:report", "cargo nextest run --message-format human", Path::new(KIT), Path::new(REP)).is_err());
    assert!(apply("cargo-nextest:report", "cargo nextest run --message-format=human", Path::new(KIT), Path::new(REP)).is_err());
}

#[test]
fn unittest_rides_on_pythonpath_and_is_refused_where_python_ignores_it() {
    let (ops, command) = applied("unittest:reporter", "python -m unittest -v");
    assert_eq!(command, "python -m unittest -v");
    assert_eq!(ops, [Op::Prepend("PYTHONPATH".into(), "/k/unittest".into()), set("FAMILIAR_AI_REPORT", "/r/results.jsonl")]);
    for c in ["python -I -m unittest", "python3 -E -m unittest discover"] {
        assert!(apply("unittest:reporter", c, Path::new(KIT), Path::new(REP)).is_err(), "{c}");
    }
}

#[test]
fn phpunit_loads_its_extension_and_the_report_readers_yield_to_the_persons_own_file() {
    let (ops, command) = applied("phpunit:reporter", "vendor/bin/phpunit --testdox");
    assert_eq!(command, r"vendor/bin/phpunit --testdox --extension 'Familiar\PhpunitExtension'");
    assert!(ops.contains(&Op::Unset("PHPUNIT_COMPACT_OUTPUT".into())), "{ops:?}");
    assert!(ops.contains(&set("FAMILIAR_AI_REPORT", "/r/results.jsonl")), "the extension is told where to write: {ops:?}");
    let scan = ops.iter().find_map(|o| match o {
        Op::Set(n, v) if n == "PHP_INI_SCAN_DIR" => Some(v.clone()),
        _ => None,
    });
    assert!(scan.as_deref().is_some_and(|s| s.ends_with(":/k/phpunit/ini")), "glued on as written: {scan:?}");
    let (_, command) = applied("phpunit:report", "vendor/bin/phpunit");
    assert_eq!(command, "vendor/bin/phpunit --log-junit /r/junit.xml");
    assert!(apply("phpunit:report", "vendor/bin/phpunit --log-junit out.xml", Path::new(KIT), Path::new(REP)).is_err());
}

#[test]
fn dotnet_logs_trx_by_env_and_by_flag_beside_the_persons_own_logger() {
    let (ops, command) = applied("dotnet:trx", "dotnet test Calc.sln");
    assert_eq!(command, "dotnet test Calc.sln");
    assert!(ops.contains(&set("VSTestLogger", "trx%3BLogFileName=/r/results.trx")), "the ; escaped as %3B: {ops:?}");
    assert!(apply("dotnet:trx", "dotnet test --logger console", Path::new(KIT), Path::new(REP)).is_err(), "a command-line logger beats the env one");
    let (_, command) = applied("dotnet:trx-flag", "dotnet test --logger console");
    assert_eq!(command, "dotnet test --logger console --logger 'trx;LogFileName=/r/results.trx'", "quoted for the shell");
    let (ops, _) = applied("maven:report", "mvn -q test");
    assert_eq!(ops, [Op::Append("MAVEN_ARGS".into(), "-B".into())]);
}

#[test]
fn a_reader_of_another_runner_or_none_is_an_error() {
    assert!(apply("jest:reporter", "go test ./...", Path::new(KIT), Path::new(REP)).is_err());
    assert!(apply("no:such", "pytest", Path::new(KIT), Path::new(REP)).is_err());
}

#[test]
fn the_kit_holds_every_runners_files_and_fills_its_templates() {
    let base = std::env::temp_dir().join(format!("familiar-ai-kit-every-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let dir = kit_dir_in(&base).expect("written");
    for f in ["pytest/familiar_ai_pytest.py", "unittest/sitecustomize.py", "node/common.cjs", "node/reporter.mjs", "jest/reporter.cjs", "vitest/reporter.mjs", "mocha/load.cjs", "mocha/reporter.cjs", "phpunit/ext.php", "phpunit/prepend.php", "phpunit/ini/zz-familiar-ai.ini"] {
        assert!(dir.join(f).is_file(), "{f}");
    }
    let ini = std::fs::read_to_string(dir.join("phpunit/ini/zz-familiar-ai.ini")).unwrap();
    assert!(ini.contains(&format!("{}/phpunit/prepend.php", dir.display())) && !ini.contains("{KIT}"), "{ini}");
    let _ = std::fs::remove_dir_all(&base);
}
