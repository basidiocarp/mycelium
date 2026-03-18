#!/bin/sh
# session-summary hook — captures session metrics in hyphae on session end.
# Installed by: mycelium init --ecosystem
# Hook event: Stop (receives JSON on stdin with session_id, transcript_path, cwd)
#
# POSIX sh only — no bashisms. Must complete in <2 seconds.
# Always exits 0 to avoid blocking Claude Code shutdown.

# Graceful dependency checks — exit 0 if missing
command -v jq >/dev/null 2>&1 || exit 0
command -v hyphae >/dev/null 2>&1 || exit 0

# Read JSON from stdin
input=$(cat) || exit 0

# Extract fields
session_id=$(printf '%s' "$input" | jq -r '.session_id // empty' 2>/dev/null) || exit 0
transcript_path=$(printf '%s' "$input" | jq -r '.transcript_path // empty' 2>/dev/null) || exit 0
cwd=$(printf '%s' "$input" | jq -r '.cwd // empty' 2>/dev/null) || exit 0

# Need at least cwd to be useful
if [ -z "$cwd" ]; then
    exit 0
fi

project=$(basename "$cwd")

# Parse transcript for metrics (if transcript exists and is readable)
msg_count=0
files_modified=0
commands_run=0
error_count=0

if [ -n "$transcript_path" ] && [ -r "$transcript_path" ]; then
    msg_count=$(wc -l < "$transcript_path" 2>/dev/null | tr -d ' ') || msg_count=0

    files_modified=$(
        grep -c '"tool_name"[[:space:]]*:[[:space:]]*"Write\|"tool_name"[[:space:]]*:[[:space:]]*"Edit\|"tool_name"[[:space:]]*:[[:space:]]*"MultiEdit' \
            "$transcript_path" 2>/dev/null
    ) || files_modified=0

    commands_run=$(
        grep -c '"tool_name"[[:space:]]*:[[:space:]]*"Bash' \
            "$transcript_path" 2>/dev/null
    ) || commands_run=0

    error_count=$(
        grep -ci '"error"\|"Error"\|"ERROR"\|"failed"\|"Failed"\|"FAILED"\|"panic"' \
            "$transcript_path" 2>/dev/null
    ) || error_count=0
fi

# Build summary
sid_short=""
if [ -n "$session_id" ]; then
    sid_short=$(printf '%.8s' "$session_id")
fi

summary="Session${sid_short:+ $sid_short} in $project: ${msg_count} messages, ${files_modified} files modified, ${commands_run} commands run, ${error_count} errors"

# Store in hyphae (fire and forget, timeout after 2s)
hyphae store \
    --topic "session/$project" \
    --content "$summary" \
    --importance medium \
    -P "$project" \
    >/dev/null 2>&1 &

# Don't wait longer than 1 second for hyphae
HYPHAE_PID=$!
sleep 1 &
SLEEP_PID=$!
wait $SLEEP_PID 2>/dev/null
kill $HYPHAE_PID 2>/dev/null

exit 0
