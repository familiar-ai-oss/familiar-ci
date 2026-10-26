# sourced by _gen/<runner>.sh
R=/tmp/claude-1000/-home-user-projects-familiar-ai-oss/ce46f81f-ec40-4c7d-885d-547ef9d911b3/scratchpad/run_logs
# fx RUNNER OUTCOME : fresh fixture dir, cd into it
# ONLY="a b c" limits generation to those outcomes (others go to a scratch dir)
fx() { RUNNER=$1; OUTCOME=$2
  if [ -n "$ONLY" ] && ! printf ' %s ' "$ONLY" | grep -q " $2 "; then D=$(mktemp -d "$R/.cache/skip.XXXX"); cd "$D"; return 0; fi
  D="$R/$1/$2"; case "$D" in "$R"/?*/?*) rm -rf "$D";; *) echo bad; return 1;; esac; mkdir -p "$D"; cd "$D" || return 1; RUNNER=$1; OUTCOME=$2; }
setcmd() { printf '%s\n' "$1" > cmd; }
setver() { printf '%s\n' "$1" > version.cmd; }
setenv() { printf '%s\n' "$1" >> env.sh; }
setpre() { printf '%s\n' "$1" >> pre.sh; }
rec() {
cat > record.sh <<'EOR'
#!/bin/sh
# Re-run this fixture: `sh -c "$(cat cmd)"` here, re-capturing stdout, stderr,
# exit, status, version, elapsed (see ../../_bin/capture.py).
# pre.sh (if any) resets build state first; env.sh (if any) is sourced first.
cd "$(dirname "$0")" || exit 1
ROOT=$(cd ../.. && pwd); export ROOT
[ -f pre.sh ] && sh ./pre.sh
[ -f env.sh ] && . ./env.sh
python3 -I "$ROOT/_bin/capture.py" .
[ -f post.sh ] && sh ./post.sh
exit 0
EOR
chmod +x record.sh
}
# mk FILE <<'X' ... X  : write a fixture file (creating dirs)
mk() { mkdir -p "$(dirname "$1")"; cat > "$1"; }
# gen RUNNER [outcomes...] : (re)generate + record; with outcomes, only those
