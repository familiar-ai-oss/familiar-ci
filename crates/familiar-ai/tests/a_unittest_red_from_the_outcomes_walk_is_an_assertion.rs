//! plugin/tests/test-outcomes.sh's "an assertion seals": a unittest test whose assertEqual fails,
//! run in a run copy by this machine's python (3.13+: the caret line under the call). Its output,
//! kept byte for byte from the walk's run log, must read as a failed test whose location is the
//! planned test's assert line with the kind `assertion`; with no location H6's rules read the
//! run can't-tell and the walk's honest red becomes the run_unclassified card (integration,
//! 2026-10-19).

use familiar_ai::door::kept::{classify, Streams};
use familiar_ai_facts::LocationKind;

const WALK_STDERR: &str = "F
======================================================================
FAIL: test (test_greeting.T.test)
----------------------------------------------------------------------
Traceback (most recent call last):
  File \"/tmp/familiar-ai-run-2874245-0/tests/test_greeting.py\", line 5, in test
    def test(self): self.assertEqual(app.greet(), \"hello\")
                    ~~~~~~~~~~~~~~~~^^^^^^^^^^^^^^^^^^^^^^
AssertionError: 'hi' != 'hello'
- hi
+ hello


----------------------------------------------------------------------
Ran 1 test in 0.000s

FAILED (failures=1)
";

#[test]
fn the_walks_unittest_assertion_is_located_at_the_test_line_as_an_assertion() {
    let s = Streams { stdout: "", stderr: WALK_STDERR, report: "" };
    let t = classify(familiar_ai_run::End::Exited(1), &s, familiar_ai_run::details_for("python3 -m unittest discover -s tests")).expect("a reading");
    assert_eq!(t.label, "tests_failed");
    let at = t.candidates.first().expect("a location");
    assert_eq!(at.file_path, "tests/test_greeting.py", "{at:?}");
    assert_eq!(at.line, 5, "{at:?}");
    assert_eq!(at.kind, Some(LocationKind::Assertion), "{at:?}");
}
