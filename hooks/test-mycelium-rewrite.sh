#!/bin/bash
# Test suite for mycelium-rewrite.sh
# Feeds mock JSON through the hook and verifies the rewritten commands.
#
# Usage: bash ~/.claude/hooks/test-mycelium-rewrite.sh

HOOK="${HOOK:-$HOME/.claude/hooks/mycelium-rewrite.sh}"
PASS=0
FAIL=0
TOTAL=0

# =========================================================
#  Colors
# =========================================================
GREEN='\033[32m'
RED='\033[31m'
DIM='\033[2m'
RESET='\033[0m'

# =========================================================
#  Test helper
# =========================================================
test_rewrite() {
  local description="$1"
  local input_cmd="$2"
  local expected_cmd="$3"  # empty string = expect no rewrite
  TOTAL=$((TOTAL + 1))

  local input_json
  input_json=$(jq -n --arg cmd "$input_cmd" '{"tool_name":"Bash","tool_input":{"command":$cmd}}')
  local output
  output=$(echo "$input_json" | bash "$HOOK" 2>/dev/null) || true

  if [ -z "$expected_cmd" ]; then
    # Expect no rewrite (hook exits 0 with no output)
    if [ -z "$output" ]; then
      printf "  ${GREEN}PASS${RESET} %s ${DIM}→ (no rewrite)${RESET}\n" "$description"
      PASS=$((PASS + 1))
    else
      local actual
      actual=$(echo "$output" | jq -r '.hookSpecificOutput.updatedInput.command // empty')
      printf "  ${RED}FAIL${RESET} %s\n" "$description"
      printf "       expected: (no rewrite)\n"
      printf "       actual:   %s\n" "$actual"
      FAIL=$((FAIL + 1))
    fi
  else
    local actual
    actual=$(echo "$output" | jq -r '.hookSpecificOutput.updatedInput.command // empty' 2>/dev/null)
    if [ "$actual" = "$expected_cmd" ]; then
      printf "  ${GREEN}PASS${RESET} %s ${DIM}→ %s${RESET}\n" "$description" "$actual"
      PASS=$((PASS + 1))
    else
      printf "  ${RED}FAIL${RESET} %s\n" "$description"
      printf "       expected: %s\n" "$expected_cmd"
      printf "       actual:   %s\n" "$actual"
      FAIL=$((FAIL + 1))
    fi
  fi
}

echo "============================================"
echo "  Mycelium Rewrite Hook Test Suite"
echo "============================================"
echo ""

# =========================================================
#  1. Existing patterns (regression tests)
# =========================================================
echo "--- Existing patterns (regression) ---"
test_rewrite "git status" \
  "git status" \
  "mycelium git status"

test_rewrite "git log --oneline -10" \
  "git log --oneline -10" \
  "mycelium git log --oneline -10"

test_rewrite "git diff HEAD" \
  "git diff HEAD" \
  "mycelium git diff HEAD"

test_rewrite "git show abc123" \
  "git show abc123" \
  "mycelium git show abc123"

test_rewrite "git add ." \
  "git add ." \
  "mycelium git add ."

test_rewrite "gh pr list" \
  "gh pr list" \
  "mycelium gh pr list"

test_rewrite "npx playwright test" \
  "npx playwright test" \
  "mycelium playwright test"

test_rewrite "ls -la" \
  "ls -la" \
  "mycelium ls -la"

test_rewrite "curl -s https://example.com" \
  "curl -s https://example.com" \
  "mycelium curl -s https://example.com"

test_rewrite "cat package.json" \
  "cat package.json" \
  "mycelium read package.json"

test_rewrite "grep -rn pattern src/" \
  "grep -rn pattern src/" \
  "mycelium grep -rn pattern src/"

test_rewrite "rg pattern src/" \
  "rg pattern src/" \
  "mycelium grep pattern src/"

test_rewrite "cargo test" \
  "cargo test" \
  "mycelium cargo test"

test_rewrite "npx prisma migrate" \
  "npx prisma migrate" \
  "mycelium prisma migrate"

echo ""

# =========================================================
#  2. Env var prefix handling (THE BIG FIX)
# =========================================================
echo "--- Env var prefix handling (new) ---"
test_rewrite "env + playwright" \
  "TEST_SESSION_ID=2 npx playwright test --config=foo" \
  "TEST_SESSION_ID=2 mycelium playwright test --config=foo"

test_rewrite "env + git status" \
  "GIT_PAGER=cat git status" \
  "GIT_PAGER=cat mycelium git status"

test_rewrite "env + git log" \
  "GIT_PAGER=cat git log --oneline -10" \
  "GIT_PAGER=cat mycelium git log --oneline -10"

test_rewrite "multi env + vitest" \
  "NODE_ENV=test CI=1 npx vitest run" \
  "NODE_ENV=test CI=1 mycelium vitest run"

test_rewrite "env + ls" \
  "LANG=C ls -la" \
  "LANG=C mycelium ls -la"

test_rewrite "env + npm run" \
  "NODE_ENV=test npm run test:e2e" \
  "NODE_ENV=test mycelium npm test:e2e"

test_rewrite "env + docker compose (unsupported subcommand, NOT rewritten)" \
  "COMPOSE_PROJECT_NAME=test docker compose up -d" \
  ""

test_rewrite "env + docker compose logs (supported, rewritten)" \
  "COMPOSE_PROJECT_NAME=test docker compose logs web" \
  "COMPOSE_PROJECT_NAME=test mycelium docker compose logs web"

echo ""

# =========================================================
#  3. New patterns
# =========================================================
echo "--- New patterns ---"
test_rewrite "npm run test:e2e" \
  "npm run test:e2e" \
  "mycelium npm test:e2e"

test_rewrite "npm run build" \
  "npm run build" \
  "mycelium npm build"

test_rewrite "npm test" \
  "npm test" \
  "mycelium npm test"

test_rewrite "vue-tsc -b" \
  "vue-tsc -b" \
  "mycelium tsc -b"

test_rewrite "npx vue-tsc --noEmit" \
  "npx vue-tsc --noEmit" \
  "mycelium tsc --noEmit"

test_rewrite "docker compose up -d (NOT rewritten — unsupported by mycelium)" \
  "docker compose up -d" \
  ""

test_rewrite "docker compose logs postgrest" \
  "docker compose logs postgrest" \
  "mycelium docker compose logs postgrest"

test_rewrite "docker compose ps" \
  "docker compose ps" \
  "mycelium docker compose ps"

test_rewrite "docker compose build" \
  "docker compose build" \
  "mycelium docker compose build"

test_rewrite "docker compose down (NOT rewritten — unsupported by mycelium)" \
  "docker compose down" \
  ""

test_rewrite "docker compose -f file.yml up (NOT rewritten — flag before subcommand)" \
  "docker compose -f docker-compose.preview.yml --project-name myapp up -d --build" \
  ""

test_rewrite "docker run --rm postgres" \
  "docker run --rm postgres" \
  "mycelium docker run --rm postgres"

test_rewrite "docker exec -it db psql" \
  "docker exec -it db psql" \
  "mycelium docker exec -it db psql"

test_rewrite "find (NOT rewritten — different arg format)" \
  "find . -name '*.ts'" \
  ""

test_rewrite "tree (NOT rewritten — different arg format)" \
  "tree src/" \
  ""

test_rewrite "wget (NOT rewritten — different arg format)" \
  "wget https://example.com/file" \
  ""

test_rewrite "gh api repos/owner/repo" \
  "gh api repos/owner/repo" \
  "mycelium gh api repos/owner/repo"

test_rewrite "gh release list" \
  "gh release list" \
  "mycelium gh release list"

test_rewrite "kubectl describe pod foo" \
  "kubectl describe pod foo" \
  "mycelium kubectl describe pod foo"

test_rewrite "kubectl apply -f deploy.yaml" \
  "kubectl apply -f deploy.yaml" \
  "mycelium kubectl apply -f deploy.yaml"

echo ""

# =========================================================
#  3b. MYCELIUM_DISABLED and redirect fixes
# =========================================================
echo "--- MYCELIUM_DISABLED (#345) ---"
test_rewrite "MYCELIUM_DISABLED=1 git status (no rewrite)" \
  "MYCELIUM_DISABLED=1 git status" \
  ""

test_rewrite "MYCELIUM_DISABLED=1 cargo test (no rewrite)" \
  "MYCELIUM_DISABLED=1 cargo test" \
  ""

test_rewrite "FOO=1 MYCELIUM_DISABLED=1 git status (no rewrite)" \
  "FOO=1 MYCELIUM_DISABLED=1 git status" \
  ""

echo ""
echo "--- Redirect operators (#346) ---"
test_rewrite "cargo test 2>&1 | head" \
  "cargo test 2>&1 | head" \
  "mycelium cargo test 2>&1 | head"

test_rewrite "cargo test 2>&1" \
  "cargo test 2>&1" \
  "mycelium cargo test 2>&1"

test_rewrite "cargo test &>/dev/null" \
  "cargo test &>/dev/null" \
  "mycelium cargo test &>/dev/null"

# Note: the bash hook rewrites only the first command segment (sed-based);
# full compound rewriting (both sides of &) is handled by `mycelium rewrite` (Rust).
# The critical behavior tested here: `&` after `cargo test` is NOT mistaken for
# a redirect — the hook still rewrites cargo test, no crash.
test_rewrite "cargo test & git status (bash hook rewrites first segment only)" \
  "cargo test & git status" \
  "mycelium cargo test & git status"

echo ""

# =========================================================
#  4. Vitest edge case (fixed double "run" bug)
# =========================================================
echo "--- Vitest run dedup ---"
test_rewrite "vitest (no args)" \
  "vitest" \
  "mycelium vitest run"

test_rewrite "vitest run (no double run)" \
  "vitest run" \
  "mycelium vitest run"

test_rewrite "vitest run --reporter" \
  "vitest run --reporter=verbose" \
  "mycelium vitest run --reporter=verbose"

test_rewrite "npx vitest run" \
  "npx vitest run" \
  "mycelium vitest run"

test_rewrite "pnpm vitest run --coverage" \
  "pnpm vitest run --coverage" \
  "mycelium vitest run --coverage"

echo ""

# =========================================================
#  5. Should NOT rewrite
# =========================================================
echo "--- Should NOT rewrite ---"
test_rewrite "already mycelium" \
  "mycelium git status" \
  ""

test_rewrite "heredoc" \
  "cat <<'EOF'
hello
EOF" \
  ""

test_rewrite "echo (no pattern)" \
  "echo hello world" \
  ""

test_rewrite "cd (no pattern)" \
  "cd /tmp" \
  ""

test_rewrite "mkdir (no pattern)" \
  "mkdir -p foo/bar" \
  ""

test_rewrite "python3 (no pattern)" \
  "python3 script.py" \
  ""

test_rewrite "node (no pattern)" \
  "node -e 'console.log(1)'" \
  ""

echo ""

# =========================================================
#  6. Audit logging
# =========================================================
echo "--- Audit logging (MYCELIUM_HOOK_AUDIT=1) ---"

AUDIT_TMPDIR=$(mktemp -d)
trap "rm -rf $AUDIT_TMPDIR" EXIT

test_audit_log() {
  local description="$1"
  local input_cmd="$2"
  local expected_action="$3"
  TOTAL=$((TOTAL + 1))

  # Clean log
  rm -f "$AUDIT_TMPDIR/hook-audit.log"

  local input_json
  input_json=$(jq -n --arg cmd "$input_cmd" '{"tool_name":"Bash","tool_input":{"command":$cmd}}')
  echo "$input_json" | MYCELIUM_HOOK_AUDIT=1 MYCELIUM_AUDIT_DIR="$AUDIT_TMPDIR" bash "$HOOK" 2>/dev/null || true

  if [ ! -f "$AUDIT_TMPDIR/hook-audit.log" ]; then
    printf "  ${RED}FAIL${RESET} %s (no log file created)\n" "$description"
    FAIL=$((FAIL + 1))
    return
  fi

  local log_line
  log_line=$(head -1 "$AUDIT_TMPDIR/hook-audit.log")
  local actual_action
  actual_action=$(echo "$log_line" | cut -d'|' -f2 | tr -d ' ')

  if [ "$actual_action" = "$expected_action" ]; then
    printf "  ${GREEN}PASS${RESET} %s ${DIM}→ %s${RESET}\n" "$description" "$actual_action"
    PASS=$((PASS + 1))
  else
    printf "  ${RED}FAIL${RESET} %s\n" "$description"
    printf "       expected action: %s\n" "$expected_action"
    printf "       actual action:   %s\n" "$actual_action"
    printf "       log line:        %s\n" "$log_line"
    FAIL=$((FAIL + 1))
  fi
}

test_audit_log "audit: rewrite git status" \
  "git status" \
  "rewrite"

test_audit_log "audit: skip already_mycelium" \
  "mycelium git status" \
  "skip:already_mycelium"

test_audit_log "audit: skip heredoc" \
  "cat <<'EOF'
hello
EOF" \
  "skip:heredoc"

test_audit_log "audit: skip no_match" \
  "echo hello world" \
  "skip:no_match"

test_audit_log "audit: rewrite cargo test" \
  "cargo test" \
  "rewrite"

# =========================================================
#  6b. Audit log format validation
# =========================================================
rm -f "$AUDIT_TMPDIR/hook-audit.log"
input_json=$(jq -n --arg cmd "git status" '{"tool_name":"Bash","tool_input":{"command":$cmd}}')
echo "$input_json" | MYCELIUM_HOOK_AUDIT=1 MYCELIUM_AUDIT_DIR="$AUDIT_TMPDIR" bash "$HOOK" 2>/dev/null || true
TOTAL=$((TOTAL + 1))
log_line=$(cat "$AUDIT_TMPDIR/hook-audit.log" 2>/dev/null || echo "")
field_count=$(echo "$log_line" | awk -F' \\| ' '{print NF}')
if [ "$field_count" = "4" ]; then
  printf "  ${GREEN}PASS${RESET} audit: log format has 4 fields ${DIM}→ %s${RESET}\n" "$log_line"
  PASS=$((PASS + 1))
else
  printf "  ${RED}FAIL${RESET} audit: log format (expected 4 fields, got %s)\n" "$field_count"
  printf "       log line: %s\n" "$log_line"
  FAIL=$((FAIL + 1))
fi

# Test no log when MYCELIUM_HOOK_AUDIT is unset
rm -f "$AUDIT_TMPDIR/hook-audit.log"
input_json=$(jq -n --arg cmd "git status" '{"tool_name":"Bash","tool_input":{"command":$cmd}}')
echo "$input_json" | MYCELIUM_AUDIT_DIR="$AUDIT_TMPDIR" bash "$HOOK" 2>/dev/null || true
TOTAL=$((TOTAL + 1))
if [ ! -f "$AUDIT_TMPDIR/hook-audit.log" ]; then
  printf "  ${GREEN}PASS${RESET} audit: no log when MYCELIUM_HOOK_AUDIT unset\n"
  PASS=$((PASS + 1))
else
  printf "  ${RED}FAIL${RESET} audit: log created when MYCELIUM_HOOK_AUDIT unset\n"
  FAIL=$((FAIL + 1))
fi

echo ""

# =========================================================
#  Summary
# =========================================================
echo "============================================"
if [ $FAIL -eq 0 ]; then
  printf "  ${GREEN}ALL $TOTAL TESTS PASSED${RESET}\n"
else
  printf "  ${RED}$FAIL FAILED${RESET} / $TOTAL total ($PASS passed)\n"
fi
echo "============================================"

exit $FAIL
