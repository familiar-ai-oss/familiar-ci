//! The touching tests have a run of their own (the owner's plan of 2026-10-21, step 0) [draft]:
//! the part's own run is its own test files only, so its counts are what H6 compares with the
//! seal run's; the touching tests, declared and derived, run under a second command with a run
//! id of its own, and each touching result names that run. A skip in a touching file leaves a
//! real pass green; a touching failure keeps the part from green. On a real repository, with a
//! project-local stand-in for pytest that prints pytest's own summary lines, so each run's counts
//! are read as pytest's are.

mod common;

use common::Project;
use serde_json::Value;

const M_AT_BASE: &str = "def greet():\n    return \"hi\"\n\n\ndef shout():\n    return greet().upper()\n";
/// An existing test that imports the fixed module: one test that checks shout, one skipped.
const TEST_M: &str = "import m\n\n\ndef test_shout():\n    assert m.shout() == \"HI\"\n\n\ndef test_on_windows():\n    assert False\n\n\ntest_on_windows.skip = True\n";
const TEST_C1: &str = "import m\n\n\ndef test_greet():\n    assert m.greet() == \"hello\"\n";
/// Stands in for pytest: runs each `test*` function of the files given (one marked `skip` is
/// skipped), with `src` on the path, and prints what pytest prints for that.
const STAND_IN: &str = r#"import importlib.util, os, sys, traceback
sys.path.insert(0, os.path.join(os.getcwd(), "src"))
passed, skipped, fails, progress = 0, 0, [], []
for path in [a for a in sys.argv[1:] if a.endswith(".py")]:
    dots = ""
    spec = importlib.util.spec_from_file_location(os.path.basename(path)[:-3], path)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    for name in [n for n in list(vars(mod)) if n.startswith("test")]:
        fn = getattr(mod, name)
        if getattr(fn, "skip", False):
            skipped += 1
            dots += "s"
            continue
        try:
            fn()
            passed += 1
            dots += "."
        except AssertionError:
            fails.append((path, name, traceback.extract_tb(sys.exc_info()[2])[-1].lineno))
            dots += "F"
    progress.append(path + " " + dots)
print("=" * 29 + " test session starts " + "=" * 30)
print("collected %d items" % (passed + skipped + len(fails)))
print("")
for line in progress:
    print(line.ljust(73) + "[100%]")
print("")
if fails:
    print("=" * 35 + " FAILURES " + "=" * 35)
    for path, name, line in fails:
        print("_" * 35 + " " + name + " " + "_" * 35)
        print("")
        print(">       assert")
        print("E       assert")
        print("")
        print(path + ":" + str(line) + ": AssertionError")
    print("=" * 27 + " short test summary info " + "=" * 28)
    for path, name, line in fails:
        print("FAILED " + path + "::" + name + " - assert")
counts = [(len(fails), "failed"), (passed, "passed"), (skipped, "skipped")]
print("=" * 30 + " " + ", ".join("%d %s" % c for c in counts if c[0]) + " in 0.01s " + "=" * 30)
sys.exit(1 if fails else 0)
"#;

fn project(name: &str) -> Project {
    let p = Project::builder(name)
        .no_app()
        .file("pytest.py", STAND_IN)
        .file("src/m.py", M_AT_BASE)
        .file("tests/test_m.py", TEST_M)
        .create();
    p.claim(&["intent_submitted", "--for", "0", "--text", "greet with hello"]);
    p.claim(&["behavior", "--behavior_id", "b1", "--text", "the greeting"]);
    p.claim(&["promise", "--promise_id", "p1", "--behavior_id", "b1", "--accepts", "nothing", "--gives_back", "a greeting", "--refuses", "nothing", "--cases", r#"[{"case_id":"n1","kind":"normal","given":"a name","expect":"a greeting"},{"case_id":"o1","kind":"opposite","given":"no name","expect":"no greeting"}]"#]);
    p.claim(&["criterion", "--criterion_id", "c1", "--text", "greet says hello", "--path", "full", "--behavior_id", "b1", "--promise_id", "p1"]);
    p.cases_kept("c1"); // the cases card passed (plan cases-card, 2026-10-25) [draft names]
    p.claim(&["place_chosen", "--criterion_id", "c1", "--places", r#"[{"file_path":"src/m.py","symbol":"greet","line_start":1,"line_end":2}]"#]);
    p.claim(&["test_planned", "--criterion_id", "c1", "--file_path", "tests/test_greet.py", "--will", "create"]);
    // the runner form of jest's table appends the files, as pytest's does; the stack's name
    // claims no runner the dry run probes, so the stand-in is read by pytest's rows alone
    p.claim(&["test_stack_detected", "--project_root", ".", "--test_stack", "standin_jest", "--test_command", "python3 -m pytest"]);
    p.claim(&["criterion_head", "--criterion_id", "c1"]);
    p.claim(&["criterion_assigned", "--criterion_id", "c1"]);
    p.claim(&["fix_location_category", "--criterion_id", "c1", "--category", "this_repository", "--where", ""]);
    p
}

const OWN: &str = "python3 -m pytest tests/test_greet.py";
const TOUCHING: &str = "python3 -m pytest tests/test_m.py";

/// The runs reserved at `commit`: (run id, command).
fn runs_at(p: &Project, commit: &str) -> Vec<(String, String)> {
    p.facts_of("run_reserved")
        .iter()
        .filter(|f| f["commit"].as_str() == Some(commit) && f["by"].as_str() == Some("binary"))
        .map(|f| (f["run_id"].as_str().unwrap_or("").to_string(), f["command"].as_str().unwrap_or("").to_string()))
        .collect()
}

fn translation(p: &Project, run_id: &str) -> Value {
    p.facts_of("test_log_translation").into_iter().find(|f| f["run_id"].as_str() == Some(run_id)).unwrap_or(Value::Null)
}

fn result_run(p: &Project, file: &str, commit: &str) -> Option<String> {
    p.facts_of("pass_please_pass")
        .iter()
        .rfind(|f| f["file_path"].as_str() == Some(file) && f["commit"].as_str() == Some(commit))
        .and_then(|f| f["run_id"].as_str().map(str::to_string))
}

#[test]
fn the_own_run_and_the_touching_run_are_two_runs_and_a_skip_in_a_touching_file_leaves_the_pass_green() {
    let p = project("touching-own-run");
    p.write("tests/test_greet.py", TEST_C1);
    p.commit("c1: tests");
    let tf = p.head();
    assert_eq!(p.state_name("c1"), "sealed", "{}", p.status());
    assert_eq!(runs_at(&p, &tf).iter().map(|(_, c)| c.as_str()).collect::<Vec<_>>(), [OWN], "before the fix nothing touching is known");
    // the fix breaks shout, which the existing tests/test_m.py checks
    p.write("src/m.py", &M_AT_BASE.replace("return \"hi\"", "return \"hello\""));
    p.commit("c1: greet says hello");
    let bad = p.head();
    let runs = runs_at(&p, &bad);
    let own = runs.iter().find(|(_, c)| c == OWN).unwrap_or_else(|| panic!("the own run: {runs:?}"));
    let touching = runs.iter().find(|(_, c)| c == TOUCHING).unwrap_or_else(|| panic!("the touching run: {runs:?}"));
    assert_eq!(runs.len(), 2, "{runs:?}");
    assert_ne!(own.0, touching.0, "two run ids");
    assert_eq!(result_run(&p, "tests/test_greet.py", &bad).as_deref(), Some(own.0.as_str()));
    assert_eq!(result_run(&p, "tests/test_m.py", &bad).as_deref(), Some(touching.0.as_str()), "the touching result names its own run");
    assert_eq!(translation(&p, &own.0)["counts"], serde_json::json!({"passed": 1}), "the own run counts its own tests only");
    assert_eq!(translation(&p, &touching.0)["label"], "tests_failed");
    assert_ne!(p.state_name("c1"), "green", "{}", p.status());
    // the fix that keeps shout as it was: the own pass is green beside the touching file's skip
    p.write("src/m.py", "def greet():\n    return \"hello\"\n\n\ndef shout():\n    return \"HI\"\n");
    p.commit("c1: shout as before");
    let good = p.head();
    let runs = runs_at(&p, &good);
    assert_eq!(runs.len(), 2, "{runs:?}");
    let own = runs.iter().find(|(_, c)| c == OWN).expect("the own run");
    let touching = runs.iter().find(|(_, c)| c == TOUCHING).expect("the touching run");
    assert_eq!(translation(&p, &own.0)["counts"], serde_json::json!({"passed": 1}));
    assert_eq!(translation(&p, &touching.0)["counts"], serde_json::json!({"passed": 1, "skipped": 1}));
    // pytest's table reads a pass beside a skip as unclassified; the run exited 0 and counted no
    // failure, which for a touching test is a pass
    assert_eq!(translation(&p, &touching.0)["label"], "unclassified");
    assert_eq!(p.state_name("c1"), "green", "{}", p.status());
    assert!(!p.status().contains("run_unclassified"), "{}", p.status());
}
