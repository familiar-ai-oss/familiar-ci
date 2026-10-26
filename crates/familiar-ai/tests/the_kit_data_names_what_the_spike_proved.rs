//! Lane AKreports (plan of record 2026-10-19, A section: readers, kits, probes). Each runner of
//! this lane has a data module under `door/kits/` holding, as plain data, what the A0 spike proved:
//! the kit files, the readers (id, kind, env operations, added flags, report globs, converter),
//! the selection flags `re_aim` must drop, the setup files that make a choice stale, and the
//! probe kit P1-P7 with its truths. Lane A-py's reader table and dry run read these constants.
//!
//! ```text
//!   kits/spec.rs      the shapes: Reader, EnvOp, KitFile, Probe, Truth, PLACEHOLDERS
//!   kits/maven.rs     maven:report   maven:pinned   maven:tables      probes P1-P7 (JUnit 5)
//!   kits/gradle.rs    gradle:report                 gradle:tables     the same Java probes
//!   kits/dotnet.rs    dotnet:trx  dotnet:trx-flag   dotnet:tables     probes P1-P7 (xunit)
//!   kits/phpunit.rs   phpunit:reporter  :report  :pinned  :tables     kit files; probes P1-P7
//!   kits/gtest.rs     gtest:report                  C/C++: report readers only,
//!   kits/catch2.rs    catch2:report                 no probe kit (not calibrated)
//!   kits/ctest.rs     ctest:report
//! ```

mod probe_runs_support;

use familiar_ai::door::kits::spec::{Kind, KitFile, Op, Probe, Reader, PLACEHOLDERS, TRUTHS};
use familiar_ai::door::kits::{catch2, ctest, dotnet, gradle, gtest, maven, phpunit};
use probe_runs_support::truth;

struct R {
    id: &'static str,
    readers: &'static [Reader],
    kit_files: &'static [KitFile],
    drop: &'static [&'static str],
    watched: &'static [&'static str],
    probes: &'static [Probe],
}

fn all() -> Vec<R> {
    vec![
        R { id: maven::ID, readers: maven::READERS, kit_files: maven::KIT_FILES, drop: maven::DROP_SELECTION, watched: maven::WATCHED, probes: maven::PROBES },
        R { id: gradle::ID, readers: gradle::READERS, kit_files: gradle::KIT_FILES, drop: gradle::DROP_SELECTION, watched: gradle::WATCHED, probes: gradle::PROBES },
        R { id: dotnet::ID, readers: dotnet::READERS, kit_files: dotnet::KIT_FILES, drop: dotnet::DROP_SELECTION, watched: dotnet::WATCHED, probes: dotnet::PROBES },
        R { id: phpunit::ID, readers: phpunit::READERS, kit_files: phpunit::KIT_FILES, drop: phpunit::DROP_SELECTION, watched: phpunit::WATCHED, probes: phpunit::PROBES },
        R { id: gtest::ID, readers: gtest::READERS, kit_files: gtest::KIT_FILES, drop: gtest::DROP_SELECTION, watched: gtest::WATCHED, probes: gtest::PROBES },
        R { id: catch2::ID, readers: catch2::READERS, kit_files: catch2::KIT_FILES, drop: catch2::DROP_SELECTION, watched: catch2::WATCHED, probes: catch2::PROBES },
        R { id: ctest::ID, readers: ctest::READERS, kit_files: ctest::KIT_FILES, drop: ctest::DROP_SELECTION, watched: ctest::WATCHED, probes: ctest::PROBES },
    ]
}

fn placeholders_in(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = s;
    while let Some(at) = rest.find('{') {
        let Some(end) = rest[at..].find('}') else { break };
        out.push(rest[at..at + end + 1].to_string());
        rest = &rest[at + end + 1..];
    }
    out
}

#[test]
fn every_reader_is_named_by_its_runner_and_well_formed() {
    let mut seen = std::collections::BTreeSet::new();
    for r in all() {
        assert!(!r.readers.is_empty(), "{} has readers", r.id);
        for reader in r.readers {
            assert!(reader.id.starts_with(&format!("{}:", r.id)), "{} is named for its runner {}", reader.id, r.id);
            assert_eq!(reader.runner, r.id, "{}", reader.id);
            assert!(seen.insert(reader.id), "{} is unique", reader.id);
            for op in reader.env {
                assert!(!op.var.is_empty() && op.var.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'), "{}: {}", reader.id, op.var);
                match op.op {
                    Op::Unset => assert!(op.value.is_empty() && op.separator.is_empty(), "{}: an unset names no value", reader.id),
                    Op::Set => assert!(!op.value.is_empty(), "{}: {} is set to something", reader.id, op.var),
                    Op::Append | Op::Prepend => assert!(!op.value.is_empty(), "{}: {} gets something", reader.id, op.var),
                }
            }
            match reader.kind {
                Kind::Reporter => assert_eq!(reader.converter, "schema", "{}: a reporter's kit writes the result file itself", reader.id),
                Kind::Report => {
                    assert!(!reader.report_globs.is_empty(), "{}: a report reader names the files it reads", reader.id);
                    assert!(["junit", "trx"].contains(&reader.converter), "{}: {}", reader.id, reader.converter);
                }
                Kind::Pinned => assert!(reader.converter.is_empty() && reader.report_globs.is_empty(), "{}: pinned console, read by the tables", reader.id),
                Kind::Tables => assert!(reader.env.is_empty() && reader.flags.is_empty() && reader.report_globs.is_empty() && reader.converter.is_empty(), "{}: the tables change nothing", reader.id),
            }
            for text in reader.env.iter().flat_map(|o| [o.value]).chain(reader.flags.iter().copied()).chain(reader.report_globs.iter().copied()) {
                for p in placeholders_in(text) {
                    assert!(PLACEHOLDERS.contains(&p.as_str()), "{}: {p} is not a placeholder of the spec", reader.id);
                }
                assert!(!text.contains('"') && !text.contains('\''), "{}: argv words and values carry no shell quoting: {text}", reader.id);
            }
        }
    }
    // maven 3, gradle 2, dotnet 3, phpunit 4, gtest 2, catch2 2, ctest 2
    assert_eq!(seen.len(), 18, "the readers of the seven runners: {seen:?}");
}

#[test]
fn only_the_phpunit_reader_needs_a_kit_the_others_read_files_the_runner_writes() {
    for r in all() {
        assert_eq!(r.kit_files.is_empty(), r.id != "phpunit", "{}", r.id);
    }
}

#[test]
fn the_last_reader_of_each_runner_is_the_tables_which_change_nothing() {
    for r in all() {
        let last = r.readers.last().unwrap();
        assert_eq!(last.kind, Kind::Tables, "{}", r.id);
        assert!(r.readers.iter().filter(|x| x.kind == Kind::Tables).count() == 1, "{}", r.id);
    }
}

#[test]
fn the_probe_truths_are_the_spikes() {
    assert_eq!(TRUTHS.len(), 8);
    for (id, t) in TRUTHS {
        let (label, kind) = truth(id);
        assert_eq!((t.label, t.kind), (label, kind), "{id}");
    }
}

#[test]
fn each_probed_runner_carries_p1_to_p7_with_their_truths_in_two_stages() {
    for r in all() {
        if r.probes.is_empty() {
            continue;
        }
        let ids: Vec<&str> = r.probes.iter().map(|p| p.id).collect();
        assert_eq!(ids, ["P1", "P2", "P3", "P4", "P5", "P6", "P3b", "P7"], "{}", r.id);
        for p in r.probes {
            let (label, kind) = truth(p.id);
            assert_eq!((p.truth.label, p.truth.kind), (label, kind), "{} {}", r.id, p.id);
            assert_eq!(p.stage, if p.id == "P3b" || p.id == "P7" { 2 } else { 1 }, "{} {}", r.id, p.id);
            assert!(!p.files.is_empty() && p.files.iter().all(|f| !f.path.is_empty() && !f.text.trim().is_empty() && !f.path.starts_with('/') && !f.path.contains("..")), "{} {} has relative, non-empty files", r.id, p.id);
        }
    }
}

/// A probe's text says what its truth is: the tokens that make it so are in the files.
#[test]
fn the_probe_files_hold_what_makes_each_truth_true() {
    // (runner, test-file marker of a test, a skip, a missing name, a broken import, an assertion that fails, a call that throws)
    let tokens: [(&str, &str, &str, &str, &str, &str, &str); 4] = [
        ("maven", "@Test", "@Disabled", "Calc.mul(", "import nosuch.Helper;", "assertEquals(5, Calc.add(2, 2))", "Calc.parse(\"x1\")"),
        ("gradle", "@Test", "@Disabled", "Calc.mul(", "import nosuch.Helper;", "assertEquals(5, Calc.add(2, 2))", "Calc.parse(\"x1\")"),
        ("dotnet", "[Fact]", "Skip =", "Calc.Mul(", "using Nosuch.Helpers;", "Assert.Equal(5, Calc.Add(2, 2))", "Calc.Parse(\"x1\")"),
        ("phpunit", "public function test", "markTestSkipped", "Calc::mul(", "NosuchHelper.php", "assertSame(5, Calc::add(2, 2))", "Calc::parse('x1')"),
    ];
    let runners = all();
    for (id, test, skip, missing, import, assertion, throws) in tokens {
        let r = runners.iter().find(|r| r.id == id).unwrap();
        let by = |p: &str| r.probes.iter().find(|x| x.id == p).unwrap();
        let text = |p: &str| by(p).files.iter().map(|f| f.text).collect::<Vec<_>>().join("\n");
        let has = |p: &str, t: &str| text(p).contains(t);
        for p in ["P1", "P2", "P3", "P4", "P5", "P3b", "P7"] {
            assert!(has(p, test), "{id} {p} holds a test");
        }
        assert!(!has("P6", test), "{id} P6 holds no test: a runner with nothing to run");
        assert!(has("P5", skip), "{id} P5 skips one");
        assert!(has("P3", missing) && has("P3b", missing), "{id} P3 and P3b ask for the missing name");
        let code = |p: &str| by(p).files.iter().filter(|f| !f.path.to_lowercase().contains("test")).map(|f| f.text).collect::<Vec<_>>().join("\n");
        let lower = |s: String| s.to_lowercase();
        assert!(!lower(code("P3")).contains("mul"), "{id} P3's code lacks the function");
        assert!(lower(code("P3b")).contains("mul"), "{id} P3b's code has it");
        assert!(has("P4", import), "{id} P4 imports what is not there");
        assert!(has("P2", assertion), "{id} P2 asserts what is false");
        assert!(has("P7", throws), "{id} P7 calls what throws");
        assert!(!has("P1", "nosuch") && !has("P1", "Nosuch"), "{id} P1 is plain");
    }
}

#[test]
fn the_java_runners_share_one_probe_kit_and_both_read_surefire_style_reports() {
    assert_eq!(maven::PROBES.len(), gradle::PROBES.len());
    for (a, b) in maven::PROBES.iter().zip(gradle::PROBES.iter()) {
        assert_eq!(a.id, b.id);
        assert_eq!(a.files.iter().map(|f| (f.path, f.text)).collect::<Vec<_>>(), b.files.iter().map(|f| (f.path, f.text)).collect::<Vec<_>>());
    }
}

#[test]
fn what_the_plan_says_each_reader_does() {
    let reader = |id: &str| all().into_iter().flat_map(|r| r.readers.iter()).find(|x| x.id == id).unwrap_or_else(|| panic!("no reader {id}"));
    let env = |r: &Reader, var: &str| r.env.iter().find(|o| o.var == var).copied();

    let m = reader("maven:report");
    assert_eq!(m.report_globs, ["target/surefire-reports/TEST-*.xml", "target/failsafe-reports/TEST-*.xml"]);
    let b = env(m, "MAVEN_ARGS").expect("MAVEN_ARGS");
    assert_eq!((b.op, b.value, b.separator), (Op::Append, "-B", " "));
    assert_eq!(m.converter, "junit");

    let g = reader("gradle:report");
    assert_eq!(g.report_globs, ["build/test-results/**/*.xml"]);
    assert_eq!(g.converter, "junit");

    let d = reader("dotnet:trx");
    assert_eq!(env(d, "VSTestLogger").map(|o| (o.op, o.value)), Some((Op::Set, "trx%3BLogFileName={REPORT_DIR}/results.trx")), "the ; is escaped as %3B");
    assert_eq!(env(d, "VSTestResultsDirectory").map(|o| (o.op, o.value)), Some((Op::Set, "{REPORT_DIR}")));
    for (var, value) in [("DOTNET_CLI_UI_LANGUAGE", "en"), ("MSBUILDTERMINALLOGGER", "off"), ("DOTNET_NOLOGO", "1")] {
        assert_eq!(env(d, var).map(|o| (o.op, o.value)), Some((Op::Set, value)), "{var}");
    }
    assert_eq!(d.skip_when_command_has, ["--logger", "-l"], "the person's own logger beats the env property");
    assert_eq!(d.converter, "trx");
    let f = reader("dotnet:trx-flag");
    assert_eq!(f.flags, ["--logger", "trx;LogFileName={REPORT_DIR}/results.trx"]);
    assert_eq!(f.only_when_command_has, ["--logger", "-l"]);
    assert!(f.env.iter().any(|o| o.var == "DOTNET_CLI_UI_LANGUAGE"), "the pinned console is the same");

    let p = reader("phpunit:reporter");
    let ini = env(p, "PHP_INI_SCAN_DIR").expect("PHP_INI_SCAN_DIR");
    assert_eq!((ini.op, ini.value, ini.separator), (Op::Append, ":{KIT}/phpunit/ini", ""), "a leading : keeps PHP's own scan dir; a person's own value is kept");
    assert_eq!(p.flags, ["--extension", "Familiar\\PhpunitExtension"]);
    assert_eq!(env(p, "PHPUNIT_COMPACT_OUTPUT").map(|o| o.op), Some(Op::Unset));
    let j = reader("phpunit:report");
    assert_eq!(j.flags, ["--log-junit", "{REPORT_DIR}/junit.xml"]);
    assert_eq!(j.skip_when_command_has, ["--log-junit"], "a second --log-junit would replace the person's own");
    assert_eq!(j.converter, "junit");
    assert_eq!(env(j, "PHPUNIT_COMPACT_OUTPUT").map(|o| o.op), Some(Op::Unset));

    let t = reader("gtest:report");
    assert_eq!(env(t, "GTEST_OUTPUT").map(|o| (o.op, o.value)), Some((Op::Set, "xml:{REPORT_DIR}/")));
    assert_eq!(env(t, "GTEST_BRIEF").map(|o| o.op), Some(Op::Unset));
    let c = reader("catch2:report");
    assert_eq!(env(c, "BAZEL_TEST").map(|o| (o.op, o.value)), Some((Op::Set, "1")));
    assert_eq!(env(c, "XML_OUTPUT_FILE").map(|o| o.op), Some(Op::Set));
    let ct = reader("ctest:report");
    assert_eq!(ct.flags[0], "--output-junit");
    assert_eq!(ct.converter, "junit");
}

#[test]
fn the_selection_flags_re_aim_drops_are_the_plans() {
    let by = |id: &str| all().into_iter().find(|r| r.id == id).unwrap().drop;
    assert_eq!(by("maven"), ["-Dtest"]);
    assert_eq!(by("gradle"), ["--tests"]);
    assert_eq!(by("dotnet"), ["--filter"]);
    assert_eq!(by("phpunit"), ["--filter", "--group"]);
    assert_eq!(by("ctest"), ["-R", "-E", "-L"]);
    assert_eq!(by("gtest"), ["--gtest_filter"]);
    assert!(by("catch2").is_empty(), "Catch2 selects by positional test specs, which re_aim drops with the targets");
}

#[test]
fn the_watched_setup_files_are_the_plans_for_each_runner() {
    let by = |id: &str| all().into_iter().find(|r| r.id == id).unwrap().watched;
    let has = |w: &[&str], names: &[&str]| names.iter().all(|n| w.contains(n));
    assert!(has(by("maven"), &["pom.xml", "**/pom.xml"]));
    assert!(has(by("gradle"), &["build.gradle", "build.gradle.kts", "**/build.gradle", "**/build.gradle.kts"]));
    assert!(has(by("dotnet"), &["*.csproj", "**/*.csproj", "global.json", "Directory.*.props", "**/Directory.*.props", "*.runsettings", "**/*.runsettings"]));
    assert!(has(by("phpunit"), &["composer.json", "composer.lock", "phpunit.xml", "**/phpunit.xml"]));
    for c in ["gtest", "catch2", "ctest"] {
        assert!(has(by(c), &["CMakeLists.txt", "**/CMakeLists.txt"]), "{c}");
    }
}

#[test]
fn the_cpp_runners_are_report_readers_only_and_say_they_are_not_calibrated() {
    for (id, readers, probes, calibrated, note) in [
        (gtest::ID, gtest::READERS, gtest::PROBES, gtest::CALIBRATED, gtest::NOT_CALIBRATED),
        (catch2::ID, catch2::READERS, catch2::PROBES, catch2::CALIBRATED, catch2::NOT_CALIBRATED),
        (ctest::ID, ctest::READERS, ctest::PROBES, ctest::CALIBRATED, ctest::NOT_CALIBRATED),
    ] {
        assert!(probes.is_empty(), "{id} has no probe kit");
        assert!(!calibrated, "{id}");
        assert!(note.contains("not calibrated"), "{id}: {note}");
        assert!(readers.iter().all(|r| matches!(r.kind, Kind::Report | Kind::Tables)), "{id}: report readers only");
    }
    for id in ["maven", "gradle", "dotnet", "phpunit"] {
        let r = all().into_iter().find(|r| r.id == id).unwrap();
        assert!(!r.probes.is_empty(), "{id} is calibrated by a probe kit");
    }
}

#[test]
fn a_report_reader_never_names_the_missing_name_or_collection_probes_as_its_own() {
    // compile errors write no report: the JVM and .NET report readers leave P3 and P4 to the tables
    for id in ["maven", "gradle", "dotnet"] {
        let r = all().into_iter().find(|r| r.id == id).unwrap();
        for reader in r.readers.iter().filter(|x| x.kind == Kind::Report) {
            assert!(reader.cannot_read.contains(&"P3") && reader.cannot_read.contains(&"P4"), "{}: {:?}", reader.id, reader.cannot_read);
        }
    }
    let maven_report = maven::READERS.iter().find(|r| r.id == "maven:report").unwrap();
    assert!(maven_report.cannot_read.contains(&"P6"), "surefire writes no XML for a class with no tests");
}

#[test]
fn the_php_kit_files_are_the_extension_the_prepend_and_the_ini() {
    let paths: Vec<&str> = phpunit::KIT_FILES.iter().map(|f| f.path).collect();
    assert_eq!(paths, ["phpunit/ext.php", "phpunit/prepend.php", "phpunit/ini/zz-familiar-ai.ini"]);
    let ini = phpunit::KIT_FILES.iter().find(|f| f.path.ends_with(".ini")).unwrap();
    assert!(ini.template, "the ini names the kit's folder, which is only known when it is written");
    assert!(ini.text.contains("auto_prepend_file={KIT}/phpunit/prepend.php"));
    let rendered = phpunit::rendered("/tmp/familiar-ai-kit-9.9.9");
    let (path, text) = rendered.iter().find(|(p, _)| p.ends_with(".ini")).unwrap();
    assert_eq!(*path, "phpunit/ini/zz-familiar-ai.ini");
    assert!(text.contains("auto_prepend_file=/tmp/familiar-ai-kit-9.9.9/phpunit/prepend.php") && !text.contains("{KIT}"));
    // only the template is rewritten
    for (p, t) in &rendered {
        if !p.ends_with(".ini") {
            assert_eq!(*t, phpunit::KIT_FILES.iter().find(|f| f.path == *p).unwrap().text);
        }
    }
}

/// "Kits never print to the console and never raise": no output call anywhere in the PHP kit,
/// every entry point PHPUnit or PHP calls (bootstrap, trace, shutdown, the autoloader) catches
/// Throwable, and the prepend does nothing when the report path is not set.
#[test]
fn the_php_kit_never_prints_and_never_raises() {
    let ext = phpunit::KIT_FILES.iter().find(|f| f.path == "phpunit/ext.php").unwrap().text;
    let prepend = phpunit::KIT_FILES.iter().find(|f| f.path == "phpunit/prepend.php").unwrap().text;
    for (name, text) in [("ext.php", ext), ("prepend.php", prepend)] {
        for banned in ["echo ", "print ", "print(", "printf(", "var_dump(", "print_r(", "var_export(", "STDOUT", "STDERR", "php://stdout", "php://stderr", "php://output", "error_log(", "trigger_error(", "exit(", "exit;", "die(", "ini_set("] {
            assert!(!text.contains(banned), "{name} must not contain {banned}");
        }
        assert!(!text.contains("throw "), "{name} never throws");
    }
    // the three callbacks PHPUnit and PHP call, and the shutdown handler, all catch
    for entry in ["public function bootstrap(", "public function trace(", "public function shutdown("] {
        let at = ext.find(entry).unwrap_or_else(|| panic!("{entry}"));
        let body = &ext[at..];
        let end = body[1..].find("    public function ").or_else(|| body[1..].find("    private function ")).map_or(body.len(), |e| e + 1);
        assert!(body[..end].contains("catch (\\Throwable"), "{entry} catches Throwable");
    }
    assert!(prepend.contains("getenv('FAMILIAR_AI_REPORT') === false"), "the prepend does nothing unless the report path is set");
    assert!(prepend.contains("return;"));
    // an end line is only written when the run finished or the reason is known (a bare end read as "nothing ran")
    assert!(ext.contains("ExecutionFinished") && ext.contains("aborted_while_loading_tests"));
}

#[test]
fn the_php_kit_writes_to_the_report_path_and_names_itself() {
    let ext = phpunit::KIT_FILES.iter().find(|f| f.path == "phpunit/ext.php").unwrap().text;
    assert!(ext.contains("getenv('FAMILIAR_AI_REPORT')"));
    assert!(ext.contains("namespace Familiar;") && ext.contains("final class PhpunitExtension"));
    assert!(!phpunit::KIT_FILES.iter().find(|f| f.path == "phpunit/prepend.php").unwrap().text.contains("FAMILIAR_AI_PHPUNIT_AUTO"), "the intrusive env-only mode is not shipped");
}
