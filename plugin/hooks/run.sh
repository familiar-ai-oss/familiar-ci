#!/bin/sh
# Relays a Claude Code hook event (run.sh <Event>, JSON on stdin) to `familiar-ai hook <Event>`,
# and refuses a tool call when the binary answers with anything but 0 or 2, which Claude Code would let through.
# Without the binary, it allows only the exact setup commands and the question tool, denies every other tool call, and never blocks.

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
        command -v node >/dev/null 2>&1 || refuse_tool_call "$MSG"
        parsed=$(node -e '
            const e = JSON.parse(require("fs").readFileSync(0, "utf8"));
            const name = typeof e.tool_name === "string" ? e.tool_name : "";
            const cmd = e.tool_input && typeof e.tool_input.command === "string" ? e.tool_input.command : "";
            if (name.includes("\n")) process.exit(1);
            process.stdout.write(name + "\n" + cmd + ".");
        ' 2>/dev/null) || refuse_tool_call "$MSG"
        name=${parsed%%"
"*}
        cmd=${parsed#*"
"}
        cmd=${cmd%.}
        [ "$name" = AskUserQuestion ] && exit 0
        # The setup commands stay allowed so /familiar-ai:setup can install the missing binary.
        [ "$name" = Bash ] && is_setup_command "$cmd" && exit 0
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
