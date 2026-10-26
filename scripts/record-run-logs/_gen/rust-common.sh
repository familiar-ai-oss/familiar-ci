. /tmp/claude-1000/-home-user-projects-familiar-ai-oss/ce46f81f-ec40-4c7d-885d-547ef9d911b3/scratchpad/run_logs/_gen/lib.sh
# RN = runner dir name, TC = test command, TV = version cmd, FILT = filter form
base() {
  fx "$RN" "$1"
  mk Cargo.toml <<'X'
[package]
name = "calc"
version = "0.1.0"
edition = "2021"

[dependencies]
X
  mk src/lib.rs <<'X'
pub mod calc;
X
  mk src/calc.rs <<'X'
pub fn add(a: i64, b: i64) -> i64 {
    a + b
}

pub fn parse(s: &str) -> i64 {
    s.parse::<i64>().unwrap()
}
X
  setpre 'rm -rf target'
  setver "$TV"
  setcmd "$TC"
}
okt() { mk tests/calc_test.rs <<'X'
use calc::calc::add;

#[test]
fn adds() {
    assert_eq!(add(2, 2), 4);
}
X
}
gen_all() {
base passed; okt; rec
base tests_failed; mk tests/calc_test.rs <<'X'
use calc::calc::add;

#[test]
fn adds() {
    assert_eq!(add(2, 2), 5);
}
X
rec
base tests_failed_exception; mk tests/calc_test.rs <<'X'
use calc::calc::parse;

#[test]
fn parses() {
    assert_eq!(parse("x1"), 1);
}
X
rec
base tests_failed_exception_backtrace; cp ../tests_failed_exception/tests -r . 2>/dev/null || mk tests/calc_test.rs <<'X'
use calc::calc::parse;

#[test]
fn parses() {
    assert_eq!(parse("x1"), 1);
}
X
setcmd "RUST_BACKTRACE=1 $TC"; rec
base collection_error; mk tests/calc_test.rs <<'X'
use calc::calc::add;

#[test]
fn adds() {
    assert_eq!(add(2, 2), 4)
    let x = 1;
}
X
rec
base collection_error_import; mk tests/calc_test.rs <<'X'
use calc::calc::add;
use calc::nosuchmod::helper;

#[test]
fn adds() {
    assert_eq!(add(2, 2), 4);
}
X
rec
base missing_target_symbol; mk tests/calc_test.rs <<'X'
use calc::calc;

#[test]
fn multiplies() {
    assert_eq!(calc::mul(2, 3), 6);
}
X
rec
base no_tests_collected_file; mk tests/calc_test.rs <<'X'
use calc::calc::add;

#[allow(dead_code)]
fn helper() -> i64 {
    add(1, 1)
}
X
rec
base no_tests_collected_filter; okt; setcmd "$TC nomatch"; rec
base setup_error_build_rs; okt; mk build.rs <<'X'
fn main() {
    panic!("setup failed: required tool not found");
}
X
rec
base usage_error; okt; setcmd "$TC --nosuchflag"; rec
base usage_error_harness; okt; setcmd "$TC -- --nosuchflag"; rec
base environment_error_cmd_not_found; okt; setcmd "$(echo "$TC" | sed "s/^cargo /cargox /")"; rec
base environment_error_missing_dep; okt; mk Cargo.toml <<'X'
[package]
name = "calc"
version = "0.1.0"
edition = "2021"

[dependencies]
helper = { path = "../nosuch-helper" }
X
rec
base runner_crash; okt; mk Cargo.toml <<'X'
[package
name = "calc"
version = "0.1.0"
edition = "2021"
X
rec
for k in segv segv_shell abort sigkill; do
  base killed_$k
  case $k in
    segv|segv_shell) body='unsafe { std::ptr::null_mut::<i32>().write_volatile(1); }';;
    abort) body='std::process::abort();';;
    sigkill) body='unsafe { raise(9); }';;
  esac
  mk tests/calc_test.rs <<X
use calc::calc::add;

extern "C" {
    fn raise(sig: i32) -> i32;
}

#[test]
fn adds() {
    assert_eq!(add(2, 2), 4);
}

#[test]
fn crashes() {
    $body
}
X
  [ $k = segv_shell ] && setcmd "$TC; exit \$?"
  rec
done
base big_run
python3 - <<'X'
import os
os.makedirs('tests', exist_ok=True)
s = 'use calc::calc::add;\n\n'
for i in range(600):
    want = -1 if i == 300 else i + 1
    s += f'#[test]\nfn add_{i:03}() {{\n    assert_eq!(add({i}, 1), {want});\n}}\n\n'
open('tests/calc_test.rs', 'w').write(s)
X
rec
}
