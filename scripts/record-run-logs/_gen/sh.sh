. /tmp/claude-1000/-home-user-projects-familiar-ai-oss/ce46f81f-ec40-4c7d-885d-547ef9d911b3/scratchpad/run_logs/_gen/lib.sh
# the `sh` runner (the owner, 2026-10-15): a test that is a shell script
base() {  # $1 outcome
  fx sh "$1"
  mk src/app.sh <<'X'
#!/bin/sh
greet() { echo hi; }
greet
X
  setver 'sh --version 2>/dev/null || readlink -f /bin/sh'
  setcmd 'sh tests/test_app.sh'
}
base passed; mk tests/test_app.sh <<'X'
grep -q hi src/app.sh
X
rec
base tests_failed; mk tests/test_app.sh <<'X'
grep -q hello src/app.sh
X
rec
base missing_target_symbol; mk tests/test_app.sh <<'X'
. src/app.sh >/dev/null
farewell
X
rec
base usage_error; mk tests/test_app.sh <<'X'
if grep -q hi src/app.sh; then
  echo ok
X
rec
