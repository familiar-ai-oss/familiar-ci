//! go test as plain data for the reader table (plan A, lane akgorust; [draft]): the readers, the
//! converter and the stream it reads, the selection flags re-aim drops, the setup files that are
//! watched, and the probe kit P1..P7 (P3b is stage 2's "the function now exists") with its truths.
//! There is no kit file: Go's own `go test -json` (test2json) is the reporter, and
//! `door::convert::test2json` reads it from the run's stdout.

/// An env op: (VAR, op `set|append|prepend|unset`, value, the separator put between the person's
/// own value and ours).
pub type EnvOp = (&'static str, &'static str, &'static str, &'static str);

/// A reader: (id, kind `reporter|report|pinned|tables`, env ops, flags added to Familiar's own run,
/// report globs).
pub type Reader = (&'static str, &'static str, &'static [EnvOp], &'static [&'static str], &'static [&'static str]);

/// A probe: (id, truth label, truth kind of a `tests_failed` or empty, files: path relative to the module root
/// and text; the first is the test the command names). The probes of stage 2 are P3b and P7.
pub type Probe = (&'static str, &'static str, &'static str, &'static [(&'static str, &'static str)]);

/// The runner's name, as the tables and the adapter file name it.
pub const RUNNER: &str = "go-test";

/// The kit files: none. Go's own `go test -json` is the reporter and `door::convert::test2json`
/// (Rust) reads it.
pub const KIT_FILES: &[(&str, &str)] = &[];

/// The converter in `door::convert` and the stream of the run it reads.
pub const CONVERTER: &str = "test2json";
pub const NATIVE_STREAM: &str = "stdout";

/// Why there are two routes, and what each run keeps (the A0 spike, go 1.23.12 and 1.25.14).
pub const NOTE: &str = "GOFLAGS=-json reaches every go call in the command: go vet then exits 0 with findings, and go list and go build (1.24 on) print JSON, so the env route is only for a command whose only go call is go test, with nothing piped after it; otherwise -json is inserted into the go test part of the command (the flag route), and a command that cannot be told gets the tables. Up to go 1.23 a setup failure (a missing import) prints plain text with no JSON, and a build failure's compiler lines go to stderr: no session line, the next reader. What Familiar keeps and shows of a run read this way (the kept log, the card's words) is JSON: the words come from the converted file.";

/// The readers, best first. The env route and the flag route are two readers for the one that is
/// possible for the person's command; the person's own `-json` makes either harmless.
pub const READERS: &[Reader] = &[
    ("go-test:reporter", "reporter", &[("GOFLAGS", "append", "-json", " ")], &[], &[]),
    ("go-test:reporter-flag", "reporter", &[], &["-json"], &[]),
    ("go-test:tables", "tables", &[], &[], &[]),
];

/// The selection flags re-aim drops from the person's command, with whether the flag takes a value
/// (`-run X` and `-run=X`); go's flag package takes the double dash too. The positional packages are
/// dropped by re-aim itself and the probe's package is appended.
pub const SELECTION_FLAGS: &[(&str, bool)] = &[("-run", true), ("--run", true), ("-skip", true), ("--skip", true)];

/// The setup files whose change calls for a new dry run, relative to the project root.
pub const WATCHED: &[&str] = &["**/go.mod"];

const CODE: &str = r#"package probe

import "strconv"

func Add(a, b int) int { return a + b }

func Parse(s string) int {
	n, err := strconv.Atoi(s)
	if err != nil {
		panic("not a number: " + s)
	}
	return n
}
"#;

const CODE_WITH_MUL: &str = r#"package probe

import "strconv"

func Add(a, b int) int { return a + b }

func Parse(s string) int {
	n, err := strconv.Atoi(s)
	if err != nil {
		panic("not a number: " + s)
	}
	return n
}

func Mul(a, b int) int { return a * b }
"#;

const TEST_P1: &str = r#"package probe

import "testing"

func TestAdd(t *testing.T) {
	if got := Add(2, 2); got != 4 {
		t.Errorf("Add(2, 2) = %d, want 4", got)
	}
}
"#;

const TEST_P2: &str = r#"package probe

import "testing"

func TestAdd(t *testing.T) {
	if got := Add(2, 2); got != 5 {
		t.Errorf("Add(2, 2) = %d, want 5", got)
	}
}
"#;

const TEST_P3: &str = r#"package probe

import "testing"

func TestMul(t *testing.T) {
	if got := Mul(2, 3); got != 6 {
		t.Errorf("Mul(2, 3) = %d, want 6", got)
	}
}
"#;

const TEST_P4: &str = r#"package probe

import (
	"testing"

	"example.com/nosuch"
)

func TestAdd(t *testing.T) {
	if got := Add(2, 2); got != nosuch.Four {
		t.Errorf("Add(2, 2) = %d, want 4", got)
	}
}
"#;

const TEST_P5: &str = r#"package probe

import "testing"

func TestAdd(t *testing.T) {
	if got := Add(2, 2); got != 4 {
		t.Errorf("Add(2, 2) = %d, want 4", got)
	}
}

func TestLater(t *testing.T) {
	t.Skip("not yet")
}
"#;

const TEST_P6: &str = r#"package probe

func helper() int { return Add(1, 1) }
"#;

const TEST_P7: &str = r#"package probe

import "testing"

func TestParse(t *testing.T) {
	if got := Parse("x1"); got != 1 {
		t.Errorf("Parse(x1) = %d, want 1", got)
	}
}
"#;

/// The probe kit, a package per probe inside the project's own module (the A0 spike's recordings
/// are the same files as modules of their own): stage 1 P1 pass, P2 assertion, P3 missing function,
/// P4 broken import (a package that is not there is a missing module, not a missing name), P5 a pass
/// beside a skip, P6 nothing to run; stage 2 P3b (the function exists) pass and P7 a panic inside
/// the code's own function. The test comes first: its package is what the command names.
pub const PROBES: &[Probe] = &[
    ("P1", "passed", "", &[("familiar_probe_p1/probe_test.go", TEST_P1), ("familiar_probe_p1/probe.go", CODE)]),
    ("P2", "tests_failed", "assertion", &[("familiar_probe_p2/probe_test.go", TEST_P2), ("familiar_probe_p2/probe.go", CODE)]),
    ("P3", "missing_target_symbol", "", &[("familiar_probe_p3/probe_test.go", TEST_P3), ("familiar_probe_p3/probe.go", CODE)]),
    ("P3b", "passed", "", &[("familiar_probe_p3b/probe_test.go", TEST_P3), ("familiar_probe_p3b/probe.go", CODE_WITH_MUL)]),
    ("P4", "collection_error", "", &[("familiar_probe_p4/probe_test.go", TEST_P4), ("familiar_probe_p4/probe.go", CODE)]),
    ("P5", "passed", "", &[("familiar_probe_p5/probe_test.go", TEST_P5), ("familiar_probe_p5/probe.go", CODE)]),
    ("P6", "no_tests_collected", "", &[("familiar_probe_p6/probe_test.go", TEST_P6), ("familiar_probe_p6/probe.go", CODE)]),
    ("P7", "tests_failed", "exception", &[("familiar_probe_p7/probe_test.go", TEST_P7), ("familiar_probe_p7/probe.go", CODE)]),
];

/// The tests of probe P5 that must read skipped, by name (a test's id carries its package's import
/// path in front).
pub const SKIPPED: &[&str] = &["TestLater"];
