//! H-2 under dash (Ubuntu's and Debian's `/bin/sh`; hosted CI, 2026-10-26): the place's own
//! script failing is an exception at its line whether the test script runs it (a shell of its
//! own, which dash starts with no `-x`) or sources it (dash names no file in its trace). Runs only
//! where a dash is installed; with none (Fedora's CI) it says so and passes. One test in this file:
//! the `sh` the run starts is read from PATH once per process, and this file's PATH puts a dash
//! first.

use std::path::PathBuf;
use std::time::Duration;

use familiar_ai::door::kept::{classify, clean, Streams};
use familiar_ai_facts::LocationKind;
use familiar_ai_run::{details_for, run_test_command_traced};

fn a_dash() -> Option<PathBuf> {
    let path = std::env::var_os("PATH").unwrap_or_default();
    std::env::split_paths(&path).map(|p| p.join("dash")).chain([PathBuf::from("/usr/bin/dash"), PathBuf::from("/bin/dash")]).find(|p| p.is_file())
}

#[test]
fn the_places_own_script_failing_under_dash_is_an_exception_at_its_line() {
    let Some(dash) = a_dash() else {
        eprintln!("no dash on this machine: skipped");
        return;
    };
    let root = std::env::temp_dir().join(format!("familiar-ai-h5-dash-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let bin = root.join("bin");
    std::fs::create_dir_all(&bin).expect("bin");
    std::os::unix::fs::symlink(&dash, bin.join("sh")).expect("sh is dash");
    let path = std::env::var_os("PATH").unwrap_or_default();
    std::env::set_var("PATH", std::env::join_paths(std::iter::once(bin.clone()).chain(std::env::split_paths(&path))).expect("PATH"));
    for (name, app, test) in [
        ("runs", "#!/bin/sh\ngreet() { echo hi; }\nexit 3\n", "sh src/app.sh\n"),
        ("sources", "#!/bin/sh\ngreet() {\n  exit 3\n}\n", ". src/app.sh\ngreet\n"),
    ] {
        let d = root.join(name);
        std::fs::create_dir_all(d.join("src")).expect("src");
        std::fs::create_dir_all(d.join("tests")).expect("tests");
        std::fs::write(d.join("src/app.sh"), app).expect("app");
        std::fs::write(d.join("tests/test_c1.sh"), test).expect("test");
        let cmd = "sh tests/test_c1.sh";
        let (out, trace) = run_test_command_traced(cmd, &d, Duration::from_secs(30), None, &[], details_for(cmd));
        let (stdout, stderr, report) = (clean(&out.stdout), clean(&out.stderr), clean(&trace));
        let t = classify(out.end, &Streams { stdout: &stdout, stderr: &stderr, report: &report }, details_for(cmd)).expect("a reading");
        assert_eq!((t.label.as_str(), t.location.map(|l| (l.file_path, l.line, l.kind))), ("tests_failed", Some(("src/app.sh".to_string(), 3, Some(LocationKind::Exception)))), "{name}: {report}");
    }
    let _ = std::fs::remove_dir_all(&root);
}
