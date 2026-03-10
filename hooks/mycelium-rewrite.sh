#!/usr/bin/env bash
# mycelium-hook-version: 2
# Mycelium Claude Code hook — rewrites commands to use mycelium for token savings.
# Requires: mycelium >= 0.23.0, jq
#
# This is a thin delegating hook: all rewrite logic lives in `mycelium rewrite`,
# which is the single source of truth (src/discover/registry.rs).
# To add or change rewrite rules, edit the Rust registry — not this file.

# =========================================================
#  Dependency guards
# =========================================================
if ! command -v jq &>/dev/null; then
  exit 0
fi

if ! command -v mycelium &>/dev/null; then
  exit 0
fi

# =========================================================
#  Version guard
# =========================================================
MYCELIUM_VERSION=$(mycelium --version 2>/dev/null | grep -oE '[0-9]+\.[0-9]+\.[0-9]+' | head -1)
if [ -n "$MYCELIUM_VERSION" ]; then
  MAJOR=$(echo "$MYCELIUM_VERSION" | cut -d. -f1)
  MINOR=$(echo "$MYCELIUM_VERSION" | cut -d. -f2)
  # Require >= 0.23.0
  if [ "$MAJOR" -eq 0 ] && [ "$MINOR" -lt 23 ]; then
    echo "[mycelium] WARNING: mycelium $MYCELIUM_VERSION is too old (need >= 0.23.0). Upgrade: cargo install mycelium" >&2
    exit 0
  fi
fi

# =========================================================
#  Parse input
# =========================================================
INPUT=$(cat)
CMD=$(echo "$INPUT" | jq -r '.tool_input.command // empty')

if [ -z "$CMD" ]; then
  exit 0
fi

# =========================================================
#  Delegate to mycelium rewrite
# =========================================================
REWRITTEN=$(mycelium rewrite "$CMD" 2>/dev/null) || exit 0

# No change — nothing to do.
if [ "$CMD" = "$REWRITTEN" ]; then
  exit 0
fi

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
