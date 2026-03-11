#!/usr/bin/env bash
# plugins/atmos.sh — Mycelium output filter for the atmos CLI
#
# Installation: place this file in your mycelium plugins/ directory.
# Mycelium will pipe atmos command output through this script automatically.
#
# The plugin receives raw stdout on stdin and writes filtered output to stdout.
# Set MYCELIUM_ORIGINAL_CMD to help the plugin detect the command type.

INPUT=$(cat)
[ -z "$INPUT" ] && exit 0

filter_terraform_plan() {
    echo "$INPUT" | grep -E "^\s+#|^Plan:|will be (created|updated|destroyed|replaced)|No changes"
}

filter_terraform_apply() {
    echo "$INPUT" | grep -E "complete after|Apply complete!|Error:|Warning:|No changes"
}

filter_json_yaml() {
    LINE_COUNT=$(echo "$INPUT" | wc -l)
    if [ "$LINE_COUNT" -le 60 ]; then
        echo "$INPUT"
    else
        echo "$INPUT" | head -60
        echo "... ($((LINE_COUNT - 60)) more lines truncated)"
    fi
}

filter_validate() {
    ERRORS=$(echo "$INPUT" | grep -iE "error|warning|failed|invalid")
    if [ -z "$ERRORS" ]; then
        echo "✓ Validation passed"
    else
        echo "$ERRORS"
    fi
}

# Route by explicit command hint first, then fall back to content detection
if [ -n "$MYCELIUM_ORIGINAL_CMD" ]; then
    case "$MYCELIUM_ORIGINAL_CMD" in
        *"terraform plan"*)   filter_terraform_plan; exit 0 ;;
        *"terraform apply"*)  filter_terraform_apply; exit 0 ;;
        *"terraform init"*)   echo "$INPUT"; exit 0 ;;
        *describe*)           filter_json_yaml; exit 0 ;;
        *validate*)           filter_validate; exit 0 ;;
    esac
fi

# Content-based fallback detection
FIRST_LINE=$(echo "$INPUT" | head -1)
if echo "$INPUT" | grep -q "^Plan:"; then
    filter_terraform_plan
elif echo "$INPUT" | grep -q "Apply complete!"; then
    filter_terraform_apply
elif echo "$FIRST_LINE" | grep -qE '^\{|\[|^---'; then
    filter_json_yaml
else
    echo "$INPUT"
fi
