//! Lane A-wire (plan of record 2026-10-19, A: the dry run for every runner with a probe kit). Each
//! runner's probes (P1–P7 from its kit data) are placed at its test location, the claimed command
//! is re-aimed at each with the runner's own selection flags dropped, every probe is read as a real
//! run is read (the reader's result file, else the tables), and the choice follows the plan's rule.
//! C/C++ has report readers but no probe kit: no dry run is owed for it.
//!
//! Fake `jest` and `go` stand in for the runners where the machine has none: they replay the A0
//! spike's recordings of the probe they are aimed at, the reporter's file only when the reader's
//! injection reached them. node is on this machine: its dry run is run for real; jest, vitest and
//! mocha for real when `FAMILIAR_AI_JS_MODULES` names a `node_modules` holding them.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use familiar_ai::door::dry_run::{choose_or_kit_failure, dry_run, owed, probes_for, re_aim_for, runner_for, watched_by, watched_for, Verdict};
use familiar_ai::door::verify_run::Place;
use familiar_ai_facts::{append_raw, Log};
use serde_json::{json, Value};

#[test]
fn each_runners_command_is_re_aimed_at_the_probe_with_its_selection_dropped() {
    let one = |t: &str| vec![t.to_string()];
    let cases: &[(&str, &str, Vec<String>, &str)] = &[
        ("jest", "cd web && npx jest --ci -t 'adds two' --config jest.config.js src/a.test.js", one("src/familiar_probe_p1.test.js"), "cd web && npx jest --ci --config jest.config.js src/familiar_probe_p1.test.js"),
        ("vitest", "npx vitest run --changed -t slow src/", one("src/familiar_probe_p1.test.js"), "npx vitest run src/familiar_probe_p1.test.js"),
        ("mocha", "npx mocha --grep fast --require ts-node/register 'test/**/*.js'", one("test/familiar_probe_p1.test.cjs"), "npx mocha --require ts-node/register test/familiar_probe_p1.test.cjs"),
        ("node-test", "node --test --test-name-pattern=adds test/", one("test/familiar_probe_p1.test.cjs"), "node --test test/familiar_probe_p1.test.cjs"),
        ("go-test", "go test -race -run TestAdd -count 1 ./...", one("./familiar_probe_p1"), "go test -race -count 1 ./familiar_probe_p1"),
        ("cargo-test", "cargo test --workspace --lib parse -- --nocapture --exact", vec!["--test".into(), "familiar_probe_p1".into()], "cargo test --workspace --test familiar_probe_p1 -- --nocapture"),
        ("cargo-nextest", "cargo nextest run -E 'test(parse)' --profile ci", vec!["--test".into(), "familiar_probe_p1".into()], "cargo nextest run --profile ci --test familiar_probe_p1"),
        ("unittest", "python -m unittest discover -s tests -p 'test_*.py' -v", one("tests/test_familiar_probe_p1.py"), "python -m unittest -v tests/test_familiar_probe_p1.py"),
        ("phpunit", "vendor/bin/phpunit --filter testAdd tests/CalcTest.php", one("tests/ProbeTest.php"), "vendor/bin/phpunit tests/ProbeTest.php"),
        ("maven", "mvn -q -Dtest=CalcTest#adds test", vec!["-Dtest=ProbeTest".into()], "mvn -q test -Dtest=ProbeTest"),
        ("gradle", "./gradlew test --tests 'calc.CalcTest'", vec!["--tests".into(), "calc.ProbeTest".into()], "./gradlew test --tests calc.ProbeTest"),
        ("dotnet", "dotnet test Calc.sln --filter Category=Fast", vec!["--filter".into(), "Name~Probe".into()], "dotnet test Calc.sln --filter Name~Probe"),
    ];
    for (runner, command, targets, want) in cases {
        assert_eq!(re_aim_for(runner, command, targets).as_deref(), Some(*want), "{runner}: {command}");
    }
    assert_eq!(re_aim_for("jest", "make test", &one("x")), None, "a command naming no runner gets no dry run");
}

#[test]
fn each_runner_with_a_probe_kit_has_its_eight_probes_and_cpp_has_none() {
    for runner in ["pytest", "unittest", "jest", "vitest", "mocha", "node-test", "go-test", "cargo-test", "cargo-nextest", "phpunit", "maven", "gradle", "dotnet"] {
        let p = probes_for(runner, "spec");
        let names: Vec<&str> = p.iter().map(|p| p.name).collect();
        let mut sorted = names.clone();
        sorted.sort();
        assert_eq!(sorted, ["P1", "P2", "P3", "P3b", "P4", "P5", "P6", "P7"], "{runner}");
        for probe in &p {
            assert!(!probe.files.is_empty() && !probe.target.is_empty(), "{runner} {}", probe.name);
            assert_eq!(probe.stage, if matches!(probe.name, "P3b" | "P7") { 2 } else { 1 }, "{runner} {}", probe.name);
            let truth = match probe.name {
                "P2" => ("tests_failed", Some("assertion")),
                "P7" => ("tests_failed", Some("exception")),
                "P3" => ("missing_target_symbol", None),
                // csc's CS0246 is a missing name (the owner, 2026-10-14)
                "P4" if runner == "dotnet" => ("missing_target_symbol", None),
                "P4" => ("collection_error", None),
                "P6" => ("no_tests_collected", None),
                _ => ("passed", None),
            };
            assert_eq!((probe.truth, probe.kind), truth, "{runner} {}", probe.name);
        }
    }
    // path-named probes go in the planned test's folder; packages and crates at the project's
    let jest = probes_for("jest", "web/src");
    assert!(jest[0].files.iter().all(|(p, _)| p.starts_with("web/src/")), "{:?}", jest[0].files);
    assert_eq!(jest[0].target, ["web/src/familiar_probe_p1.test.js"]);
    let go = probes_for("go-test", "ignored");
    assert_eq!(go[0].target, ["./familiar_probe_p1"]);
    let cargo = probes_for("cargo-nextest", "ignored");
    assert_eq!(cargo[0].target, ["--test", "familiar_probe_p1"]);
    assert_eq!(cargo[0].files[0].0, "tests/familiar_probe_p1.rs");
    for runner in ["ctest", "gtest", "catch2"] {
        assert!(probes_for(runner, "tests").is_empty(), "{runner}: not calibrated");
    }
}

fn add(log: &Log, kind: &str, f: Value) -> Log {
    let at = log.len() as f64 + 1.0;
    append_raw(log, kind, f.as_object().cloned().unwrap_or_default(), at, "t").expect(kind)
}

fn stack(name: &str, command: &str) -> Log {
    add(&Vec::new(), "test_stack_detected", json!({"codebase_id": "x", "project_root": ".", "test_stack": name, "test_command": command}))
}

#[test]
fn a_dry_run_is_owed_to_every_runner_with_a_probe_kit_claimed_as_itself() {
    for (name, command) in [("node_jest", "npx jest"), ("vitest", "npx vitest run"), ("mocha", "npx mocha"), ("node", "node --test"), ("go", "go test ./..."), ("rust", "cargo test"), ("rust_nextest", "cargo nextest run"), ("python_unittest", "python -m unittest"), ("php", "vendor/bin/phpunit"), ("java_maven", "mvn test"), ("gradle", "./gradlew test"), ("dotnet", "dotnet test")] {
        assert_eq!(owed(&stack(name, command)).as_deref(), Some(command), "{name}: {command}");
    }
    assert_eq!(owed(&stack("cpp", "ctest --test-dir build")), None, "C/C++ has no probe kit: not calibrated");
    assert_eq!(owed(&stack("sh", "./jest.sh")), None);
    assert_eq!(owed(&stack("go", "npx jest")), None, "a stack claimed as another runner");
    assert_eq!(runner_for("node_jest", "npx jest --ci"), Some("jest"));
    // a commit changing a file the runner watches owes it again
    let log = stack("go", "go test ./...");
    let log = add(&log, "run_reserved", json!({"run_id": "d-1", "criterion_id": "", "criterion_rev": "", "commit": "K", "command": "go test ./...", "resources": ["tests"], "wait": "60", "by": "dry_run"}));
    assert_eq!(owed(&log), None);
    let seen = |file: &str| json!({"commit": "K2", "files": [file], "touches_tests": false, "touches_code": true, "anchor_overlap": []});
    assert_eq!(owed(&add(&log, "commit_seen", seen("conftest.py"))), None, "not go's");
    assert_eq!(owed(&add(&log, "commit_seen", seen("svc/go.mod"))).as_deref(), Some("go test ./..."));
}

#[test]
fn the_watched_files_match_by_name_at_any_depth_or_by_path() {
    assert!(watched_by(watched_for("jest"), "web/jest.config.ts"));
    assert!(watched_by(watched_for("mocha"), ".mocharc.yml"));
    assert!(watched_by(watched_for("cargo-nextest"), ".config/nextest.toml"));
    assert!(!watched_by(watched_for("cargo-nextest"), "other/nextest.toml"));
    assert!(watched_by(watched_for("dotnet"), "src/App/App.csproj"));
    assert!(!watched_by(watched_for("go-test"), "src/main.go"));
}

#[test]
fn a_kit_that_cannot_run_here_keeps_the_tables_with_every_probe_a_gap() {
    use Verdict::{CantTell as C, Wrong as W};
    let readings = [("go-test:reporter", vec![("P1", W), ("P2", C)]), ("go-test:tables", vec![("P1", W), ("P2", W)])];
    let (reader, gaps, why) = choose_or_kit_failure(&readings, "go-test:tables");
    assert_eq!((reader.as_str(), gaps), ("go-test:tables", vec!["P1".to_string(), "P2".to_string()]));
    assert!(why.contains("probe kit"), "{why}");
}

// ---- dry runs: fakes replaying the spike's recordings, and node for real ----

fn recordings(runner: &str, setup: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/probe_runs").join(runner).join(setup)
}

fn repo(name: &str, files: &[(&str, &str)]) -> (PathBuf, String) {
    let dir = std::env::temp_dir().join(format!("familiar-ai-dry-every-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    for (p, t) in files {
        let at = dir.join(p);
        std::fs::create_dir_all(at.parent().unwrap()).unwrap();
        std::fs::write(&at, t).unwrap();
        if !p.contains('.') {
            let _ = Command::new("chmod").arg("+x").arg(&at).status();
        }
    }
    let git = |args: &[&str]| {
        let _ = Command::new("git").args(args).current_dir(&dir).env("GIT_AUTHOR_NAME", "t").env("GIT_AUTHOR_EMAIL", "t@t").env("GIT_COMMITTER_NAME", "t").env("GIT_COMMITTER_EMAIL", "t@t").status();
    };
    git(&["init", "-q"]);
    git(&["add", "-A"]);
    git(&["commit", "-q", "-m", "files"]);
    let head = familiar_ai_git::head_commit(&dir).ok().flatten().unwrap();
    (dir, head)
}

/// A fake jest: aimed at a probe's test file, it replays that probe's recording; the reporter's
/// file only when Familiar's reporter flag reached it. P3 and P3b share a test file: the probe's
/// code tells them apart.
const FAKE_JEST: &str = r#"#!/bin/sh
REC=__REC__
on=; t=
for a in "$@"; do
  case "$a" in
    --reporters=*/jest/reporter.cjs) on=1 ;;
    *familiar_probe_p*.test.js) t="$a" ;;
  esac
done
[ -n "$t" ] || exit 9
p=$(basename "$t" .test.js); p=${p#familiar_probe_p}
P="P$p"
if [ "$p" = 3 ] && grep -q 'function mul' "$(dirname "$t")/familiar_probe_calc.js"; then P=P3b; fi
[ -n "$on" ] && cp "$REC/$P/report/results.jsonl" "$FAMILIAR_AI_REPORT"
cat "$REC/$P/stdout"; cat "$REC/$P/stderr" >&2
exit $(cat "$REC/$P/exit")
"#;

#[test]
fn a_jest_project_chooses_familiars_reporter_injected_by_its_flags() {
    let fake = FAKE_JEST.replace("__REC__", &recordings("jest", "plain").to_string_lossy());
    let (dir, head) = repo("fake-jest", &[("jest", &fake), ("package.json", "{}\n"), ("src/a.test.js", "test('a', () => {});\n")]);
    let p = Place { worktree: &dir, root: &dir, head: &head, wait: Duration::from_secs(60) };
    let chosen = dry_run(&p, "d-j", "./jest --ci src/a.test.js", &head, &["src/a.test.js".to_string()], &mut |_| true).expect("the dry run ran");
    assert_eq!((chosen.runner.as_str(), chosen.reader.as_str()), ("jest", "jest:reporter"), "{}", chosen.why);
    assert!(chosen.gaps.is_empty(), "{:?}", chosen.gaps);
    assert!(chosen.versions.as_deref().is_some_and(|v| v.contains("jest 30.5.2")), "{:?}", chosen.versions);
    assert!(chosen.watched.iter().any(|w| w == "**/jest.config.*"), "{:?}", chosen.watched);
    assert!(!dir.join("src/familiar_probe_p1.test.js").exists(), "the probes went in the copy");
    let _ = std::fs::remove_dir_all(&dir);
}

/// A fake go: `go test ./familiar_probe_p<k>` replays go 1.25's recording with `-json` (by GOFLAGS
/// or the flag), its plain console without.
const FAKE_GO: &str = r#"#!/bin/sh
JSON=__JSON__
PLAIN=__PLAIN__
[ "$1" = test ] || exit 9
json=; case " $GOFLAGS " in *" -json "*) json=1 ;; esac
for a in "$@"; do
  case "$a" in
    -json) json=1 ;;
    ./familiar_probe_p*) p=${a#./familiar_probe_p} ;;
  esac
done
P="P$p"; D=$PLAIN; [ -n "$json" ] && D=$JSON
cat "$D/$P/stdout"; cat "$D/$P/stderr" >&2
exit $(cat "$D/$P/exit")
"#;

#[test]
fn a_go_project_chooses_test2json_by_goflags_and_by_the_flag_when_go_vet_runs_too() {
    let fake = FAKE_GO.replace("__JSON__", &recordings("go-test", "go125").to_string_lossy()).replace("__PLAIN__", &recordings("go-test", "noinject").to_string_lossy());
    let (dir, head) = repo("go", &[("go", &fake), ("go.mod", "module example.com/x\n\ngo 1.25\n")]);
    let p = Place { worktree: &dir, root: &dir, head: &head, wait: Duration::from_secs(60) };
    let chosen = dry_run(&p, "d-g", "./go test ./...", &head, &[], &mut |_| true).expect("the dry run ran");
    assert_eq!((chosen.runner.as_str(), chosen.reader.as_str()), ("go-test", "go-test:reporter"), "{}", chosen.why);
    assert!(chosen.gaps.is_empty(), "{:?}", chosen.gaps);
    // with go vet in the command GOFLAGS would reach it: the flag route
    let chosen = dry_run(&p, "d-g2", "true && ./go test ./... && echo done", &head, &[], &mut |_| true).expect("the dry run ran");
    assert_eq!(chosen.reader, "go-test:reporter", "the only go call: {}", chosen.why);
    let fake_vet = format!("{fake}\n");
    let (dir2, head2) = repo("go-vet", &[("go", &fake_vet.replace("[ \"$1\" = test ] || exit 9", "[ \"$1\" = vet ] && exit 0\n[ \"$1\" = test ] || exit 9")), ("go.mod", "module example.com/x\n")]);
    let p2 = Place { worktree: &dir2, root: &dir2, head: &head2, wait: Duration::from_secs(60) };
    let chosen = dry_run(&p2, "d-g3", "./go vet ./... && ./go test ./...", &head2, &[], &mut |_| true).expect("the dry run ran");
    assert_eq!(chosen.reader, "go-test:reporter-flag", "{}", chosen.why);
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&dir2);
}

fn node_major() -> Option<u32> {
    let out = Command::new("node").arg("--version").output().ok()?;
    String::from_utf8(out.stdout).ok()?.trim().trim_start_matches('v').split('.').next()?.parse().ok()
}

#[test]
fn a_node_test_project_chooses_familiars_reporter_on_real_probes() {
    if node_major().filter(|m| *m >= 20).is_none() {
        eprintln!("skipped: no node 20 or later on PATH");
        return;
    }
    let (dir, head) = repo("node", &[("package.json", "{\"name\": \"x\"}\n"), ("test/a.test.cjs", "const { test } = require('node:test');\ntest('a', () => {});\n")]);
    let p = Place { worktree: &dir, root: &dir, head: &head, wait: Duration::from_secs(120) };
    let chosen = dry_run(&p, "d-n", "node --test test/", &head, &["test/a.test.cjs".to_string()], &mut |_| true).expect("the dry run ran");
    assert_eq!((chosen.runner.as_str(), chosen.reader.as_str()), ("node-test", "node:reporter"), "{}", chosen.why);
    assert!(chosen.gaps.is_empty(), "{:?}", chosen.gaps);
    // the person's own reporter: the flags reader
    let chosen = dry_run(&p, "d-n2", "node --test --test-reporter=dot test/", &head, &["test/a.test.cjs".to_string()], &mut |_| true).expect("the dry run ran");
    assert_eq!(chosen.reader, "node:reporter-flags", "{}", chosen.why);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_unittest_project_chooses_familiars_reporter_on_real_probes() {
    if Command::new("python3").arg("--version").output().is_err() {
        eprintln!("skipped: no python3 on PATH");
        return;
    }
    let (dir, head) = repo("unittest", &[("tests/test_a.py", "import unittest\n\n\nclass A(unittest.TestCase):\n    def test_a(self):\n        pass\n")]);
    let p = Place { worktree: &dir, root: &dir, head: &head, wait: Duration::from_secs(120) };
    let chosen = dry_run(&p, "d-u", "python3 -m unittest discover -s tests -v", &head, &["tests/test_a.py".to_string()], &mut |_| true).expect("the dry run ran");
    assert_eq!((chosen.runner.as_str(), chosen.reader.as_str()), ("unittest", "unittest:reporter"), "{}", chosen.why);
    assert!(chosen.gaps.is_empty(), "{:?}", chosen.gaps);
    // python -I ignores PYTHONPATH: the reporter is never tried there
    let chosen = dry_run(&p, "d-u2", "python3 -I -m unittest discover -s tests", &head, &["tests/test_a.py".to_string()], &mut |_| true).expect("the dry run ran");
    assert_ne!(chosen.reader, "unittest:reporter", "{}", chosen.why);
    let _ = std::fs::remove_dir_all(&dir);
}

#[cfg(unix)]
#[test]
fn the_js_runners_choose_familiars_reporter_on_real_probes_when_installed() {
    let Some(modules) = std::env::var_os("FAMILIAR_AI_JS_MODULES").map(PathBuf::from).filter(|p| p.is_dir()) else {
        eprintln!("skipped: FAMILIAR_AI_JS_MODULES names no node_modules with jest, vitest and mocha");
        return;
    };
    for (runner, command, test) in [("jest", "npx jest", "src/a.test.js"), ("vitest", "npx vitest run", "src/a.test.js"), ("mocha", "npx mocha", "test/a.test.cjs")] {
        let (dir, head) = repo(&format!("real-{runner}"), &[("package.json", "{}\n"), (test, "// the project's own test\n")]);
        std::os::unix::fs::symlink(&modules, dir.join("node_modules")).unwrap();
        let p = Place { worktree: &dir, root: &dir, head: &head, wait: Duration::from_secs(120) };
        let chosen = dry_run(&p, &format!("d-{runner}"), command, &head, &[test.to_string()], &mut |_| true).expect("the dry run ran");
        assert_eq!(chosen.reader, format!("{runner}:reporter"), "{runner}: {}", chosen.why);
        assert!(chosen.gaps.is_empty(), "{runner}: {:?}", chosen.gaps);
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[test]
#[ignore = "builds eight probe crates with cargo-nextest, a few minutes: cargo test --test dry_run_every_runner -- --ignored"]
fn a_nextest_project_chooses_libtest_json_on_real_probes() {
    let (dir, head) = repo("nextest", &[("Cargo.toml", "[package]\nname = \"host\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[dependencies]\n"), ("src/lib.rs", "")]);
    let p = Place { worktree: &dir, root: &dir, head: &head, wait: Duration::from_secs(300) };
    let chosen = dry_run(&p, "d-x", "cargo nextest run", &head, &[], &mut |_| true).expect("the dry run ran");
    assert_eq!(chosen.reader, "cargo-nextest:report", "{}", chosen.why);
    let _ = std::fs::remove_dir_all(&dir);
}

/// go in a container, as the person's `go` on PATH would be: the copy is mounted at its own path,
/// GOFLAGS forwarded, the build cache kept between probes.
const DOCKER_GO: &str = r#"#!/bin/sh
cache="${TMPDIR:-/tmp}/familiar-ai-gocache"; mkdir -p "$cache"
exec docker run --rm -u "$(id -u):$(id -g)" -e HOME=/tmp -e GOFLAGS -e GOCACHE=/gocache -e GOTOOLCHAIN=local -v "$cache":/gocache -v "$PWD":"$PWD" -w "$PWD" golang:1.25-bookworm go "$@"
"#;

#[test]
#[ignore = "runs go 1.25 in docker (golang:1.25-bookworm), a few minutes: cargo test --test dry_run_every_runner -- --ignored"]
fn a_go_project_chooses_test2json_on_real_probes() {
    let (dir, head) = repo("real-go", &[("go", DOCKER_GO), ("go.mod", "module example.com/host\n\ngo 1.25\n")]);
    let p = Place { worktree: &dir, root: &dir, head: &head, wait: Duration::from_secs(300) };
    let chosen = dry_run(&p, "d-rg", "./go test ./...", &head, &[], &mut |_| true).expect("the dry run ran");
    assert_eq!(chosen.reader, "go-test:reporter", "{}", chosen.why);
    assert!(chosen.gaps.is_empty(), "{:?}", chosen.gaps);
    let _ = std::fs::remove_dir_all(&dir);
}

const POM: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<project xmlns="http://maven.apache.org/POM/4.0.0">
  <modelVersion>4.0.0</modelVersion>
  <groupId>com.example</groupId>
  <artifactId>host</artifactId>
  <version>1.0</version>
  <properties>
    <maven.compiler.release>21</maven.compiler.release>
    <project.build.sourceEncoding>UTF-8</project.build.sourceEncoding>
  </properties>
  <dependencies>
    <dependency><groupId>org.junit.jupiter</groupId><artifactId>junit-jupiter</artifactId><version>5.11.4</version><scope>test</scope></dependency>
  </dependencies>
  <build><plugins><plugin><groupId>org.apache.maven.plugins</groupId><artifactId>maven-surefire-plugin</artifactId><version>3.5.2</version></plugin></plugins></build>
</project>
"#;

/// Maven in a container, offline against the local repository `FAMILIAR_AI_M2` names.
const DOCKER_MVN: &str = r#"#!/bin/sh
exec docker run --rm -u "$(id -u):$(id -g)" -e HOME=/tmp -e MAVEN_ARGS -e MAVEN_CONFIG=/tmp/.m2 -e MAVEN_OPTS=-Dmaven.repo.local=/m2 -v "$FAMILIAR_AI_M2":/m2 -v "$PWD":"$PWD" -w "$PWD" maven:3.9-eclipse-temurin-21 mvn -o "$@"
"#;

#[test]
#[ignore = "runs Maven 3.9 in docker offline against FAMILIAR_AI_M2 (a local repository with JUnit 5.11.4 and surefire 3.5.2), minutes"]
fn a_maven_project_chooses_its_surefire_reports_on_real_probes() {
    if std::env::var_os("FAMILIAR_AI_M2").is_none() {
        eprintln!("skipped: FAMILIAR_AI_M2 names no local Maven repository");
        return;
    }
    let (dir, head) = repo("real-mvn", &[("mvn", DOCKER_MVN), ("pom.xml", POM)]);
    let p = Place { worktree: &dir, root: &dir, head: &head, wait: Duration::from_secs(300) };
    let chosen = dry_run(&p, "d-rm", "./mvn -q test", &head, &[], &mut |_| true).expect("the dry run ran");
    eprintln!("maven chose {} gaps {:?}: {}", chosen.reader, chosen.gaps, chosen.why);
    // P3, P4 (compile errors) and P6 (no XML) are the tables' to read: right, or a gap
    assert!(chosen.reader == "maven:report" || chosen.reader == "maven:pinned", "{} {}", chosen.reader, chosen.why);
    assert!(chosen.gaps.iter().all(|g| ["P3", "P4", "P6"].contains(&g.as_str())), "{:?} {}", chosen.gaps, chosen.why);
    let _ = std::fs::remove_dir_all(&dir);
}

const CSPROJ: &str = r#"<Project Sdk="Microsoft.NET.Sdk">
  <PropertyGroup>
    <TargetFramework>net8.0</TargetFramework>
    <ImplicitUsings>enable</ImplicitUsings>
    <Nullable>enable</Nullable>
    <IsPackable>false</IsPackable>
    <IsTestProject>true</IsTestProject>
  </PropertyGroup>
  <ItemGroup>
    <PackageReference Include="Microsoft.NET.Test.Sdk" Version="17.11.1" />
    <PackageReference Include="xunit" Version="2.9.2" />
    <PackageReference Include="xunit.runner.visualstudio" Version="2.8.2" />
  </ItemGroup>
</Project>
"#;

/// dotnet in a container: the temp folder (the copy and the run's report folder) mounted at its own
/// path, the reader's variables forwarded, packages from the cache `FAMILIAR_AI_DOTNET_CACHE` names
/// (its `nuget`, `dotnet-home`, `dotnet-passwd`, `dotnet-group`).
const DOCKER_DOTNET: &str = r#"#!/bin/sh
C="$FAMILIAR_AI_DOTNET_CACHE"; T="${TMPDIR:-/tmp}"
exec docker run --rm -v "$C/dotnet-passwd":/etc/passwd:ro -v "$C/dotnet-group":/etc/group:ro -u "$(id -u):$(id -g)" \
  -e HOME=/tmp -e NUGET_PACKAGES=/nuget -e DOTNET_CLI_HOME=/dotnethome -e DOTNET_NOLOGO=1 -e DOTNET_CLI_TELEMETRY_OPTOUT=1 -e DOTNET_SKIP_FIRST_TIME_EXPERIENCE=1 \
  -e VSTestLogger -e VSTestResultsDirectory -e DOTNET_CLI_UI_LANGUAGE -e MSBUILDTERMINALLOGGER \
  -v "$C/nuget":/nuget -v "$C/dotnet-home":/dotnethome -v "$T":"$T" -v "$PWD":"$PWD" -w "$PWD" mcr.microsoft.com/dotnet/sdk:8.0 dotnet "$@"
"#;

#[test]
#[ignore = "runs the .NET 8 SDK in docker against FAMILIAR_AI_DOTNET_CACHE (xunit 2.9.2 packages), minutes"]
fn a_dotnet_project_chooses_its_trx_on_real_probes() {
    if std::env::var_os("FAMILIAR_AI_DOTNET_CACHE").is_none() {
        eprintln!("skipped: FAMILIAR_AI_DOTNET_CACHE names no package cache");
        return;
    }
    let (dir, head) = repo("real-dotnet", &[("dotnet", DOCKER_DOTNET), ("Calc.Tests.csproj", CSPROJ)]);
    let p = Place { worktree: &dir, root: &dir, head: &head, wait: Duration::from_secs(300) };
    let chosen = dry_run(&p, "d-rd", "./dotnet test", &head, &[], &mut |_| true).expect("the dry run ran");
    eprintln!("dotnet chose {} gaps {:?}: {}", chosen.reader, chosen.gaps, chosen.why);
    assert_eq!(chosen.reader, "dotnet:trx", "{}", chosen.why);
    assert!(chosen.gaps.iter().all(|g| ["P3", "P4", "P6"].contains(&g.as_str())), "{:?} {}", chosen.gaps, chosen.why);
    let _ = std::fs::remove_dir_all(&dir);
}
