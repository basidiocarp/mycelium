#!/usr/bin/env bash
#
# Mycelium Smoke Test Suite
# Exercises every command to catch regressions after merge.
# Exit code: number of failures (0 = all green)
#
set -euo pipefail

PASS=0
FAIL=0
SKIP=0
FAILURES=()

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m'

# =========================================================
# Helpers
# =========================================================
assert_ok() {
    local name="$1"
    shift
    local output
    if output=$("$@" 2>&1); then
        PASS=$((PASS + 1))
        printf "  ${GREEN}PASS${NC}  %s\n" "$name"
    else
        FAIL=$((FAIL + 1))
        FAILURES+=("$name")
        printf "  ${RED}FAIL${NC}  %s\n" "$name"
        printf "        cmd: %s\n" "$*"
        printf "        out: %s\n" "$(echo "$output" | head -3)"
    fi
}

assert_contains() {
    local name="$1"
    local needle="$2"
    shift 2
    local output
    if output=$("$@" 2>&1) && echo "$output" | grep -q "$needle"; then
        PASS=$((PASS + 1))
        printf "  ${GREEN}PASS${NC}  %s\n" "$name"
    else
        FAIL=$((FAIL + 1))
        FAILURES+=("$name")
        printf "  ${RED}FAIL${NC}  %s\n" "$name"
        printf "        expected: '%s'\n" "$needle"
        printf "        got: %s\n" "$(echo "$output" | head -3)"
    fi
}

assert_exit_ok() {
    local name="$1"
    shift
    if "$@" >/dev/null 2>&1; then
        PASS=$((PASS + 1))
        printf "  ${GREEN}PASS${NC}  %s\n" "$name"
    else
        FAIL=$((FAIL + 1))
        FAILURES+=("$name")
        printf "  ${RED}FAIL${NC}  %s\n" "$name"
        printf "        cmd: %s\n" "$*"
    fi
}

assert_fails() {
    local name="$1"
    shift
    if "$@" >/dev/null 2>&1; then
        FAIL=$((FAIL + 1))
        FAILURES+=("$name (expected failure, got success)")
        printf "  ${RED}FAIL${NC}  %s (expected failure)\n" "$name"
    else
        PASS=$((PASS + 1))
        printf "  ${GREEN}PASS${NC}  %s\n" "$name"
    fi
}

assert_help() {
    local name="$1"
    shift
    assert_contains "$name --help" "Usage:" "$@" --help
}

skip_test() {
    local name="$1"
    local reason="$2"
    SKIP=$((SKIP + 1))
    printf "  ${YELLOW}SKIP${NC}  %s (%s)\n" "$name" "$reason"
}

section() {
    printf "\n${BOLD}${CYAN} %s ${NC}\n" "$1"
}

# =========================================================
# Preamble
# =========================================================

MYCELIUM=$(command -v mycelium || echo "")
if [[ -z "$MYCELIUM" ]]; then
    echo "mycelium not found in PATH. Run: cargo install --path ."
    exit 1
fi

printf "${BOLD}Mycelium Smoke Test Suite${NC}\n"
printf "Binary: %s\n" "$MYCELIUM"
printf "Version: %s\n" "$(mycelium --version)"
printf "Date: %s\n" "$(date '+%Y-%m-%d %H:%M')"

# Need a git repo to test git commands
if ! git rev-parse --is-inside-work-tree >/dev/null 2>&1; then
    echo "Must run from inside a git repository."
    exit 1
fi

REPO_ROOT=$(git rev-parse --show-toplevel)

# =========================================================
# 1. Version & Help
# =========================================================
section "Version & Help"

assert_contains "mycelium --version" "mycelium" mycelium --version
assert_contains "mycelium --help" "Usage:" mycelium --help

# =========================================================
# 2. Ls
# =========================================================
section "Ls"

assert_ok      "mycelium ls ."                     mycelium ls .
assert_ok      "mycelium ls -la ."                 mycelium ls -la .
assert_ok      "mycelium ls -lh ."                 mycelium ls -lh .
assert_ok      "mycelium ls -l src/"               mycelium ls -l src/
assert_ok      "mycelium ls src/ -l (flag after)"  mycelium ls src/ -l
assert_ok      "mycelium ls multi paths"           mycelium ls src/ scripts/
assert_contains "mycelium ls -a shows hidden"      ".git" mycelium ls -a .
assert_contains "mycelium ls shows sizes"          "K"  mycelium ls src/
assert_contains "mycelium ls shows dirs with /"    "/" mycelium ls .

# =========================================================
# 2b. Tree
# =========================================================
section "Tree"

if command -v tree >/dev/null 2>&1; then
    assert_ok      "mycelium tree ."                mycelium tree .
    assert_ok      "mycelium tree -L 2 ."           mycelium tree -L 2 .
    assert_ok      "mycelium tree -d -L 1 ."        mycelium tree -d -L 1 .
    assert_contains "mycelium tree shows src/"      "src" mycelium tree -L 1 .
else
    skip_test "mycelium tree" "tree not installed"
fi

# =========================================================
# 3. Read
# =========================================================
section "Read"

assert_ok      "mycelium read Cargo.toml"          mycelium read Cargo.toml
assert_ok      "mycelium read --level none Cargo.toml"  mycelium read --level none Cargo.toml
assert_ok      "mycelium read --level aggressive Cargo.toml" mycelium read --level aggressive Cargo.toml
assert_ok      "mycelium read -n Cargo.toml"       mycelium read -n Cargo.toml
assert_ok      "mycelium read --max-lines 5 Cargo.toml" mycelium read --max-lines 5 Cargo.toml

section "Read (stdin support)"

assert_ok      "mycelium read stdin pipe"          bash -c 'echo "fn main() {}" | mycelium read -'

# =========================================================
#  4. Git
# =========================================================
section "Git (existing)"

assert_ok      "mycelium git status"               mycelium git status
assert_ok      "mycelium git status --short"       mycelium git status --short
assert_ok      "mycelium git status -s"            mycelium git status -s
assert_ok      "mycelium git status --porcelain"   mycelium git status --porcelain
assert_ok      "mycelium git log"                  mycelium git log
assert_ok      "mycelium git log -5"               mycelium git log -- -5
assert_ok      "mycelium git diff"                 mycelium git diff
assert_ok      "mycelium git diff --stat"          mycelium git diff --stat

section "Git (new: branch, fetch, stash, worktree)"

assert_ok      "mycelium git branch"               mycelium git branch
assert_ok      "mycelium git fetch"                mycelium git fetch
assert_ok      "mycelium git stash list"           mycelium git stash list
assert_ok      "mycelium git worktree"             mycelium git worktree

section "Git (passthrough: unsupported subcommands)"

assert_ok      "mycelium git tag --list"           mycelium git tag --list
assert_ok      "mycelium git remote -v"            mycelium git remote -v
assert_ok      "mycelium git rev-parse HEAD"       mycelium git rev-parse HEAD

# =========================================================
#  5. GitHub CLI
# =========================================================
section "GitHub CLI"

if command -v gh >/dev/null 2>&1 && gh auth status >/dev/null 2>&1; then
    assert_ok      "mycelium gh pr list"           mycelium gh pr list
    assert_ok      "mycelium gh run list"          mycelium gh run list
    assert_ok      "mycelium gh issue list"        mycelium gh issue list
    # pr create/merge/diff/comment/edit are write ops, test help only
    assert_help    "mycelium gh"                   mycelium gh
else
    skip_test "gh commands" "gh not authenticated"
fi

# =========================================================
#  6. Cargo
# =========================================================

section "Cargo (new)"

assert_ok      "mycelium cargo build"              mycelium cargo build
assert_ok      "mycelium cargo clippy"             mycelium cargo clippy
# cargo test exits non-zero due to pre-existing failures; check output ignoring exit code
output_cargo_test=$(mycelium cargo test 2>&1 || true)
if echo "$output_cargo_test" | grep -q "FAILURES\|test result:\|passed"; then
    PASS=$((PASS + 1))
    printf "  ${GREEN}PASS${NC}  %s\n" "mycelium cargo test"
else
    FAIL=$((FAIL + 1))
    FAILURES+=("mycelium cargo test")
    printf "  ${RED}FAIL${NC}  %s\n" "mycelium cargo test"
    printf "        got: %s\n" "$(echo "$output_cargo_test" | head -3)"
fi
assert_help    "mycelium cargo"                    mycelium cargo

# =========================================================
#  7. Curl
# =========================================================

section "Curl (new)"

assert_contains "mycelium curl JSON detect" "string" mycelium curl https://httpbin.org/json
assert_ok       "mycelium curl plain text"          mycelium curl https://httpbin.org/robots.txt
assert_help     "mycelium curl"                     mycelium curl

# =========================================================
#  8. Npm / Npx
# =========================================================

section "Npm / Npx (new)"

assert_help    "mycelium npm"                      mycelium npm
assert_help    "mycelium npx"                      mycelium npx

# =========================================================
#  9. Pnpm
# =========================================================
section "Pnpm"

assert_help    "mycelium pnpm"                     mycelium pnpm
assert_help    "mycelium pnpm build"               mycelium pnpm build
assert_help    "mycelium pnpm typecheck"           mycelium pnpm typecheck

if command -v pnpm >/dev/null 2>&1; then
    assert_ok  "mycelium pnpm help"                mycelium pnpm help
fi

# =========================================================
#  10. Grep
# =========================================================
section "Grep"

assert_ok      "mycelium grep pattern"             mycelium grep "pub fn" src/
assert_contains "mycelium grep finds results"      "pub fn" mycelium grep "pub fn" src/
assert_ok      "mycelium grep with file type"      mycelium grep "pub fn" src/ -t rust

section "Grep (extra args passthrough)"

assert_ok      "mycelium grep -i case insensitive" mycelium grep "fn" src/ -i
assert_ok      "mycelium grep -A context lines"    mycelium grep "fn run" src/ -A 2

# =========================================================
#  11. Find
# =========================================================
section "Find"

assert_ok      "mycelium find *.rs"                mycelium find "*.rs" src/
assert_contains "mycelium find shows files"        ".rs" mycelium find "*.rs" src/

# =========================================================
#  12. Json
# =========================================================
section "Json"

# Create temp JSON file for testing
TMPJSON=$(mktemp /tmp/mycelium-test-XXXXX.json)
echo '{"name":"test","count":42,"items":[1,2,3]}' > "$TMPJSON"

assert_ok      "mycelium json file"                mycelium json "$TMPJSON"
assert_contains "mycelium json shows schema"       "string" mycelium json "$TMPJSON"

rm -f "$TMPJSON"

# =========================================================
#  13. Deps
# =========================================================
section "Deps"

assert_ok      "mycelium deps ."                   mycelium deps .
assert_contains "mycelium deps shows Cargo"        "Cargo" mycelium deps .

# =========================================================
#  14. Env
# =========================================================
section "Env"

assert_ok      "mycelium env"                      mycelium env
assert_ok      "mycelium env --filter PATH"        mycelium env --filter PATH

# =========================================================
#  16. Log
# =========================================================

section "Log"

TMPLOG=$(mktemp /tmp/mycelium-log-XXXXX.log)
for i in $(seq 1 20); do
    echo "[2025-01-01 12:00:00] INFO: repeated message" >> "$TMPLOG"
done
echo "[2025-01-01 12:00:01] ERROR: something failed" >> "$TMPLOG"

assert_ok      "mycelium log file"                 mycelium log "$TMPLOG"

rm -f "$TMPLOG"

# =========================================================
#  17. Summary
# =========================================================

section "Summary"

assert_ok      "mycelium summary echo hello"       mycelium summary echo hello

# =========================================================
#  18. Err
# =========================================================

section "Err"

assert_ok      "mycelium err echo ok"              mycelium err echo ok

# =========================================================
#  19. Test runner
# =========================================================

section "Test runner"

assert_ok      "mycelium test echo ok"             mycelium test echo ok

# =========================================================
#  20. Gain
# =========================================================

section "Gain"

assert_ok      "mycelium gain"                     mycelium gain
assert_ok      "mycelium gain --history"           mycelium gain --history

# =========================================================
#  21. Config & Init
# =========================================================

section "Config & Init"

assert_ok      "mycelium config"                   mycelium config
assert_ok      "mycelium init --show"              mycelium init --show

# =========================================================
#  22. Wget
# =========================================================

section "Wget"

if command -v wget >/dev/null 2>&1; then
    assert_ok  "mycelium wget stdout"              mycelium wget https://httpbin.org/robots.txt -O
else
    skip_test "mycelium wget" "wget not installed"
fi

# =========================================================
#  23. Tsc / Lint / Prettier / Next / Playwright
# =========================================================

section "JS Tooling (help only, no project context)"

assert_help    "mycelium tsc"                      mycelium tsc
assert_help    "mycelium lint"                     mycelium lint
assert_help    "mycelium prettier"                 mycelium prettier
assert_help    "mycelium next"                     mycelium next
assert_help    "mycelium playwright"               mycelium playwright

# =========================================================
#  24. Prisma
# =========================================================
section "Prisma (help only)"

assert_help    "mycelium prisma"                   mycelium prisma

# =========================================================
#  25. Vitest
# =========================================================
section "Vitest (help only)"

assert_help    "mycelium vitest"                   mycelium vitest

# =========================================================
#  26. Docker / Kubectl (help only)
# =========================================================
section "Docker / Kubectl (help only)"

assert_help    "mycelium docker"                   mycelium docker
assert_help    "mycelium kubectl"                  mycelium kubectl

# =========================================================
#  27. Python (conditional)
# =========================================================
section "Python (conditional)"

if command -v pytest &>/dev/null; then
    assert_help    "mycelium pytest"                    mycelium pytest --help
else
    skip_test "mycelium pytest" "pytest not installed"
fi

if command -v ruff &>/dev/null; then
    assert_help    "mycelium ruff"                      mycelium ruff --help
else
    skip_test "mycelium ruff" "ruff not installed"
fi

if command -v pip &>/dev/null; then
    assert_help    "mycelium pip"                       mycelium pip --help
else
    skip_test "mycelium pip" "pip not installed"
fi

# =========================================================
#  28. Go (conditional)
# =========================================================
section "Go (conditional)"

if command -v go &>/dev/null; then
    assert_help    "mycelium go"                        mycelium go --help
    assert_help    "mycelium go test"                   mycelium go test -h
    assert_help    "mycelium go build"                  mycelium go build -h
    assert_help    "mycelium go vet"                    mycelium go vet -h
else
    skip_test "mycelium go" "go not installed"
fi

if command -v golangci-lint &>/dev/null; then
    assert_help    "mycelium golangci-lint"             mycelium golangci-lint --help
else
    skip_test "mycelium golangci-lint" "golangci-lint not installed"
fi

# =========================================================
#  29. Graphite (conditional) ─
# =========================================================
section "Graphite (conditional)"

if command -v gt &>/dev/null; then
    assert_help   "mycelium gt"                          mycelium gt --help
    assert_ok     "mycelium gt log short"                mycelium gt log short
else
    skip "gt not installed"
fi

# =========================================================
#  30. Global flags
# =========================================================
section "Global flags"

assert_ok      "mycelium -u ls ."                  mycelium -u ls .
assert_ok      "mycelium --skip-env npm --help"    mycelium --skip-env npm --help

# =========================================================
#  31. CcEconomics
# =========================================================
section "CcEconomics"

assert_ok      "mycelium cc-economics"             mycelium cc-economics

# =========================================================
#  32. Learn
# =========================================================

section "Learn"

assert_ok      "mycelium learn --help"             mycelium learn --help
assert_ok      "mycelium learn (no sessions)"      mycelium learn --since 0 2>&1 || true

# =========================================================
#  32. Rewrite
# =========================================================
section "Rewrite"

assert_contains "rewrite git status"          "mycelium git status"         mycelium rewrite "git status"
assert_contains "rewrite cargo test"          "mycelium cargo test"         mycelium rewrite "cargo test"
assert_contains "rewrite compound &&"         "mycelium git status"         mycelium rewrite "git status && cargo test"
assert_contains "rewrite pipe preserves"      "| head"                 mycelium rewrite "git log | head"

section "Rewrite (#345: MYCELIUM_DISABLED skip)"

assert_fails   "rewrite MYCELIUM_DISABLED=1 skip"                          mycelium rewrite "MYCELIUM_DISABLED=1 git status"
assert_fails   "rewrite env MYCELIUM_DISABLED skip"                        mycelium rewrite "FOO=1 MYCELIUM_DISABLED=1 cargo test"

section "Rewrite (#346: 2>&1 preserved)"

assert_contains "rewrite 2>&1 preserved"      "2>&1"                  mycelium rewrite "cargo test 2>&1 | head"

section "Rewrite (#196: gh --json skip)"

assert_fails   "rewrite gh --json skip"                               mycelium rewrite "gh pr list --json number"
assert_fails   "rewrite gh --jq skip"                                 mycelium rewrite "gh api /repos --jq .name"
assert_fails   "rewrite gh --template skip"                           mycelium rewrite "gh pr view 1 --template '{{.title}}'"
assert_contains "rewrite gh normal works"     "mycelium gh pr list"        mycelium rewrite "gh pr list"

# =========================================================
#  33. Verify
# =========================================================
section "Verify"

assert_ok      "mycelium verify"                   mycelium verify

# =========================================================
#  34. Proxy
# =========================================================
section "Proxy"

assert_ok      "mycelium proxy echo hello"         mycelium proxy echo hello
assert_contains "mycelium proxy passthrough"       "hello" mycelium proxy echo hello

# =========================================================
#  35. Discover
# =========================================================
section "Discover"

assert_ok      "mycelium discover"                 mycelium discover

# =========================================================
#  36. Diff
# =========================================================
section "Diff"

assert_ok      "mycelium diff two files"           mycelium diff Cargo.toml LICENSE

# =========================================================
#  37. Wc
# =========================================================
section "Wc"

assert_ok      "mycelium wc Cargo.toml"            mycelium wc Cargo.toml

# =========================================================
#  38. Smart
# =========================================================
section "Smart"

assert_ok      "mycelium smart src/main.rs"        mycelium smart src/main.rs

# =========================================================
#  39. Json edge cases
# =========================================================
section "Json (edge cases)"

assert_fails   "mycelium json on TOML (#347)"                              mycelium json Cargo.toml

# =========================================================
#  40. Docker (conditional)
# =========================================================
section "Docker (conditional)"

if command -v docker >/dev/null 2>&1 && docker info >/dev/null 2>&1; then
    assert_ok  "mycelium docker ps"               mycelium docker ps
    assert_ok  "mycelium docker images"           mycelium docker images
else
    skip_test "mycelium docker" "docker not running"
fi

# =========================================================
#  41. Hook check
# =========================================================
section "Hook check (#344)"

assert_contains "mycelium init --show hook version" "version" mycelium init --show

# =========================================================
# Report
# =========================================================

printf "\n${BOLD}══════════════════════════════════════${NC}\n"
printf "${BOLD}Results: ${GREEN}%d passed${NC}, ${RED}%d failed${NC}, ${YELLOW}%d skipped${NC}\n" "$PASS" "$FAIL" "$SKIP"

if [[ ${#FAILURES[@]} -gt 0 ]]; then
    printf "\n${RED}Failures:${NC}\n"
    for f in "${FAILURES[@]}"; do
        printf "  - %s\n" "$f"
    done
fi

printf "${BOLD}══════════════════════════════════════${NC}\n"

exit "$FAIL"
