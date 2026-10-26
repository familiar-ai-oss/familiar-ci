//! A unittest `FAIL:` block is unittest's own word for an assertion (failureException): its
//! last frame before the `AssertionError` line is an assertion-kind location, so an honest red
//! of a unittest test seals under the honesty rule (H-1) instead of reading can't-tell
//! (integration, 2026-10-19: the milestone referee's editable-install walk).

use familiar_ai::door::kept::{classify, Streams};
use familiar_ai_facts::LocationKind;

#[test]
fn a_fail_block_gives_its_test_frame_as_an_assertion() {
    let d = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/run_logs/unittest/tests_failed");
    let stdout = std::fs::read_to_string(format!("{d}/stdout")).unwrap_or_default();
    let stderr = std::fs::read_to_string(format!("{d}/stderr")).unwrap_or_default();
    let s = Streams { stdout: &stdout, stderr: &stderr, report: "" };
    let t = classify(familiar_ai_run::End::Exited(1), &s, familiar_ai_run::details_for("python -m unittest tests.test_calc")).expect("a reading");
    assert_eq!(t.label, "tests_failed");
    let at = t.candidates.first().expect("a location");
    assert!(at.file_path.ends_with("tests/test_calc.py"), "{at:?}");
    assert_eq!(at.line, 8);
    assert_eq!(at.kind, Some(LocationKind::Assertion), "{at:?}");
}

#[test]
fn a_fail_block_with_its_own_failure_exception_is_an_assertion_too() {
    let stderr = "F\n======================================================================\nFAIL: test_add (tests.test_calc.CalcTest.test_add)\n----------------------------------------------------------------------\nTraceback (most recent call last):\n  File \"tests/test_calc.py\", line 8, in test_add\n    self.assertEqual(add(2, 2), 5)\nMyFailure: 4 != 5\n\n----------------------------------------------------------------------\nRan 1 test in 0.000s\n\nFAILED (failures=1)\n";
    let s = Streams { stdout: "", stderr, report: "" };
    let t = classify(familiar_ai_run::End::Exited(1), &s, familiar_ai_run::details_for("python -m unittest tests.test_calc")).expect("a reading");
    let at = t.candidates.first().expect("a location");
    assert_eq!((at.line, at.kind), (8, Some(LocationKind::Assertion)), "{at:?}");
}
