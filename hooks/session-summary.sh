#!/bin/sh
# session-summary hook — captures rich semantic content from Claude Code transcript.
# Installed by: mycelium init --ecosystem
# Hook event: Stop (receives JSON on stdin with session_id, transcript_path, cwd)
#
# Extracts from JSONL transcript:
# - Task description (first user message)
# - Files modified (from Write/Edit tool calls)
# - Errors resolved (error→success patterns)
# - Tool usage (counts per tool type)
# - Key outcome (last assistant message summary)
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

# Initialize variables
task_description=""
files_modified=""
tool_counts=""
errors_resolved=0
key_outcome=""

# ─────────────────────────────────────────────────────────────────────────────
# Parse transcript if available
# ─────────────────────────────────────────────────────────────────────────────

if [ -n "$transcript_path" ] && [ -r "$transcript_path" ]; then
    # Extract task description (first user message, first 100 chars)
    task_description=$(
        jq -r '
            [.[] | select(.type == "human") | .text] | .[0]?
            | if . then . | gsub("\n"; " ") | .[0:100] else "Session work" end
        ' "$transcript_path" 2>/dev/null
    ) || task_description="Session work"

    # Extract file paths from Write/Edit tool calls
    files_modified=$(
        jq -r '
            [
                .[] | select(.type == "tool_use" and (.tool_name == "Write" or .tool_name == "Edit"))
                | .input.file_path?
            ] | unique | join(", ")
        ' "$transcript_path" 2>/dev/null
    ) || files_modified=""

    # Count tool usage by tool_name
    tool_counts=$(
        jq -r '
            [.[] | select(.type == "tool_use") | .tool_name] |
            group_by(.) |
            map({tool: .[0], count: length}) |
            sort_by(.count) | reverse |
            .[] |
            "\(.tool)(\(.count))"
            | @csv
        ' "$transcript_path" 2>/dev/null | tr '\n' ' ' | sed 's/,$//'
    ) || tool_counts=""

    # Count errors resolved: find tool_result with error followed by successful execution of same tool
    # For now, simple heuristic: count distinct tool_use_ids with error in results
    errors_resolved=$(
        jq '[.[] | select(.type == "tool_result" and (.content | test("error|Error|ERROR|failed|Failed|FAILED|panic"; "i"))) | .tool_use_id] | length' \
            "$transcript_path" 2>/dev/null
    ) || errors_resolved=0

    # Extract key outcome: last assistant message (first 150 chars)
    key_outcome=$(
        jq -r '
            [.[] | select(.type == "assistant") | .text] | .[-1]?
            | if . then . | gsub("\n"; " ") | .[0:150] else "Work completed" end
        ' "$transcript_path" 2>/dev/null
    ) || key_outcome="Work completed"
fi

# ─────────────────────────────────────────────────────────────────────────────
# Build summary
# ─────────────────────────────────────────────────────────────────────────────

# Ensure non-empty task description
if [ -z "$task_description" ]; then
    task_description="Session work"
fi

# Build memory content
summary="Session in $project: $task_description"

# Add files modified if present
if [ -n "$files_modified" ]; then
    summary="$summary
Files: $files_modified"
fi

# Add tool counts if present
if [ -n "$tool_counts" ]; then
    # Clean up CSV quotes
    tool_counts_clean=$(printf '%s' "$tool_counts" | sed 's/"//g')
    summary="$summary
Tools: $tool_counts_clean"
fi

# Add errors resolved if any
if [ "$errors_resolved" -gt 0 ]; then
    summary="$summary
Errors resolved: $errors_resolved"
fi

# Add key outcome if present
if [ -n "$key_outcome" ]; then
    summary="$summary
Outcome: $key_outcome"
fi

# ─────────────────────────────────────────────────────────────────────────────
# Store in hyphae (fire and forget, timeout after 1s)
# ─────────────────────────────────────────────────────────────────────────────

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
