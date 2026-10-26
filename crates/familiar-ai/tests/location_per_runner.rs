//! S9b: the location and missing name of a crash, an exception or a compile error, per runner
//! (plan §6.2, §7.3 line 4; Assumptions 36 and 38). Each recorded run (S0's fixtures) that carries
//! one is read through the binary's path: the runner its command names classifies it, its output
//! is moved into a run copy (`familiar-ai-run-<pid>-<n>`, as `verify_run` lays it out), and the
//! place kept is the first one git tracks at the run's commit, stood in for by the fixture's own
//! sources (`run_sources/<runner>/<fixture>`, the project as the recorder wrote it; never `.venv`,
//! `node_modules` or a build folder). A fixture with nothing to extract says why.

use std::path::{Path, PathBuf};

use familiar_ai::door::kept::{classify, clean, function_extent, in_project, place_candidate, Streams};
use familiar_ai_facts::{Location, Place, TestLogTranslation};
use familiar_ai_run::{details_for, End};

/// Where the recorder ran each fixture: the scratch tree (`R` in `_gen/lib.sh`) and the docker
/// mount; both are the project root of `<runner>/<fixture>` below them.
const RECORDED_AT: &[&str] = &["/tmp/claude-1000/-home-user-projects-familiar-ai-oss/ce46f81f-ec40-4c7d-885d-547ef9d911b3/scratchpad/run_logs", "/r"];

/// The run copy the binary makes (`verify_run`, plan §6.2).
const COPY: &str = "/tmp/familiar-ai-run-4242-7";

/// What a fixture's run says: the place (root-relative file, line, the function git's hunk header
/// names above it) and the missing name, or why nothing is extracted.
enum Want {
    At(&'static str, u64, &'static str, Option<&'static str>),
    /// A missing name with no line to point at.
    NameOnly(&'static str, &'static str),
    Nothing(&'static str),
}
use Want::{At, NameOnly, Nothing};

const SIGNAL: &str = "the process died of the signal and printed no frame of its own";
const SIGKILL: &str = "killed from outside: the label is killed, which carries no location";
const ABSORBED: &str = "the .NET runtime absorbed the signal and the run passed: no location to read";
const CPP: &str = "C++ exceptions and crashes carry no location of the code under test (plan §9.5): googletest says `unknown file`, Catch2 names the test's last REQUIRE line, ctest prints the test's name only";
const SUREFIRE: &str = "the forked JVM died; surefire prints only its own frames (no project frame)";
const GRADLE_FORK: &str = "the forked test JVM died; gradle prints only that the executor exited";
const NODE_LOC: &str = "node's TAP `location:` names the test file's first line, not where the process died";
const VITEST_WORKER: &str = "the worker died; vitest prints only its own frames from node_modules";
const SPIKE_KIT: &str = "recorded under the A0 spike's kit, whose not-found handler printed `bash: <name>: command not found` with no file or line (Familiar's kit has none: bash's own line stays)";
const JEST_WORKER: &str = "the worker died; jest prints only its own frames from node_modules";
const TESTHOST: &str = "the testhost died; VSTest prints that it crashed, with no frame";
const VITE_LINES: &str = "vite transforms the test file and node's native stack counts the transformed lines (line 11 of a 10-line file): not the source's line";
const VITE_PARSE: &str = "vite's parse error says 1:0, not the line of the error (6)";
const TB_NO: &str = "--tb=no prints only the FAILED line (the terminal width cuts the exception's text): the reading has no file or line to point at";
const TWO_SECTIONS: &str = "two failing tests print two sections: pytest's rows read one section only, so neither is the failure (H3 rule 4 generalised)";
const LINK: &str = "a link error names the symbol and an object file's offset, no source line";

/// Every fixture under a prefix S9b covers (exception, missing name, compile error, signal).
const WANT: &[(&str, &str, Want)] = &[
    ("cargo-nextest", "collection_error", At("tests/calc_test.rs", 5, "fn adds() {", None)),
    ("cargo-nextest", "collection_error_import", At("tests/calc_test.rs", 2, "use calc::nosuchmod::helper;", None)),
    ("cargo-nextest", "killed_abort", Nothing(SIGNAL)),
    ("cargo-nextest", "killed_segv", Nothing(SIGNAL)),
    ("cargo-nextest", "killed_segv_shell", Nothing(SIGNAL)),
    ("cargo-nextest", "killed_sigkill", Nothing(SIGKILL)),
    ("cargo-nextest", "missing_target_symbol", At("tests/calc_test.rs", 5, "fn multiplies() {", Some("mul"))),
    ("cargo-nextest", "tests_failed_exception", At("src/calc.rs", 6, "pub fn parse(s: &str) -> i64 {", None)),
    ("cargo-nextest", "tests_failed_exception_assert_in_code", At("src/calc.rs", 6, "pub fn parse(s: &str) -> i64 {", None)),
    ("cargo-nextest", "tests_failed_exception_assert_message", At("tests/calc_test.rs", 5, "fn adds() {", None)),
    ("cargo-nextest", "tests_failed_exception_backtrace", At("src/calc.rs", 6, "pub fn parse(s: &str) -> i64 {", None)),
    ("cargo-nextest", "tests_failed_exception_status_level_none", At("src/calc.rs", 6, "pub fn parse(s: &str) -> i64 {", None)),
    ("cargo-nextest", "tests_failed_exception_unwrap_in_test", At("tests/calc_test.rs", 5, "fn parses() {", None)),
    ("cargo-test", "collection_error", At("tests/calc_test.rs", 5, "fn adds() {", None)),
    ("cargo-test", "collection_error_import", At("tests/calc_test.rs", 2, "use calc::nosuchmod::helper;", None)),
    ("cargo-test", "killed_abort", Nothing(SIGNAL)),
    ("cargo-test", "killed_segv", Nothing(SIGNAL)),
    ("cargo-test", "killed_segv_shell", Nothing(SIGNAL)),
    ("cargo-test", "killed_sigkill", Nothing(SIGKILL)),
    ("cargo-test", "missing_target_symbol", At("tests/calc_test.rs", 5, "fn multiplies() {", Some("mul"))),
    ("cargo-test", "missing_target_symbol_message_format_short", At("tests/calc_test.rs", 5, "fn multiplies() {", Some("mul"))),
    ("cargo-test", "tests_failed_exception", At("src/calc.rs", 6, "pub fn parse(s: &str) -> i64 {", None)),
    ("cargo-test", "tests_failed_exception_assert_in_code", At("src/calc.rs", 6, "pub fn parse(s: &str) -> i64 {", None)),
    ("cargo-test", "tests_failed_exception_assert_message", At("tests/calc_test.rs", 5, "fn adds() {", None)),
    ("cargo-test", "tests_failed_exception_backtrace", At("src/calc.rs", 6, "pub fn parse(s: &str) -> i64 {", None)),
    ("cargo-test", "tests_failed_exception_format_json", At("src/calc.rs", 6, "pub fn parse(s: &str) -> i64 {", None)),
    ("cargo-test", "tests_failed_exception_unwrap_in_test", At("tests/calc_test.rs", 5, "fn parses() {", None)),
    ("catch2", "collection_error", At("tests/calc_test.cpp", 7, "TEST_CASE(\"Adds\") {", None)),
    ("catch2", "killed_abort", Nothing(CPP)),
    ("catch2", "killed_segv", Nothing(CPP)),
    ("catch2", "killed_segv_shell", Nothing(CPP)),
    ("catch2", "killed_sigkill", Nothing(SIGKILL)),
    ("catch2", "missing_target_symbol", At("tests/calc_test.cpp", 6, "TEST_CASE(\"Multiplies\") {", Some("mul"))),
    ("catch2", "tests_failed_exception", Nothing(CPP)),
    ("ctest", "collection_error", At("tests/calc_test.cpp", 5, "int main() {", None)),
    ("ctest", "killed_abort", Nothing(CPP)),
    ("ctest", "killed_segv", Nothing(CPP)),
    ("ctest", "killed_segv_shell", Nothing(CPP)),
    ("ctest", "killed_sigkill", Nothing(SIGKILL)),
    ("ctest", "missing_target_symbol", At("tests/calc_test.cpp", 4, "int main() {", Some("mul"))),
    ("ctest", "missing_target_symbol_link", NameOnly("mul", LINK)),
    ("ctest", "tests_failed_exception", Nothing(CPP)),
    ("ctest", "tests_failed_exception_output_on_failure", Nothing(CPP)),
    ("dotnet-mstest", "collection_error", At("tests/CalcTests.cs", 12, "public void Adds()", None)),
    ("dotnet-mstest", "killed_failfast", Nothing(TESTHOST)),
    ("dotnet-mstest", "killed_segv", Nothing(TESTHOST)),
    ("dotnet-mstest", "killed_segv_sent_by_kill", Nothing(ABSORBED)),
    ("dotnet-mstest", "killed_sigkill", Nothing(SIGKILL)),
    ("dotnet-mstest", "missing_target_symbol", At("tests/CalcTests.cs", 12, "public void Multiplies()", Some("Mul"))),
    ("dotnet-mstest", "tests_failed_exception", At("src/Calc.cs", 9, "public static int Parse(string s)", None)),
    ("dotnet-nunit", "collection_error", At("tests/CalcTests.cs", 12, "public void Adds()", None)),
    ("dotnet-nunit", "collection_error_import", At("tests/CalcTests.cs", 1, "using Nosuch.Helpers;", Some("Nosuch"))),
    ("dotnet-nunit", "killed_failfast", Nothing(TESTHOST)),
    ("dotnet-nunit", "killed_segv", Nothing(TESTHOST)),
    ("dotnet-nunit", "killed_segv_sent_by_kill", Nothing(ABSORBED)),
    ("dotnet-nunit", "killed_sigkill", Nothing(SIGKILL)),
    ("dotnet-nunit", "missing_target_symbol", At("tests/CalcTests.cs", 12, "public void Multiplies()", Some("Mul"))),
    ("dotnet-nunit", "tests_failed_exception", At("src/Calc.cs", 9, "public static int Parse(string s)", None)),
    ("dotnet-xunit", "collection_error", At("tests/CalcTests.cs", 11, "public void Adds()", None)),
    ("dotnet-xunit", "collection_error_import", At("tests/CalcTests.cs", 1, "using Nosuch.Helpers;", Some("Nosuch"))),
    ("dotnet-xunit", "killed_failfast", Nothing(TESTHOST)),
    ("dotnet-xunit", "killed_segv", Nothing(TESTHOST)),
    ("dotnet-xunit", "killed_segv_sent_by_kill", Nothing(ABSORBED)),
    ("dotnet-xunit", "killed_segv_shell", Nothing(TESTHOST)),
    ("dotnet-xunit", "killed_sigkill", Nothing(SIGKILL)),
    ("dotnet-xunit", "missing_target_symbol", At("tests/CalcTests.cs", 11, "public void Multiplies()", Some("Mul"))),
    ("dotnet-xunit", "tests_failed_exception", At("src/Calc.cs", 9, "public static int Parse(string s)", None)),
    ("dotnet-xunit", "tests_failed_exception_locale_de", Nothing("German VSTest words (Fehlermeldung:, Stapelverfolgung:): no English row reads the block, so no table gives a place")),
    ("go-test", "collection_error", At("calc_test.go", 8, "func TestAdd(t *testing.T) {", None)),
    ("go-test", "collection_error_import", Nothing("go prints the same words for a missing package of this module and for a third-party one: a card, which carries no location (lane h4rustgo)")),
    ("go-test", "killed_abort", At("calc_test.go", 16, "func TestCrash(t *testing.T) {", None)),
    ("go-test", "killed_nil_deref", Nothing("a recovered panic whose innermost project frame is the test file reads as a card (lane h4rustgo, D1): no label that carries a location")),
    ("go-test", "killed_segv", At("calc_test.go", 16, "func TestCrash(t *testing.T) {", None)),
    ("go-test", "killed_segv_shell", At("calc_test.go", 16, "func TestCrash(t *testing.T) {", None)),
    ("go-test", "killed_sigkill", Nothing(SIGKILL)),
    ("go-test", "missing_target_symbol", At("calc_test.go", 6, "func TestMul(t *testing.T) {", Some("Mul"))),
    ("go-test", "missing_target_symbol_json", At("calc_test.go", 6, "func TestMul(t *testing.T) {", Some("Mul"))),
    ("go-test", "missing_target_symbol_json_go125", At("calc_test.go", 6, "func TestMul(t *testing.T) {", Some("Mul"))),
    ("go-test", "tests_failed_exception", At("calc.go", 12, "func Parse(s string) int {", None)),
    ("go-test", "tests_failed_exception_json", Nothing("`go test -json` wraps every frame line in an event: a panic there is a card and keeps no location (lane h4rustgo)")),
    ("go-test", "tests_failed_exception_nil_deref", At("calc.go", 14, "func Parse(s string) int {", None)),
    ("googletest", "collection_error", At("tests/calc_test.cpp", 7, "TEST(Calc, Adds) {", None)),
    ("googletest", "killed_abort", Nothing(CPP)),
    ("googletest", "killed_segv", Nothing(CPP)),
    ("googletest", "killed_segv_shell", Nothing(CPP)),
    ("googletest", "killed_sigkill", Nothing(SIGKILL)),
    ("googletest", "missing_target_symbol", At("tests/calc_test.cpp", 6, "TEST(Calc, Multiplies) {", Some("mul"))),
    ("googletest", "tests_failed_exception", Nothing(CPP)),
    ("gradle", "collection_error", At("src/test/java/calc/CalcTest.java", 10, "void adds() {", None)),
    ("gradle", "collection_error_import", At("src/test/java/calc/CalcTest.java", 6, "import org.nosuch.Helper;", None)),
    ("gradle", "killed_abort", Nothing(GRADLE_FORK)),
    ("gradle", "killed_exit", Nothing(GRADLE_FORK)),
    ("gradle", "killed_segv", Nothing(GRADLE_FORK)),
    ("gradle", "killed_segv_shell", Nothing(GRADLE_FORK)),
    ("gradle", "killed_sigkill", Nothing(SIGKILL)),
    ("gradle", "missing_target_symbol", At("src/test/java/calc/CalcTest.java", 10, "void multiplies() {", Some("mul"))),
    ("gradle", "tests_failed_exception", At("src/main/java/calc/Calc.java", 9, "public static int parse(String s) {", None)),
    ("jest", "collection_error", At("test/calc.test.js", 2, "const helper = require('./nosuchhelper');", None)),
    ("jest", "collection_error_syntax", At("test/calc.test.js", 5, "test('adds', () => {", None)),
    ("jest", "killed_abort", At("test/calc.test.js", 8, "test('crashes', () => {", None)),
    ("jest", "killed_segv", Nothing(SIGNAL)),
    ("jest", "killed_segv_shell", Nothing(SIGNAL)),
    ("jest", "killed_segv_workers", Nothing(JEST_WORKER)),
    ("jest", "killed_sigkill", Nothing(SIGKILL)),
    ("jest", "missing_target_symbol", At("test/calc.test.ts", 1, "import { add, mul } from '../src/calc';", Some("mul"))),
    ("jest", "missing_target_symbol_js", At("test/calc.test.js", 4, "test('multiplies', () => {", Some("mul"))),
    ("jest", "tests_failed_exception", At("src/calc.js", 7, "function parse(s) {", None)),
    ("maven", "collection_error", At("src/test/java/calc/CalcTest.java", 10, "void adds() {", None)),
    ("maven", "collection_error_import", At("src/test/java/calc/CalcTest.java", 6, "import org.nosuch.Helper;", None)),
    ("maven", "killed_abort", Nothing(SUREFIRE)),
    ("maven", "killed_exit", Nothing(SUREFIRE)),
    ("maven", "killed_segv", Nothing(SUREFIRE)),
    ("maven", "killed_segv_shell", Nothing(SUREFIRE)),
    ("maven", "killed_sigkill", Nothing(SIGKILL)),
    ("maven", "missing_target_symbol", At("src/test/java/calc/CalcTest.java", 10, "void multiplies() {", Some("mul"))),
    ("maven", "missing_target_symbol_stale_report", At("src/test/java/calc/CalcTest.java", 10, "void multiplies() {", Some("mul"))),
    ("maven", "tests_failed_exception", At("src/main/java/calc/Calc.java", 9, "public static int parse(String s) {", None)),
    ("maven", "tests_failed_exception_quiet", At("src/main/java/calc/Calc.java", 9, "public static int parse(String s) {", None)),
    ("mocha", "collection_error", At("test/calc.test.js", 3, "const helper = require('./nosuchhelper');", None)),
    ("mocha", "collection_error_syntax", At("test/calc.test.js", 7, "describe('calc', () => {", None)),
    ("mocha", "killed_abort", At("test/calc.test.js", 10, "describe('calc', () => {", None)),
    ("mocha", "killed_segv", Nothing(SIGNAL)),
    ("mocha", "killed_segv_shell", Nothing(SIGNAL)),
    ("mocha", "killed_sigkill", Nothing(SIGKILL)),
    ("mocha", "missing_target_symbol", At("test/calc.test.mjs", 2, "import { mul } from '../src/calc.mjs';", Some("mul"))),
    ("mocha", "missing_target_symbol_js", At("test/calc.test.js", 6, "describe('calc', () => {", Some("mul"))),
    ("mocha", "tests_failed_exception", At("src/calc.js", 7, "function parse(s) {", None)),
    ("node-test", "collection_error", At("test/calc.test.js", 4, "const helper = require('./nosuchhelper');", None)),
    ("node-test", "collection_error_syntax", At("test/calc.test.js", 7, "test('adds', () => {", None)),
    ("node-test", "collection_error_spec", At("test/calc.test.js", 4, "const helper = require('./nosuchhelper');", None)),
    ("node-test", "missing_target_symbol_spec", At("test/calc.test.js", 6, "test('multiplies', () => {", Some("mul"))),
    ("node-test", "tests_failed_exception_spec", At("src/calc.js", 7, "function parse(s) {", None)),
    ("node-test", "killed_abort", At("test/calc.test.js", 10, "test('crashes', () => {", None)),
    ("node-test", "killed_segv", Nothing(NODE_LOC)),
    ("node-test", "killed_segv_no_isolation", Nothing(SIGNAL)),
    ("node-test", "killed_segv_shell", Nothing(NODE_LOC)),
    ("node-test", "killed_sigkill", Nothing(SIGKILL)),
    ("node-test", "missing_target_symbol", At("test/calc.test.mjs", 3, "import { mul } from '../src/calc.mjs';", Some("mul"))),
    ("node-test", "missing_target_symbol_js", At("test/calc.test.js", 6, "test('multiplies', () => {", Some("mul"))),
    ("node-test", "tests_failed_exception", At("src/calc.js", 7, "function parse(s) {", None)),
    ("pest", "collection_error", At("tests/CalcTest.php", 11, "test('Add', function () {", None)),
    ("pest", "collection_error_import", At("tests/CalcTest.php", 6, "require_once __DIR__ . '/../src/NosuchHelper.php';", None)),
    ("pest", "killed_abort", Nothing(SIGNAL)),
    ("pest", "killed_segv", Nothing(SIGNAL)),
    ("pest", "killed_segv_shell", Nothing(SIGNAL)),
    ("pest", "killed_sigkill", Nothing(SIGKILL)),
    ("pest", "missing_target_symbol", At("tests/CalcTest.php", 10, "test('Mul', function () {", Some("mul"))),
    ("pest", "missing_target_symbol_compact", At("tests/CalcTest.php", 10, "test('Mul', function () {", Some("mul"))),
    ("pest", "tests_failed_exception", At("src/Calc.php", 17, "public static function parse(string $s): int", None)),
    ("pest", "tests_failed_exception_compact", At("src/Calc.php", 17, "public static function parse(string $s): int", None)),
    ("phpunit", "collection_error", At("tests/CalcTest.php", 15, "public function testAdd(): void", None)),
    ("phpunit", "collection_error_compact", At("tests/CalcTest.php", 15, "public function testAdd(): void", None)),
    ("phpunit", "collection_error_import", At("tests/CalcTest.php", 6, "require_once __DIR__ . '/../src/NosuchHelper.php';", None)),
    ("phpunit", "killed_abort", Nothing(SIGNAL)),
    ("phpunit", "killed_segv", Nothing(SIGNAL)),
    ("phpunit", "killed_segv_shell", Nothing(SIGNAL)),
    ("phpunit", "killed_sigkill", Nothing(SIGKILL)),
    ("phpunit", "missing_target_symbol", At("tests/CalcTest.php", 14, "public function testMul(): void", Some("mul"))),
    ("phpunit", "missing_target_symbol_compact", At("tests/CalcTest.php", 14, "public function testMul(): void", Some("mul"))),
    ("phpunit", "tests_failed_exception", At("src/Calc.php", 17, "public static function parse(string $s): int", None)),
    ("phpunit", "tests_failed_exception_compact", At("src/Calc.php", 17, "public static function parse(string $s): int", None)),
    ("pytest", "collection_error", At("tests/test_calc.py", 1, "import nosuchmodule", None)),
    ("pytest", "collection_error_pretty", At("tests/test_calc.py", 1, "import nosuchmodule", None)),
    ("pytest", "collection_error_syntax", At("tests/test_calc.py", 4, "def test_add(:", None)),
    ("pytest", "killed_abort", At("tests/test_calc.py", 12, "def test_crash():", None)),
    ("pytest", "killed_segv", At("tests/test_calc.py", 12, "def test_crash():", None)),
    ("pytest", "killed_segv_shell", At("tests/test_calc.py", 12, "def test_crash():", None)),
    ("pytest", "killed_sigkill", Nothing(SIGKILL)),
    ("pytest", "missing_target_symbol", At("tests/test_calc.py", 1, "from calc import mul", Some("calc.mul"))),
    ("pytest", "missing_target_symbol_attr", At("tests/test_calc.py", 5, "def test_mul():", Some("calc.mul"))),
    ("pytest", "tests_failed_exception", At("src/calc.py", 7, "def parse(s):", None)),
    ("pytest", "collection_error_syntax_xdist", At("tests/test_calc.py", 4, "def test_add(:", None)),
    ("pytest", "collection_error_xdist", At("tests/test_calc.py", 1, "import nosuchmodule", None)),
    ("pytest", "collection_error_xdist_q", At("tests/test_calc.py", 1, "import nosuchmodule", None)),
    ("pytest", "missing_target_symbol_mixed_typo", At("tests/test_calc.py", 10, "def test_mul():", Some("calc.mull"))),
    ("pytest", "missing_target_symbol_tb_line", At("tests/test_calc.py", 5, "def test_mul():", Some("calc.mul"))),
    ("pytest", "missing_target_symbol_tb_native", At("tests/test_calc.py", 5, "def test_mul():", Some("calc.mul"))),
    ("pytest", "missing_target_symbol_tb_no", Nothing(TB_NO)),
    ("pytest", "missing_target_symbol_tb_short", At("tests/test_calc.py", 5, "def test_mul():", Some("calc.mul"))),
    ("pytest", "missing_target_symbol_wrong_module", At("tests/test_calc.py", 5, "def test_mul():", Some("os.mul"))),
    ("pytest", "missing_target_symbol_xdist", At("tests/test_calc.py", 1, "from calc import mul", Some("calc.mul"))),
    ("pytest", "tests_failed_exception_attr", At("src/calc.py", 9, "def parse(s):", None)),
    ("pytest", "tests_failed_exception_attr_tb_line", At("src/calc.py", 9, "def parse(s):", None)),
    ("pytest", "tests_failed_exception_attr_tb_native", At("src/calc.py", 9, "def parse(s):", None)),
    ("pytest", "tests_failed_exception_attr_tb_short", At("src/calc.py", 9, "def parse(s):", None)),
    ("pytest", "tests_failed_exception_tb_line", At("src/calc.py", 7, "def parse(s):", None)),
    ("pytest", "tests_failed_exception_tb_native", At("src/calc.py", 7, "def parse(s):", None)),
    ("pytest", "tests_failed_exception_tb_no", Nothing(TB_NO)),
    ("pytest", "tests_failed_exception_tb_short", At("src/calc.py", 7, "def parse(s):", None)),
    ("pytest", "tests_failed_exception_two", Nothing(TWO_SECTIONS)),
    ("sh", "missing_target_symbol", At("tests/test_app.sh", 2, "farewell", Some("farewell"))),
    // lane H5: the A0 spike's not-found probe, per setup
    ("sh", "missing_target_symbol_bash_env_notfound", Nothing(SPIKE_KIT)),
    ("sh", "missing_target_symbol_bash_shebang_notfound", Nothing(SPIKE_KIT)),
    ("sh", "missing_target_symbol_busybox_x_notfound", At("posix/s_notfound.sh", 3, "frobnicate_widget --all", Some("frobnicate_widget"))),
    ("sh", "missing_target_symbol_dash_x_notfound", At("posix/s_notfound.sh", 3, "frobnicate_widget --all", Some("frobnicate_widget"))),
    ("sh", "missing_target_symbol_sh_is_bash_bashenv_notfound", At("posix/s_notfound.sh", 3, "frobnicate_widget --all", Some("frobnicate_widget"))),
    ("sh", "missing_target_symbol_sh_is_bash_shellopts_notfound", At("posix/s_notfound.sh", 3, "frobnicate_widget --all", Some("frobnicate_widget"))),
    ("unittest", "collection_error", At("tests/test_calc.py", 3, "import nosuchmodule", None)),
    ("unittest", "collection_error_syntax", At("tests/test_calc.py", 5, "class CalcTest(unittest.TestCase):", None)),
    ("unittest", "killed_abort", Nothing(SIGNAL)),
    ("unittest", "killed_segv", Nothing(SIGNAL)),
    ("unittest", "killed_segv_shell", Nothing(SIGNAL)),
    ("unittest", "killed_sigkill", Nothing(SIGKILL)),
    ("unittest", "missing_target_symbol", At("tests/test_calc.py", 3, "from calc import mul", Some("calc.mul"))),
    ("unittest", "tests_failed_exception", At("calc.py", 7, "def parse(s):", None)),
    ("unittest", "missing_target_symbol_attr", At("tests/test_calc.py", 8, "class CalcTest(unittest.TestCase):", Some("calc.mul"))),
    ("unittest", "missing_target_symbol_mixed_typo", At("tests/test_calc.py", 12, "class CalcTest(unittest.TestCase):", Some("calc.mull"))),
    ("unittest", "missing_target_symbol_typo", At("tests/test_calc.py", 8, "class CalcTest(unittest.TestCase):", Some("calc.mull"))),
    ("unittest", "missing_target_symbol_wrong_module", At("tests/test_calc.py", 7, "class CalcTest(unittest.TestCase):", Some("os.mul"))),
    ("unittest", "tests_failed_exception_two", At("calc.py", 7, "def parse(s):", None)),
    ("vitest", "collection_error", At("test/calc.test.js", 3, "import { helper } from './nosuchhelper.js';", None)),
    ("vitest", "collection_error_syntax", Nothing(VITE_PARSE)),
    ("vitest", "killed_abort", Nothing(VITE_LINES)),
    ("vitest", "killed_segv", Nothing(VITEST_WORKER)),
    ("vitest", "killed_segv_shell", Nothing(VITEST_WORKER)),
    ("vitest", "killed_sigkill", Nothing(SIGKILL)),
    ("vitest", "missing_target_symbol", At("test/calc.test.js", 5, "test('multiplies', () => {", Some("mul"))),
    ("vitest", "missing_target_symbol_ts", At("test/calc.test.ts", 5, "test('multiplies', () => {", Some("mul"))),
    ("vitest", "tests_failed_exception", At("src/calc.js", 7, "export function parse(s) {", None)),
];

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

struct Fixture {
    runner: String,
    folder: String,
    end: End,
    stdout: String,
    stderr: String,
    report: String,
    cmd: String,
}

impl Fixture {
    fn streams(&self) -> Streams<'_> {
        Streams { stdout: &self.stdout, stderr: &self.stderr, report: &self.report }
    }
}

/// The text as a run copy would print it: each recording root of this fixture becomes the copy.
fn text(p: &Path, runner: &str, folder: &str) -> String {
    let mut t = clean(&String::from_utf8_lossy(&std::fs::read(p).unwrap_or_default()));
    for at in RECORDED_AT {
        t = t.replace(&format!("{at}/{runner}/{folder}/"), &format!("{COPY}/")).replace(&format!("{at}/{runner}/{folder}"), COPY);
    }
    t
}

fn report(dir: &Path, runner: &str, folder: &str) -> String {
    let mut files = Vec::new();
    let mut stack = vec![dir.join("report")];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).into_iter().flatten().flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else {
                files.push(p);
            }
        }
    }
    files.sort();
    files.iter().map(|p| text(p, runner, folder)).collect::<Vec<_>>().join("\n")
}

const PREFIXES: &[&str] = &["tests_failed_exception", "missing_target_symbol", "collection_error", "killed_"];

fn fixtures() -> Vec<Fixture> {
    let mut out = Vec::new();
    let mut runners: Vec<PathBuf> = std::fs::read_dir(root().join("run_logs")).into_iter().flatten().flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
    runners.sort();
    for r in runners {
        let mut folders: Vec<PathBuf> = std::fs::read_dir(&r).into_iter().flatten().flatten().map(|e| e.path()).filter(|p| p.join("exit").is_file()).collect();
        folders.sort();
        for d in folders {
            let runner = r.file_name().unwrap_or_default().to_string_lossy().into_owned();
            let folder = d.file_name().unwrap_or_default().to_string_lossy().into_owned();
            if !PREFIXES.iter().any(|p| folder.starts_with(p)) {
                continue;
            }
            let code: i32 = std::fs::read_to_string(d.join("exit")).unwrap_or_default().trim().parse().unwrap_or_else(|_| panic!("{} has an exit code", d.display()));
            let end = if code < 0 { End::Signalled(-code) } else { End::Exited(code) };
            out.push(Fixture {
                end,
                stdout: text(&d.join("stdout"), &runner, &folder),
                stderr: text(&d.join("stderr"), &runner, &folder),
                report: report(&d, &runner, &folder),
                cmd: std::fs::read_to_string(d.join("cmd")).unwrap_or_default().trim().to_string(),
                runner,
                folder,
            });
        }
    }
    out
}

/// The command as the recording ran it, without the recorder's docker wrapper.
fn command(cmd: &str) -> String {
    match cmd.strip_prefix("../../_bin/dock ") {
        Some(rest) => {
            let inner = rest.split_once(' ').map_or(rest, |(_, c)| c).trim();
            match inner.strip_prefix('\'') {
                Some(q) => q.split_once('\'').map_or(q, |(c, _)| c).to_string(),
                None => inner.to_string(),
            }
        }
        None => cmd.to_string(),
    }
}

/// git's blob at the run's commit, stood in for by the fixture's sources.
fn sources(runner: &str, folder: &str) -> impl Fn(&str) -> Option<Vec<u8>> {
    let dir = root().join("run_sources").join(runner).join(folder);
    move |p: &str| std::fs::read(dir.join(p)).ok()
}

/// What the binary keeps for a fixture: (location, missing name).
fn read(f: &Fixture) -> (Option<Location>, Option<String>) {
    let Some(t) = classify(f.end, &f.streams(), details_for(&command(&f.cmd))) else {
        return (None, None);
    };
    assert_eq!(t.location, t.candidates.first().cloned(), "{}/{}: location is the first candidate", f.runner, f.folder);
    (in_project(&t.candidates, &sources(&f.runner, &f.folder)), t.missing_name)
}

fn check(runner: &str) {
    let all: Vec<Fixture> = fixtures().into_iter().filter(|f| f.runner == runner).collect();
    assert!(!all.is_empty(), "{runner} has fixtures");
    let mut wrong = Vec::new();
    for f in &all {
        let Some((_, _, want)) = WANT.iter().find(|(r, d, _)| *r == f.runner && *d == f.folder) else {
            wrong.push(format!("{}/{}: not in WANT", f.runner, f.folder));
            continue;
        };
        let (loc, name) = read(f);
        let got = (loc.as_ref().map(|l| (l.file_path.as_str(), l.line, l.function.as_deref().unwrap_or(""))), name.as_deref());
        let ok = match want {
            At(file, line, function, n) => got == (Some((*file, *line, *function)), *n),
            NameOnly(n, _) => got == (None, Some(*n)),
            Nothing(_) => got == (None, None),
        };
        if !ok {
            wrong.push(format!("{}/{}: got {got:?}", f.runner, f.folder));
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

macro_rules! per_runner {
    ($($name:ident => $runner:literal),* $(,)?) => {
        $(
            #[test]
            fn $name() {
                check($runner);
            }
        )*
    };
}

per_runner! {
    cargo_nextest => "cargo-nextest",
    cargo_test => "cargo-test",
    catch2 => "catch2",
    ctest => "ctest",
    dotnet_mstest => "dotnet-mstest",
    dotnet_nunit => "dotnet-nunit",
    dotnet_xunit => "dotnet-xunit",
    go_test => "go-test",
    googletest => "googletest",
    gradle => "gradle",
    jest => "jest",
    maven => "maven",
    mocha => "mocha",
    node_test => "node-test",
    pest => "pest",
    phpunit => "phpunit",
    pytest => "pytest",
    sh => "sh",
    unittest => "unittest",
    vitest => "vitest",
}

#[test]
fn every_covered_fixture_has_a_row_and_every_row_a_fixture() {
    let all = fixtures();
    assert_eq!(all.len(), WANT.len(), "one row per fixture");
    for (r, d, want) in WANT {
        assert!(all.iter().any(|f| f.runner == *r && f.folder == *d), "{r}/{d} is recorded");
        if let NameOnly(_, why) | Nothing(why) = want {
            assert!(!why.is_empty(), "{r}/{d} says why nothing is extracted");
        }
    }
}

fn loc(file: &str, line: u64) -> Location {
    Location { file_path: file.to_string(), line, function: None, kind: None }
}

#[test]
fn only_a_tracked_file_inside_the_project_is_kept() {
    // a stand-in for a filesystem, not git: every path reads
    let any = |_: &str| -> Option<Vec<u8>> { Some(b"def f():\n    x\n    y\n".to_vec()) };
    for outside in [".venv/lib/python3.12/site-packages/x.py", "node_modules/lib/x.js", "a/node_modules/x.js", "/usr/lib/x.py", "../../.cache/x.py", "a/../b.py", ""] {
        assert_eq!(in_project(&[loc(outside, 2)], &any), None, "{outside}");
    }
    // past the file's end (a transformed module's line) or line 0: not the source's
    assert_eq!(in_project(&[loc("src/a.py", 4)], &any), None);
    assert_eq!(in_project(&[loc("src/a.py", 0)], &any), None);
    // the first candidate git tracks wins, with its function
    let git = |p: &str| -> Option<Vec<u8>> { (p == "src/a.py").then(|| b"def f():\n    x\n    y\n".to_vec()) };
    let got = in_project(&[loc("node_modules/x.js", 1), loc("src/b.py", 2), loc("src/a.py", 3)], &git);
    assert_eq!(got, Some(Location { file_path: "src/a.py".to_string(), line: 3, function: Some("def f():".to_string()), kind: None }));
}

#[test]
fn a_jvm_frame_is_found_under_its_package_in_the_source_folders() {
    let git = |p: &str| -> Option<Vec<u8>> { (p == "src/main/java/calc/Calc.java").then(|| b"package calc;\n\npublic class Calc {\n    int x;\n}\n".to_vec()) };
    let got = in_project(&[loc("calc/Calc.java", 4)], &git).expect("found");
    assert_eq!((got.file_path.as_str(), got.function.as_deref()), ("src/main/java/calc/Calc.java", Some("public class Calc {")), "no method encloses the line: git's header");
}

fn translation(label: &str, at: Option<Location>) -> TestLogTranslation {
    TestLogTranslation {
        run_id: "c1-r1".to_string(),
        criterion_id: "c1".to_string(),
        commit: "abc".to_string(),
        by: "regex".to_string(),
        label: label.to_string(),
        evidence: String::new(),
        failing_tests: Vec::new(),
        classified_on: "full".to_string(),
        location: at,
        missing_name: None,
        counts: None,
        ran_tests: None,
        skipped_tests: None,
        versions: None,
        frames: None,
    }
}

/// Plan §7.3 line 4: a crash's translation gives `place_chosen`'s place, the function git names
/// above its location and that function's lines, up to the line before the next header.
#[test]
fn a_crash_gives_a_place_chosen_candidate_from_its_function() {
    let cases: &[(&str, &str, &str, u64, &str, u64, u64)] = &[
        ("pytest", "tests_failed_exception", "src/calc.py", 7, "def parse(s):", 5, 8),
        ("go-test", "tests_failed_exception", "calc.go", 12, "func Parse(s string) int {", 9, 15),
        ("maven", "tests_failed_exception", "src/main/java/calc/Calc.java", 9, "public static int parse(String s) {", 8, 10),
        ("dotnet-xunit", "tests_failed_exception", "src/Calc.cs", 9, "public static int Parse(string s)", 7, 10),
        ("phpunit", "tests_failed_exception", "src/Calc.php", 17, "public static function parse(string $s): int", 14, 20),
        ("cargo-test", "tests_failed_exception", "src/calc.rs", 6, "pub fn parse(s: &str) -> i64 {", 5, 7),
    ];
    for (runner, folder, file, line, symbol, start, end) in cases {
        let f = fixtures().into_iter().find(|f| f.runner == *runner && f.folder == *folder).expect("recorded");
        let (at, _) = read(&f);
        assert_eq!(at.as_ref().map(|l| (l.file_path.as_str(), l.line)), Some((*file, *line)), "{runner}");
        let bytes = sources(runner, folder)(file).expect("the source");
        let got = place_candidate(&translation("tests_failed", at), &bytes);
        assert_eq!(got, Some(Place { file_path: file.to_string(), symbol: symbol.to_string(), line_start: *start, line_end: *end }), "{runner}");
    }
}

#[test]
fn only_a_crash_or_an_exception_gives_a_place() {
    let bytes = b"def f():\n    x\n";
    for label in ["missing_target_symbol", "collection_error", "unclassified", "passed"] {
        assert_eq!(place_candidate(&translation(label, Some(loc("a.py", 2))), bytes), None, "{label}");
    }
    assert_eq!(place_candidate(&translation("crashed", None), bytes), None, "no location");
    assert_eq!(place_candidate(&translation("crashed", Some(loc("a.py", 1))), b"  indented\n  x\n"), None, "no function above");
    assert!(place_candidate(&translation("crashed", Some(loc("a.py", 2))), bytes).is_some());
}

#[test]
fn a_functions_extent_runs_to_the_line_before_the_next_header_without_its_trailing_blanks() {
    let text = b"#!/bin/sh\nf() {\n  a\n}\n\n\ng() {\n  b\n}\n";
    assert_eq!(function_extent(text, 3), Some((2, "f() {".to_string(), 4)));
    assert_eq!(function_extent(text, 8), Some((7, "g() {".to_string(), 9)));
    assert_eq!(function_extent(text, 1), None, "the shebang starts with #");
}

/// A Java, C# or PHP method sits inside its class, so git's header above a line in it is the
/// class's: the binary reads the innermost method whose braces enclose the line instead, so an
/// exception in the planned method names that method (the place rule's `function`, plan §6.2).
/// (path, source, line, the method's header, its first line, its last line)
type MethodCase = (&'static str, &'static [u8], u64, &'static str, u64, u64);

#[test]
fn a_jvm_csharp_or_php_line_names_the_method_around_it_not_its_class() {
    let cases: &[MethodCase] = &[
        ("src/C.java", b"package c;\n\npublic class C {\n    int f(int a) {\n        if (a > 0) {\n            return a;\n        }\n        return 0;\n    }\n\n    int g() { return 1; }\n}\n", 8, "int f(int a) {", 4, 9),
        ("src/C.cs", b"namespace N;\n\npublic class C\n{\n    public int F(int a)\n    {\n        foreach (var x in xs)\n        {\n            Use(x);\n        }\n        return a;\n    }\n    public int G() => 1;\n}\n", 9, "public int F(int a)", 5, 12),
        ("src/C.cs", b"public class C\n{\n    public int G() => Boom();\n}\n", 3, "public int G() => Boom();", 3, 3),
        ("src/C.php", b"<?php\nfinal class C\n{\n    public function f(): int\n    {\n        while (true) {\n            throw new \\Exception('x');\n        }\n    }\n}\n", 7, "public function f(): int", 4, 9),
    ];
    for (path, bytes, line, header, start, end) in cases {
        let got = in_project(&[loc(path, *line)], &|p: &str| (p == *path).then(|| bytes.to_vec())).expect("in the project");
        assert_eq!(got.function.as_deref(), Some(*header), "{path}:{line}");
        let place = place_candidate(&translation("tests_failed", Some(got)), bytes);
        assert_eq!(place.map(|p| (p.symbol, p.line_start, p.line_end)), Some((header.to_string(), *start, *end)), "{path}:{line}");
    }
    // a line in no method (a field, the class's own line) keeps git's header
    let field = b"public class C {\n    int x = boom();\n}\n";
    assert_eq!(in_project(&[loc("C.java", 2)], &|_: &str| Some(field.to_vec())).and_then(|l| l.function), Some("public class C {".to_string()));
}
