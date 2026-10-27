#!/bin/sh
# Relays a Claude Code hook event (run.sh <Event>, JSON on stdin) to `familiar-ai hook <Event>`,
# and refuses a tool call when the binary answers with anything but 0 or 2, which Claude Code would let through.
# Without the binary, it allows only the exact setup commands and the question tool, denies every other tool call, and never blocks, with the shell's base tools alone: no node.

MSG='familiar-ai: not installed yet, so nothing is checked · next: /familiar-ai:setup installs it'
EVENT="$1"
RELEASES=https://github.com/familiar-ai-oss/familiar-ci/releases
TARGETS="x86_64-unknown-linux-gnu aarch64-unknown-linux-gnu aarch64-apple-darwin x86_64-apple-darwin"

# By Claude Code's hook protocol the deny reason in this JSON reaches the agent and the systemMessage reaches the person; the walks check only the JSON printed here, not its delivery.
refuse_tool_call() {
    printf '{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"deny","permissionDecisionReason":"%s"},"systemMessage":"%s"}\n' "$1" "$1" && exit 0
    printf '%s\n' "$1" >&2
    exit 2
}

install_cmd() {
    D='$'
    printf '%s' "curl -fsSL $RELEASES/download/v$1/familiar-ai-$2.tar.gz -o /tmp/familiar-ai-$2.tar.gz && curl -fsSL $RELEASES/download/v$1/familiar-ai-$2.tar.gz.sha256 -o /tmp/familiar-ai-$2.tar.gz.sha256 && cd /tmp && $3 -c familiar-ai-$2.tar.gz.sha256 && mkdir -p \"${D}HOME/.local/bin\" && tar xzf familiar-ai-$2.tar.gz -C \"${D}HOME/.local/bin\" && chmod +x \"${D}HOME/.local/bin/familiar-ai\""
}

# The setup commands of plugin/skills/setup/SKILL.md are matched whole, so nothing appended or inserted runs unchecked.
is_setup_command() {
    case "$1" in
        "uname -s"|"uname -m"|"cargo install familiar-ai"|"familiar-ai --version"|"familiar-ai setup") return 0 ;;
    esac
    rest=${1#"curl -fsSL $RELEASES/download/v"}
    [ "$rest" != "$1" ] || return 1
    ver=${rest%%/*}
    case "$ver" in
        ""|*[!0-9a-zA-Z.-]*) return 1 ;;
    esac
    for target in $TARGETS; do
        for shatool in "sha256sum" "shasum -a 256"; do
            [ "$1" = "$(install_cmd "$ver" "$target" "$shatool")" ] && return 0
        done
    done
    return 1
}

# read_tool_call: the PreToolUse JSON on stdin, read with tr, sed and awk alone (Claude Code installed natively
# starts hooks with no node on PATH). It prints q for the question tool, b and the command for the Bash tool, and -
# for every other call and for anything that is not JSON. Each string is cut out whole, its \\ and \" set aside
# first, so nothing inside a string is read as the JSON's own; only the top level's tool_name and its tool_input's
# own command count, the last of each, as JSON.parse keeps them. A command with an escape other than \", or a byte
# other than printable ASCII, is no setup command, so it reads as -.
read_tool_call() {
    esc_bs=$(printf '\002') # stands for \\ while the strings are cut out
    esc_q=$(printf '\003')  # stands for \"
    # NUL and the two stand-ins become \001, which JSON never holds raw; a line break becomes a tab: still white
    # space between tokens, and still a byte no string may hold raw
    LC_ALL=C tr '\000\002\003\n\r' '\001\001\001\t\t' |
        # every setup command is short: a call past 64 KiB (one line by now) is refused unread, as a Mac's
        # sed and awk take seconds over 200 kB
        LC_ALL=C awk 'length($0) > 65536 { exit } { print }' |
        LC_ALL=C sed -e 's/\\\\/'"$esc_bs"'/g' -e 's/\\"/'"$esc_q"'/g' -e 's/"[^"]*"/\
&\
/g' |
        LC_ALL=C sed -e '/^"/!{' -e 's/[][{}:,]/\
&\
/g' -e 's/[[:blank:]][[:blank:]]*/\
/g' -e '}' |
        LC_ALL=C awk -v q="$esc_q" '
            # one token a line; want is what may come next: a value, a key, a colon, or what comes after a value
            function bad() { broken = 1; exit }
            # a value starts here: the top level members sit at depth 1, its tool_input object members at depth 2
            function value(t, is_string) {
                if (depth == 1 && key1 == "\"tool_name\"") name = is_string ? t : ""
                if (depth == 1 && key1 == "\"tool_input\"") { command = ""; input = (t == "{") }
                if (depth == 2 && input && key2 == "\"command\"") command = is_string ? t : ""
            }
            BEGIN { want = "value"; control = sprintf("[%c%c-%c]", 1, 4, 31) }
            $0 == "" { next }
            closed { bad() }
            $0 == "{" || $0 == "[" {
                if (want != "value" || (depth == 0 && $0 != "{")) bad()
                value($0, 0)
                stack = stack $0; depth++
                want = ($0 == "{") ? "key" : "value"; opened = 1
                next
            }
            $0 == "}" || $0 == "]" {
                if (substr(stack, depth, 1) != ($0 == "}" ? "{" : "[") || (want != "after" && !opened)) bad()
                if (depth == 2) input = 0
                stack = substr(stack, 1, depth - 1); depth--
                closed = (depth == 0); want = "after"; opened = 0
                next
            }
            { opened = 0 }
            $0 == ":" { if (want != "colon") bad(); want = "value"; next }
            $0 == "," { if (want != "after") bad(); want = (substr(stack, depth, 1) == "{") ? "key" : "value"; next }
            /^"/ {
                # a raw control byte, any but the two stand-ins, is not JSON inside a string
                if (length($0) < 2 || substr($0, length($0)) != "\"" || $0 ~ control) bad()
                if (want == "key") { if (depth == 1) key1 = $0; if (depth == 2) key2 = $0; want = "colon"; next }
                if (want != "value") bad()
                value($0, 1); want = "after"; next
            }
            {
                if (want != "value") bad()
                if ($0 != "true" && $0 != "false" && $0 != "null" && $0 !~ /^-?(0|[1-9][0-9]*)(\.[0-9]+)?([eE][-+]?[0-9]+)?$/) bad()
                value($0, 0); want = "after"
            }
            END {
                if (broken || !closed) name = ""
                if (name == "\"AskUserQuestion\"") { print "q"; exit }
                if (name != "\"Bash\"" || command == "") { print "-"; exit }
                c = substr(command, 2, length(command) - 2)
                gsub(q, "\"", c)
                if (c ~ /[^ -~]/ || index(c, "\\")) { print "-"; exit }
                print "b" c
            }'
}

# The push after a commit (plan §1.2): exit 2 wakes the agent with the result; nothing else blocks.
if [ "$EVENT" = await-run ]; then
    command -v familiar-ai >/dev/null 2>&1 || exit 0
    familiar-ai await-run
    [ "$?" = 2 ] && exit 2
    exit 0
fi

if command -v familiar-ai >/dev/null 2>&1; then
    CLAUDE_PLUGIN_ROOT="${CLAUDE_PLUGIN_ROOT:-$(cd "$(dirname "$0")/.." && pwd)}"
    export CLAUDE_PLUGIN_ROOT
    familiar-ai hook "$EVENT"
    rc=$?
    case "$rc" in
        0|2) exit "$rc" ;;
    esac
    BROKEN="familiar-ai did not answer (exit $rc), so nothing can be checked. Run /familiar-ai:setup to repair it."
    case "$EVENT" in
        PreToolUse) refuse_tool_call "$BROKEN" ;;
        *) printf '{"systemMessage": "%s"}\n' "$BROKEN"; exit 0 ;;
    esac
fi

case "$EVENT" in
    PreToolUse)
        parsed=$(read_tool_call)
        [ "$parsed" = q ] && exit 0
        # The setup commands stay allowed so /familiar-ai:setup can install the missing binary.
        case "$parsed" in
            b*) is_setup_command "${parsed#b}" && exit 0 ;;
        esac
        refuse_tool_call "$MSG"
        ;;
    SessionStart|UserPromptSubmit|Stop|SubagentStop)
        # A blocked stop would re-prompt the agent forever and a blocked prompt would lock the person out of typing /familiar-ai:setup.
        cat >/dev/null 2>&1 || :
        printf '{"systemMessage": "%s"}\n' "$MSG"
        exit 0
        ;;
    *)
        cat >/dev/null 2>&1 || :
        exit 0
        ;;
esac
