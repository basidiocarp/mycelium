#!/usr/bin/env bash
# mycelium-hook-version: 3
# Mycelium Claude Code hook — rewrites commands to use mycelium for token savings.
# Requires: mycelium >= 0.1.0, jq
#
# This is a thin delegating hook: all rewrite logic lives in `mycelium rewrite`,
# which is the single source of truth (src/discover/registry.rs).
# To add or change rewrite rules, edit the Rust registry — not this file.

# =========================================================
#  Audit logging (opt-in via MYCELIUM_HOOK_AUDIT=1)
# =========================================================
_mycelium_audit_log() {
  if [ "${MYCELIUM_HOOK_AUDIT:-0}" != "1" ]; then return; fi
  local action="$1" original="$2" rewritten="${3:--}"
  local dir="${MYCELIUM_AUDIT_DIR:-${HOME}/.local/share/mycelium}"
  mkdir -p "$dir"
  printf '%s | %s | %s | %s\n' \
    "$(date -u +%Y-%m-%dT%H:%M:%SZ)" "$action" "$original" "$rewritten" \
    >> "${dir}/hook-audit.log"
}

# =========================================================
#  Dependency guards
# =========================================================
if ! command -v mycelium &>/dev/null || ! command -v jq &>/dev/null; then
  _mycelium_audit_log "skip:no_deps" "-"
  exit 0
fi

# =========================================================
#  Version guard
# =========================================================
MYCELIUM_VERSION=$(mycelium --version 2>/dev/null | grep -oE '[0-9]+\.[0-9]+\.[0-9]+' | head -1)
if [ -n "$MYCELIUM_VERSION" ]; then
  MAJOR=$(echo "$MYCELIUM_VERSION" | cut -d. -f1)
  MINOR=$(echo "$MYCELIUM_VERSION" | cut -d. -f2)
  # Require >= 0.1.0
  if [ "$MAJOR" -eq 0 ] && [ "$MINOR" -lt 1 ]; then
    echo "[mycelium] WARNING: mycelium $MYCELIUM_VERSION is too old (need >= 0.1.0). Upgrade: cargo install --path ." >&2
    _mycelium_audit_log "skip:old_version" "$MYCELIUM_VERSION"
    exit 0
  fi
fi

set -euo pipefail

# =========================================================
#  Parse input
# =========================================================
INPUT=$(cat)
CMD=$(echo "$INPUT" | jq -r '.tool_input.command // empty' 2>/dev/null) || {
  _mycelium_audit_log "skip:jq_parse_error" "-"
  exit 0
}

if [ -z "$CMD" ]; then
  _mycelium_audit_log "skip:empty" "-"
  exit 0
fi

# Skip heredocs (mycelium rewrite also skips them, but bail early)
case "$CMD" in
  *'<<'*) _mycelium_audit_log "skip:heredoc" "$CMD"; exit 0 ;;
esac

# =========================================================
#  Delegate to mycelium rewrite
# =========================================================
REWRITTEN=$(mycelium rewrite "$CMD" 2>/dev/null) || {
  _mycelium_audit_log "skip:no_match" "$CMD"
  exit 0
}

# No change — nothing to do.
if [ "$CMD" = "$REWRITTEN" ]; then
  _mycelium_audit_log "skip:already_mycelium" "$CMD"
  exit 0
fi

_mycelium_audit_log "rewrite" "$CMD" "$REWRITTEN"

# =========================================================
#  Output rewrite instruction
# =========================================================
ORIGINAL_INPUT=$(echo "$INPUT" | jq -c '.tool_input')
UPDATED_INPUT=$(echo "$ORIGINAL_INPUT" | jq --arg cmd "$REWRITTEN" '.command = $cmd')

jq -n \
  --argjson updated "$UPDATED_INPUT" \
  '{
    "hookSpecificOutput": {
      "hookEventName": "PreToolUse",
      "permissionDecision": "allow",
      "permissionDecisionReason": "Mycelium auto-rewrite",
      "updatedInput": $updated
    }
  }'
