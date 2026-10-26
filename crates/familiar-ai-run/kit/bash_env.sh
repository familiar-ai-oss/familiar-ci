# Familiar's bash trace (H-2), loaded by BASH_ENV in Familiar's own run only: bash read as
# bash, never as sh. Never prints, never forks (a trap that forks under functrace recurses
# without end: seen in the A0 spike). Side file $FAMILIAR_AI_SH_TRACE, tab-separated:
#   kind pid status file line command
#   err  : the ERR trap (a failing simple command outside if/while/&&/||/!)
#   ran  : at exit, the last 3 simple commands run, each with the status it left
#   fail : at exit, the last simple command that left a non-zero status
#   exit : at exit, the shell's status and the last command it ran
if [ -n "${FAMILIAR_AI_SH_TRACE-}" ]; then
  # Familiar's own xtrace (SHELLOPTS, for bash read as sh) is off here: this shell writes the
  # rows above instead, and its traps are not traced. A child read as sh still traces.
  case "${PS4-}" in "+familiar-ai"*) set +o xtrace ;; esac
  __fa_K=${BASH_SOURCE[0]} __fa_T=$'\t'
  __fa_p= __fa_pl= __fa_pc= __fa_r0= __fa_r1= __fa_r2= __fa_f=
  __fa_w() { local c=${3//$'\n'/\\n}; printf '%s\t%s\t%s\t%s\n' "$1" "$$" "$2" "$c" >> "$FAMILIAR_AI_SH_TRACE" 2>/dev/null; }
  # DEBUG runs before each simple command; $? is what the previous one left.
  # Entering a trap fires it once more with the interrupted command (same text, line
  # unchanged or 1): skipped, so the pending command keeps its true line.
  # (one line: inside a trap LINENO counts the handler's own lines)
  __fa_dbg='__fa_q=$?; if [[ ${BASH_SOURCE[0]-} != "$__fa_K" && ${FUNCNAME[0]-} != __fa_* && $BASH_COMMAND != __fa_* ]] && ! [[ $BASH_COMMAND == "$__fa_pc" && ( $LINENO == "$__fa_pl" || $LINENO == 1 ) ]]; then if [[ -n $__fa_p ]]; then __fa_r2=$__fa_r1 __fa_r1=$__fa_r0 __fa_r0=$__fa_q$__fa_T$__fa_p; [[ $__fa_q != 0 ]] && __fa_f=$__fa_r0; fi; __fa_pl=$LINENO __fa_pc=$BASH_COMMAND __fa_p=${BASH_SOURCE[0]:-$0}$__fa_T$LINENO$__fa_T$BASH_COMMAND; fi'
  __fa_exit() { trap - DEBUG; local x
    for x in "$__fa_r2" "$__fa_r1" "$__fa_r0"; do [[ -n $x ]] && __fa_w ran "${x%%$__fa_T*}" "${x#*$__fa_T}"; done
    [[ $1 != 0 && -n $__fa_p ]] && __fa_f=$1$__fa_T$__fa_p
    [[ -n $__fa_f ]] && __fa_w fail "${__fa_f%%$__fa_T*}" "${__fa_f#*$__fa_T}"
    __fa_w exit "$1" "$__fa_p"; }
  set -o errtrace -o functrace
  trap "$__fa_dbg" DEBUG
  trap '__fa_w err $? "${BASH_SOURCE[0]:-$0}$__fa_T$LINENO$__fa_T$BASH_COMMAND"' ERR
  trap '__fa_exit $?' EXIT
fi
# the person's own BASH_ENV, after the traps: one that sets its own traps replaces these (fewer rows,
# can't tell, never a wrong reading)
if [ -n "${FAMILIAR_AI_BASH_ENV-}" ]; then . "$FAMILIAR_AI_BASH_ENV"; fi
