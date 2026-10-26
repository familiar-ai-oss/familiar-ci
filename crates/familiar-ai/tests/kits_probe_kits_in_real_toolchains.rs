//! The probe kits of go test and cargo/nextest (plan A, lane akgorust) run for real: each probe is
//! written into a throwaway host project (a Cargo package with an empty lib, a Go module), run as
//! the dry run will run it (`cargo nextest run --test familiar_probe_<k>` with the reader's env, a
//! package path for go with GOFLAGS=-json), and the converter's output read through the plan's
//! reading must be the probe's truth or "cannot tell", never anything else. Ignored by default: it
//! needs cargo-nextest, and docker with a golang image, and takes a minute.
//!
//! cargo test --test kits_probe_kits_in_real_toolchains -- --ignored

#[path = "common/gorust_probe_runs.rs"]
mod gorust_probe_runs;

use std::path::{Path, PathBuf};
use std::process::Command;

use familiar_ai::door::convert::{libtest_json, test2json};
use familiar_ai::door::kits::{cargo_nextest, cargo_test, go_test};
use gorust_probe_runs::{lines, of_type, read_through_schema, verdict, PROBES};

const R: &str = "right";
const C: &str = "cant_tell";

/// A throwaway directory of this run, removed by the caller.
fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("familiar-ai-probe-kit-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn write(dir: &Path, files: &[(&str, &str)]) {
    for (path, text) in files {
        let at = dir.join(path);
        std::fs::create_dir_all(at.parent().unwrap()).unwrap();
        std::fs::write(at, text).unwrap();
    }
}

#[test]
#[ignore = "runs cargo and cargo-nextest, about a minute: cargo test --test kits_probe_kits_in_real_toolchains -- --ignored"]
fn the_rust_probe_kit_reads_its_truth_in_real_nextest_and_cargo() {
    // P3 and P4 do not build and P6 has nothing to run: nextest prints no JSON for them
    let want = [R, R, C, R, C, R, C, R];
    for (probe, want) in PROBES.iter().zip(want) {
        let (id, _label, _kind, files) = cargo_test::PROBES.iter().find(|p| p.0 == *probe).unwrap();
        let skipped: &[&str] = if *id == "P5" { cargo_test::SKIPPED } else { &[] };
        let dir = scratch(&format!("rust-{id}"));
        write(&dir, &[("Cargo.toml", "[package]\nname = \"host\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[dependencies]\n"), ("src/lib.rs", "")]);
        write(&dir, files);
        let target = format!("familiar_probe_{}", id.to_lowercase());

        let mut nextest = Command::new("cargo");
        nextest.args(["nextest", "run", "--test", &target]).current_dir(&dir).env("CARGO_TARGET_DIR", dir.join("target"));
        for (var, op, value, _sep) in cargo_nextest::READERS[0].2 {
            assert_eq!(*op, "set");
            nextest.env(var, value);
        }
        let out = nextest.output().expect("cargo nextest");
        let (stdout, exit) = (String::from_utf8_lossy(&out.stdout).to_string(), out.status.code().unwrap_or(-1));
        let report = libtest_json::to_report(&stdout, exit);
        let got = read_through_schema(&report, exit);
        assert_eq!(verdict(&got, id), want, "nextest {id}: read {got:?}, exit {exit}\n{stdout}\n{}\n{report}", String::from_utf8_lossy(&out.stderr));
        let all = if report.is_empty() { vec![] } else { lines(&report) };
        let tests = of_type(&all, "test");
        for name in skipped {
            assert!(tests.iter().any(|t| t["outcome"] == "skipped" && t["id"].as_str().unwrap().ends_with(&format!("${name}"))), "{id}: {name} reads skipped: {report}");
        }
        if *id == "P2" {
            assert_eq!(tests[0]["error"]["kind"], "assertion", "{report}");
            assert_eq!(tests[0]["error"]["frames"][0]["file"], format!("tests/{target}.rs"), "{report}");
        }
        if *id == "P7" {
            // the panic is inside the probe's own function parse, lines 1 to 6 of the file
            assert_eq!(tests[0]["error"]["kind"], "exception", "{report}");
            let line = tests[0]["error"]["frames"][0]["line"].as_u64().unwrap();
            assert!((1..=6).contains(&line), "{id}: line {line}: {report}");
            assert_eq!(tests[0]["error"]["frames"][0]["file"], format!("tests/{target}.rs"), "{report}");
        }

        // and plain cargo test, whose console the tables read: only its exit code is checked here
        let plain = Command::new("cargo").args(["test", "--test", &target]).current_dir(&dir).env("CARGO_TARGET_DIR", dir.join("target")).output().expect("cargo test");
        let expect = if matches!(*id, "P2" | "P3" | "P4" | "P7") { 101 } else { 0 };
        assert_eq!(plain.status.code(), Some(expect), "cargo test {id}: {}", String::from_utf8_lossy(&plain.stderr));
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[test]
#[ignore = "runs go in docker, about a minute: cargo test --test kits_probe_kits_in_real_toolchains -- --ignored"]
fn the_go_probe_kit_reads_its_truth_in_a_real_go() {
    let id_of = |flag: &str| String::from_utf8(Command::new("id").arg(flag).output().unwrap().stdout).unwrap().trim().to_string();
    let user = format!("{}:{}", id_of("-u"), id_of("-g"));
    // go 1.23 prints a build failure's words on stderr and a setup failure as plain text: P3 and P4
    // cannot be told from stdout; go 1.24 on prints them as build events inside the JSON
    for (image, want) in [("golang:1.23-bookworm", [R, R, C, R, C, R, R, R]), ("golang:1.25-bookworm", [R; 8])] {
        if !Command::new("docker").args(["image", "inspect", image]).output().map(|o| o.status.success()).unwrap_or(false) {
            eprintln!("no docker image {image}: not run");
            continue;
        }
        for (probe, want) in PROBES.iter().zip(want) {
            let (id, _label, _kind, files) = go_test::PROBES.iter().find(|p| p.0 == *probe).unwrap();
            let skipped: &[&str] = if *id == "P5" { go_test::SKIPPED } else { &[] };
            let dir = scratch(&format!("go-{id}"));
            write(&dir, &[("go.mod", "module example.com/host\n\ngo 1.23\n")]);
            write(&dir, files);
            let package = format!("./familiar_probe_{}/...", id.to_lowercase());
            let (var, op, value, sep) = go_test::READERS[0].2[0];
            assert_eq!((op, sep), ("append", " "));
            let out = Command::new("docker")
                .args(["run", "--rm", "-u", &user, "-e", "HOME=/tmp", "-e", "GOCACHE=/tmp/gocache", "-e", "GOPATH=/tmp/go", "-e", "GOTOOLCHAIN=local", "-e", &format!("{var}={value}")])
                .args(["-v", &format!("{0}:{0}", dir.display()), "-w", &dir.display().to_string(), image, "sh", "-c", &format!("go test {package}")])
                .output()
                .expect("docker run");
            let (stdout, exit) = (String::from_utf8_lossy(&out.stdout).to_string(), out.status.code().unwrap_or(-1));
            let report = test2json::to_report(&stdout, exit);
            let got = read_through_schema(&report, exit);
            assert_eq!(verdict(&got, id), want, "{image} {id}: read {got:?}, exit {exit}\n{stdout}\n{}\n{report}", String::from_utf8_lossy(&out.stderr));
            let all = if report.is_empty() { vec![] } else { lines(&report) };
            let tests = of_type(&all, "test");
            for name in skipped {
                assert!(tests.iter().any(|t| t["outcome"] == "skipped" && t["id"].as_str().unwrap().ends_with(&format!(".{name}"))), "{image} {id}: {name} reads skipped: {report}");
            }
            if *id == "P7" {
                // the panic is inside Parse, in probe.go, the deepest project frame
                let frames = tests[0]["error"]["frames"].as_array().unwrap();
                assert!(frames.last().unwrap()["file"].as_str().unwrap().ends_with("probe.go"), "{image}: {report}");
            }
            let _ = std::fs::remove_dir_all(&dir);
        }
    }
}
