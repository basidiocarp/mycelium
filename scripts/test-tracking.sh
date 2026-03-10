#!/usr/bin/env bash
# Test tracking end-to-end: run commands, verify they appear in mycelium gain --history
set -euo pipefail

# Workaround for macOS bash pipe handling in strict mode
set +e  # Allow errors in pipe chains to continue

PASS=0; FAIL=0; FAILURES=()
RED='\033[0;31m'; GREEN='\033[0;32m'; NC='\033[0m'

# =========================================================
#  Test helper
# =========================================================
check() {
    local name="$1" needle="$2"
    shift 2
    local output
    if output=$("$@" 2>&1) && echo "$output" | grep -q "$needle"; then
        PASS=$((PASS+1)); printf "  ${GREEN}PASS${NC}  %s\n" "$name"
    else
        FAIL=$((FAIL+1)); FAILURES+=("$name")
        printf "  ${RED}FAIL${NC}  %s\n" "$name"
        printf "        expected: '%s'\n" "$needle"
        printf "        got: %s\n" "$(echo "$output" | head -3)"
    fi
}

echo "═══ Mycelium Tracking Validation ═══"
echo ""

# =========================================================
#  Optimized commands (token savings)
# =========================================================
echo "── Optimized commands (token savings) ──"
mycelium ls . >/dev/null 2>&1
check "mycelium ls tracked" "mycelium ls" mycelium gain --history

mycelium git status >/dev/null 2>&1
check "mycelium git status tracked" "mycelium git status" mycelium gain --history

mycelium git log -5 >/dev/null 2>&1
check "mycelium git log tracked" "mycelium git log" mycelium gain --history

# =========================================================
#  Passthrough commands (timing-only)
# =========================================================
echo ""
echo "── Passthrough commands (timing-only) ──"
mycelium git tag --list >/dev/null 2>&1
check "git passthrough tracked" "git tag --list" mycelium gain --history

# =========================================================
#  GitHub CLI tracking
# =========================================================
echo ""
echo "── GitHub CLI tracking ──"
if command -v gh >/dev/null 2>&1 && gh auth status >/dev/null 2>&1; then
    mycelium gh pr list >/dev/null 2>&1 || true
    check "mycelium gh pr list tracked" "mycelium gh pr" mycelium gain --history

    mycelium gh run list >/dev/null 2>&1 || true
    check "mycelium gh run list tracked" "mycelium gh run" mycelium gain --history
else
    echo "  SKIP  gh (not authenticated)"
fi

# =========================================================
#  Stdin commands
# =========================================================
echo ""
echo "── Stdin commands ──"
echo -e "line1\nline2\nline1\nERROR: bad\nline1" | mycelium log >/dev/null 2>&1
check "mycelium log stdin tracked" "mycelium log" mycelium gain --history

# =========================================================
#  Summary integrity
# =========================================================
echo ""
echo "── Summary integrity ──"
output=$(mycelium gain 2>&1)
if echo "$output" | grep -q "Tokens saved"; then
    PASS=$((PASS+1)); printf "  ${GREEN}PASS${NC}  mycelium gain summary works\n"
else
    FAIL=$((FAIL+1)); printf "  ${RED}FAIL${NC}  mycelium gain summary\n"
fi

# =========================================================
#  Results
# =========================================================
echo ""
echo "═══ Results: ${PASS} passed, ${FAIL} failed ═══"
if [ ${#FAILURES[@]} -gt 0 ]; then
    echo "Failures: ${FAILURES[*]}"
fi
exit $FAIL
