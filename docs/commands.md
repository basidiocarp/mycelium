# Mycelium Command Reference

> Reference for the public Mycelium command surface. Hidden/internal commands such as `wc`, `parse-health`, `hook-audit`, and `rewrite` are intentionally omitted. For an overview of filtering strategies and savings, see [features.md](features.md).

---

## Table of Contents

1. [File Commands](#file-commands)
2. [Git Commands](#git-commands)
3. [GitHub CLI Commands](#github-cli-commands)
4. [Test Commands](#test-commands)
5. [Build, Lint, and Formatting Commands](#build-lint-and-formatting-commands)
6. [Package Managers](#package-managers)
7. [Containers and Orchestration](#containers-and-orchestration)
8. [Data and Network](#data-and-network)
9. [Cloud and Databases](#cloud-and-databases)
10. [Stacked PRs (Graphite)](#stacked-prs-graphite)
11. [Analytics and Context](#analytics-and-context)
12. [Setup and Utilities](#setup-and-utilities)

---

## File Commands

### `mycelium ls` -- Directory Listing

**Purpose:** Replaces `ls` and `tree` with a token-optimized output.

**Syntax:**
```bash
mycelium ls [args...]
```

All native `ls` flags are supported (`-l`, `-a`, `-h`, `-R`, etc.).

**Savings:** ~80% token reduction

**Before / After:**
```
# ls -la (45 lines, ~800 tokens)          # mycelium ls (12 lines, ~150 tokens)
drwxr-xr-x  15 user staff 480 ...          my-project/
-rw-r--r--   1 user staff 1234 ...          +-- src/ (8 files)
-rw-r--r--   1 user staff 567 ...           |   +-- main.rs
...40 more lines...                        +-- Cargo.toml
                                            +-- README.md
```

---

### `mycelium tree` -- Directory Tree

**Purpose:** Proxy to native `tree` with filtered output.

**Syntax:**
```bash
mycelium tree [args...]
```

Supports all native `tree` flags (`-L`, `-d`, `-a`, etc.).

**Savings:** ~80%

---

### `mycelium read` -- File Reading

**Purpose:** Replaces `cat`, `head`, `tail` with smart content filtering.

**Syntax:**
```bash
mycelium read <file> [options]
mycelium read - [options]          # Read from stdin
```

**Options:**

| Option | Short | Default | Description |
|--------|-------|---------|-------------|
| `--level` | `-l` | `minimal` | Filter level: `none`, `minimal`, `aggressive` |
| `--max-lines` | `-m` | unlimited | Maximum number of lines |
| `--line-numbers` | `-n` | no | Show line numbers |

**Filter levels:**

| Level | Description | Savings |
|-------|-------------|---------|
| `none` | No filtering, raw output | 0% |
| `minimal` | Removes comments and excessive blank lines | ~30% |
| `aggressive` | Signatures only (removes function bodies) | ~74% |

**Before / After (aggressive mode):**
```
# cat main.rs (~200 lines)                # mycelium read main.rs -l aggressive (~50 lines)
fn main() -> Result<()> {                   fn main() -> Result<()> { ... }
    let config = Config::load()?;           fn process_data(input: &str) -> Vec<u8> { ... }
    let data = process_data(&input);        struct Config { ... }
    for item in data {                      impl Config { fn load() -> Result<Self> { ... } }
        println!("{}", item);
    }
    Ok(())
}
...
```

**Languages supported for filtering:** Rust, Python, JavaScript, TypeScript, Go, C, C++, Java, Ruby, Shell.

---

### `mycelium peek` -- Heuristic Summary

**Purpose:** Generates a 2-line technical summary of a source file.

**Syntax:**
```bash
mycelium peek <file> [--model heuristic] [--force-download]
```

**Savings:** ~95%

**Example:**
```
$ mycelium peek src/tracking.rs
SQLite-based token tracking system for command executions.
Records input/output tokens, savings %, execution times with 90-day retention.
```

---

### `mycelium find` -- File Search

**Purpose:** Replaces `find` and `fd` with compact output grouped by directory.

**Syntax:**
```bash
mycelium find [args...]
```

Supports both Mycelium syntax and native `find` syntax (`-name`, `-type`, etc.).

**Savings:** ~80%

**Before / After:**
```
# find . -name "*.rs" (30 lines)           # mycelium find "*.rs" . (8 lines)
./src/main.rs                                src/ (12 .rs)
./src/git.rs                                   main.rs, git.rs, config.rs
./src/config.rs                                tracking.rs, filter.rs, utils.rs
./src/tracking.rs                              ...6 more
./src/filter.rs                              tests/ (3 .rs)
./src/utils.rs                                 test_git.rs, test_ls.rs, test_filter.rs
...24 more lines...
```

---

### `mycelium grep` -- Content Search

**Purpose:** Replaces `grep` and `rg` with output grouped by file and truncated.

**Syntax:**
```bash
mycelium grep <pattern> [path] [options]
```

**Options:**

| Option | Short | Default | Description |
|--------|-------|---------|-------------|
| `--max-len` | `-l` | 80 | Maximum line length |
| `--max` | `-m` | 50 | Maximum number of results |
| `--context-only` | `-c` | no | Show only the match context |
| `--file-type` | `-t` | all | Filter by type (ts, py, rust, etc.) |
| `--line-numbers` | `-n` | yes | Line numbers (always active) |

Additional arguments are passed through to `rg` (ripgrep).

**Savings:** ~80%

**Before / After:**
```
# rg "fn run" (20 lines)                   # mycelium grep "fn run" (10 lines)
src/git.rs:45:pub fn run(...)                src/git.rs
src/git.rs:120:fn run_status(...)              45: pub fn run(...)
src/ls.rs:12:pub fn run(...)                   120: fn run_status(...)
src/ls.rs:25:fn run_tree(...)                src/ls.rs
...                                            12: pub fn run(...)
                                               25: fn run_tree(...)
```

---

### `mycelium diff` -- Condensed Diff

**Purpose:** Ultra-condensed diff between two files (only changed lines).

**Syntax:**
```bash
mycelium diff <file1> <file2>
mycelium diff <file1>              # Stdin as second file
```

**Savings:** ~60%

---

## Git Commands

### Overview

All git subcommands are supported. Unrecognized commands are passed directly to git (passthrough).

**Global git options:**

| Option | Description |
|--------|-------------|
| `-C <path>` | Change directory before execution |
| `-c <key=value>` | Override a git config value |
| `--git-dir <path>` | Path to the .git directory |
| `--work-tree <path>` | Path to the working tree |
| `--no-pager` | Disable pager |
| `--no-optional-locks` | Skip optional locks |
| `--bare` | Treat as bare repo |
| `--literal-pathspecs` | Literal pathspecs |

---

### `mycelium git status` -- Compact Status

**Savings:** ~80%

```bash
mycelium git status [args...]    # Supports all git status flags
```

**Before / After:**
```
# git status (~20 lines, ~400 tokens)      # mycelium git status (~5 lines, ~80 tokens)
On branch main                               main | 3M 1? 1A
Your branch is up to date with               M src/main.rs
  'origin/main'.                              M src/git.rs
                                              M tests/test_git.rs
Changes not staged for commit:                ? new_file.txt
  (use "git add <file>..." to update)        A staged_file.rs
  modified:   src/main.rs
  modified:   src/git.rs
  ...
```

---

### `mycelium git log` -- Compact History

**Savings:** ~80%

```bash
mycelium git log [args...]    # Supports --oneline, --graph, --all, -n, etc.
```

**Before / After:**
```
# git log (50+ lines)                      # mycelium git log -n 5 (5 lines)
commit abc123def... (HEAD -> main)           abc123 Fix token counting bug
Author: User <user@email.com>               def456 Add vitest support
Date:   Mon Jan 15 10:30:00 2024            789abc Refactor filter engine
                                             012def Update README
    Fix token counting bug                   345ghi Initial commit
...
```

---

### `mycelium git diff` -- Compact Diff

**Savings:** ~75%

```bash
mycelium git diff [args...]    # Supports --stat, --cached, --staged, etc.
```

**Before / After:**
```
# git diff (~100 lines)                    # mycelium git diff (~25 lines)
diff --git a/src/main.rs b/src/main.rs      src/main.rs (+5/-2)
index abc123..def456 100644                    +  let config = Config::load()?;
--- a/src/main.rs                              +  config.validate()?;
+++ b/src/main.rs                              -  // old code
@@ -10,6 +10,8 @@                              -  let x = 42;
   fn main() {                               src/git.rs (+1/-1)
+    let config = Config::load()?;              ~  format!("ok {}", branch)
...30 lines of headers and context...
```

---

### `mycelium git show` -- Compact Show

**Savings:** ~80%

```bash
mycelium git show [args...]
```

Displays commit summary + stat + compact diff.

---

### `mycelium git add` -- Ultra-Compact Add

**Savings:** ~92%

```bash
mycelium git add [args...]    # Supports -A, -p, --all, etc.
```

**Output:** `ok` (a single word)

---

### `mycelium git commit` -- Ultra-Compact Commit

**Savings:** ~92%

```bash
mycelium git commit -m "message" [args...]    # Supports -a, --amend, --allow-empty, etc.
```

**Output:** `ok abc1234` (confirmation + short hash)

---

### `mycelium git push` -- Ultra-Compact Push

**Savings:** ~92%

```bash
mycelium git push [args...]    # Supports -u, remote, branch, etc.
```

**Before / After:**
```
# git push (15 lines, ~200 tokens)         # mycelium git push (1 line, ~10 tokens)
Enumerating objects: 5, done.                ok main
Counting objects: 100% (5/5), done.
Delta compression using up to 8 threads
...
```

---

### `mycelium git pull` -- Ultra-Compact Pull

**Savings:** ~92%

```bash
mycelium git pull [args...]
```

**Output:** `ok 3 files +10 -2`

---

### `mycelium git branch` -- Compact Branches

```bash
mycelium git branch [args...]    # Supports -d, -D, -m, etc.
```

Displays current branch, local branches, and remote branches in compact form.

---

### `mycelium git fetch` -- Compact Fetch

```bash
mycelium git fetch [args...]
```

**Output:** `ok fetched (N new refs)`

---

### `mycelium git stash` -- Compact Stash

```bash
mycelium git stash [list|show|pop|apply|drop|push] [args...]
```

---

### `mycelium git worktree` -- Compact Worktree

```bash
mycelium git worktree [add|remove|prune|list] [args...]
```

---

### Git Passthrough

Any git subcommand not listed above is executed directly:

```bash
mycelium git rebase main        # Executes git rebase main
mycelium git cherry-pick abc    # Executes git cherry-pick abc
mycelium git tag v1.0.0         # Executes git tag v1.0.0
```

---

## GitHub CLI Commands

### `mycelium gh` -- Compact GitHub CLI

**Purpose:** Replaces `gh` with optimized output.

**Syntax:**
```bash
mycelium gh <subcommand> [args...]
```

**Supported subcommands:**

| Command | Description | Savings |
|---------|-------------|---------|
| `mycelium gh pr list` | Compact PR list | ~80% |
| `mycelium gh pr view <num>` | PR details + checks | ~87% |
| `mycelium gh pr checks` | CI check status | ~79% |
| `mycelium gh issue list` | Compact issue list | ~80% |
| `mycelium gh run list` | Workflow run status | ~82% |
| `mycelium gh api <endpoint>` | Compact API response | ~26% |

**Before / After:**
```
# gh pr list (~30 lines)                   # mycelium gh pr list (~10 lines)
Showing 10 of 15 pull requests in org/repo   #42 feat: add vitest (open, 2d)
                                              #41 fix: git diff crash (open, 3d)
#42  feat: add vitest support                 #40 chore: update deps (merged, 5d)
  user opened about 2 days ago                #39 docs: add guide (merged, 1w)
  ... labels: enhancement
...
```

---

## Test Commands

### `mycelium test` -- Generic Test Wrapper

**Purpose:** Runs any test command and displays only failures.

**Syntax:**
```bash
mycelium test <command...>
```

**Savings:** ~90%

**Example:**
```bash
mycelium test cargo test
mycelium test npm test
mycelium test bun test
mycelium test pytest
```

**Before / After:**
```
# cargo test (200+ lines on failure)       # mycelium test cargo test (~20 lines)
running 15 tests                             FAILED: 2/15 tests
test utils::test_parse ... ok                  test_edge_case: assertion failed
test utils::test_format ... ok                 test_overflow: panic at utils.rs:18
test utils::test_edge_case ... FAILED
...150 lines of backtrace...
```

---

### `mycelium err` -- Errors/Warnings Only

**Purpose:** Runs a command and shows only errors and warnings.

**Syntax:**
```bash
mycelium err <command...>
```

**Savings:** ~80%

**Example:**
```bash
mycelium err npm run build
mycelium err cargo build
```

---

### `mycelium cargo test` -- Rust Tests

**Savings:** ~90%

```bash
mycelium cargo test [args...]
```

Displays only failures. Supports all `cargo test` arguments.

---

### `mycelium cargo nextest` -- Rust Tests (nextest)

```bash
mycelium cargo nextest [run|list|--lib] [args...]
```

Filters `cargo nextest` output to show only failures. Mycelium forwards the
rest of the `cargo nextest` argument surface unchanged, so the command works
with normal nextest flags and filters.

Common uses:

```bash
mycelium cargo nextest run
mycelium cargo nextest list
mycelium cargo nextest --lib
```

---

### `mycelium vitest run` -- Vitest Tests

**Savings:** ~99.5%

```bash
mycelium vitest run [args...]
```

---

### `mycelium playwright` -- Playwright E2E Tests

**Savings:** ~94%

```bash
mycelium playwright [args...]
```

---

### `mycelium pytest` -- Python Tests

**Savings:** ~90%

```bash
mycelium pytest [args...]
```

---

### `mycelium go` -- Go Tooling

| Command | Description | Savings |
|---------|-------------|---------|
| `mycelium go test [args...]` | Compact test output via NDJSON streaming | ~90% |
| `mycelium go build [args...]` | Errors-focused build output | ~75% |
| `mycelium go vet [args...]` | Filtered vet diagnostics | ~75% |

---

## Build, Lint, and Formatting Commands

### `mycelium cargo build` -- Rust Build

**Savings:** ~80%

```bash
mycelium cargo build [args...]
```

Removes "Compiling..." lines, keeps only errors and the final result.

---

### `mycelium cargo check` -- Rust Check

**Savings:** ~80%

```bash
mycelium cargo check [args...]
```

Removes "Checking..." lines, keeps only errors.

---

### `mycelium cargo clippy` -- Rust Clippy

**Savings:** ~80%

```bash
mycelium cargo clippy [args...]
```

Groups warnings by lint rule.

---

### `mycelium cargo install` -- Rust Install

```bash
mycelium cargo install [args...]
```

Removes dependency compilation output, keeps only the installation result and errors.

---

### `mycelium tsc` -- TypeScript Compiler

**Savings:** ~83%

```bash
mycelium tsc [args...]
```

Groups TypeScript errors by file and error code.

**Before / After:**
```
# tsc --noEmit (50 lines)                  # mycelium tsc (15 lines)
src/api.ts(12,5): error TS2345: ...          src/api.ts (3 errors)
src/api.ts(15,10): error TS2345: ...           TS2345: Argument type mismatch (x2)
src/api.ts(20,3): error TS7006: ...            TS7006: Parameter implicitly has 'any'
src/utils.ts(5,1): error TS2304: ...         src/utils.ts (1 error)
...                                            TS2304: Cannot find name 'foo'
```

---

### `mycelium lint` -- ESLint / Biome

**Savings:** ~84%

```bash
mycelium lint [args...]
mycelium lint biome [args...]
```

Groups violations by rule and file. Auto-detects the linter.

---

### `mycelium prettier` -- Format Checking

**Savings:** ~70%

```bash
mycelium prettier [args...]    # e.g.: mycelium prettier --check .
```

Shows only files that need formatting.

---

### `mycelium format` -- Universal Formatter

```bash
mycelium format [args...]
```

Auto-detects the project formatter (prettier, black, ruff format) and applies a compact filter.

---

### `mycelium next` -- Next.js Build

**Savings:** ~87%

```bash
mycelium next [args...]
```

Compact output with route metrics.

---

### `mycelium ruff` -- Python Linter/Formatter

**Savings:** ~80%

```bash
mycelium ruff check [args...]
mycelium ruff format --check [args...]
```

Compressed JSON output.

---

### `mycelium mypy` -- Python Type Checker

```bash
mycelium mypy [args...]
```

Groups type errors by file.

---

### `mycelium golangci-lint` -- Go Linter

**Savings:** ~85%

```bash
mycelium golangci-lint run [args...]
```

Compressed JSON output.

---

## Package Managers

### `mycelium pnpm` -- pnpm

| Command | Description | Savings |
|---------|-------------|---------|
| `mycelium pnpm list [-d N]` | Compact dependency tree | ~70% |
| `mycelium pnpm outdated` | Outdated packages: `pkg: old -> new` | ~80% |
| `mycelium pnpm install [pkgs...]` | Filters progress bars | ~60% |
| `mycelium pnpm build` | Delegates to Next.js filter | ~87% |
| `mycelium pnpm typecheck` | Delegates to tsc filter | ~83% |

Unrecognized subcommands are passed directly to pnpm (passthrough).

---

### `mycelium npm` -- npm

```bash
mycelium npm [args...]    # e.g.: mycelium npm run build
```

Filters npm boilerplate (progress bars, headers, etc.).

---

### `mycelium npx` -- npx with Smart Routing

```bash
mycelium npx [args...]
```

Intelligently routes to specialized filters:
- `mycelium npx tsc` -> tsc filter
- `mycelium npx eslint` -> lint filter
- `mycelium npx prisma` -> prisma filter
- Others -> passthrough filter

---

### `mycelium pip` -- pip / uv

```bash
mycelium pip list              # Package list (auto-detects uv)
mycelium pip outdated          # Outdated packages
mycelium pip install <pkg>     # Installation
```

Auto-detects `uv` if available and uses it instead of `pip`.

---

### `mycelium deps` -- Dependency Summary

**Purpose:** Compact summary of project dependencies.

```bash
mycelium deps [path]    # Default: current directory
```

Auto-detects: `Cargo.toml`, `package.json`, `pyproject.toml`, `go.mod`, `Gemfile`, etc.

**Savings:** ~70%

---

### `mycelium prisma` -- Prisma ORM

| Command | Description |
|---------|-------------|
| `mycelium prisma generate` | Client generation (removes ASCII art) |
| `mycelium prisma migrate dev [--name N]` | Create and apply a migration |
| `mycelium prisma migrate status` | Migration status |
| `mycelium prisma migrate deploy` | Deploy to production |
| `mycelium prisma db-push` | Schema push |

---

## Containers and Orchestration

### `mycelium docker` -- Docker

| Command | Description | Savings |
|---------|-------------|---------|
| `mycelium docker ps` | Compact container list | ~80% |
| `mycelium docker images` | Compact image list | ~80% |
| `mycelium docker logs <container>` | Deduplicated logs (default --tail 500, respects user's --tail) | ~70% |
| `mycelium docker compose ps` | Compact Compose services | ~80% |
| `mycelium docker compose logs [service]` | Deduplicated Compose logs | ~70% |
| `mycelium docker compose build [service]` | Build summary | ~60% |

Unrecognized subcommands are passed directly (passthrough).

**Before / After:**
```
# docker ps (long lines, ~30 tokens/line)    # mycelium docker ps (~10 tokens/line)
CONTAINER ID   IMAGE          COMMAND     ...      web  nginx:1.25 Up 2d (healthy)
abc123def456   nginx:1.25     "/dock..."  ...      db   postgres:16 Up 2d (healthy)
789012345678   postgres:16    "docker..."           redis redis:7 Up 1d
```

---

### `mycelium kubectl` -- Kubernetes

| Command | Description | Options |
|---------|-------------|---------|
| `mycelium kubectl pods [-n ns] [-A]` | Compact pod list | Namespace or all |
| `mycelium kubectl services [-n ns] [-A]` | Compact service list | Namespace or all |
| `mycelium kubectl logs <pod> [-c container]` | Deduplicated logs | Specific container |

Unrecognized subcommands are passed directly (passthrough).

---

## Data and Network

### `mycelium json` -- JSON Structure

**Purpose:** Displays the structure of a JSON file without values.

```bash
mycelium json <file> [--depth N]    # Default: depth 5
mycelium json -                      # From stdin
```

**Savings:** ~60%

**Before / After:**
```
# cat package.json (50 lines)              # mycelium json package.json (10 lines)
{                                            {
  "name": "my-app",                            name: string
  "version": "1.0.0",                         version: string
  "dependencies": {                            dependencies: { 15 keys }
    "react": "^18.2.0",                        devDependencies: { 8 keys }
    "next": "^14.0.0",                         scripts: { 6 keys }
    ...15 dependencies...                   }
  },
  ...
}
```

---

### `mycelium env` -- Environment Variables

```bash
mycelium env                    # All variables (sensitive ones masked)
mycelium env -f AWS             # Filter by name
mycelium env --show-all         # Include sensitive values
```

Sensitive variables (tokens, secrets, passwords) are masked by default: `AWS_SECRET_ACCESS_KEY=***`.

---

### `mycelium log` -- Deduplicated Logs

**Purpose:** Filters and deduplicates log output.

```bash
mycelium log <file>     # From a file
mycelium log               # From stdin (pipe)
```

Repeated lines are merged: `[ERROR] Connection refused (x42)`.

**Savings:** ~60-80% (depending on repetitiveness)

---

### `mycelium curl` -- HTTP with JSON Detection

```bash
mycelium curl [args...]
```

Auto-detects JSON responses. Error responses (4xx/5xx) and small JSON (<5KB) are kept with full values. Larger JSON responses have string values truncated and deeply nested objects collapsed to preserve structure without bloating context.

---

### `mycelium wget` -- Compact Download

```bash
mycelium wget <url> [args...]
mycelium wget -O - <url>           # Output to stdout
```

Removes progress bars and noise.

---

### `mycelium summary` -- Heuristic Summary

**Purpose:** Runs a command and generates a heuristic summary of the output.

```bash
mycelium summary <command...>
```

Useful for long-running commands whose output has no dedicated filter.

---

### `mycelium proxy` -- Passthrough with Tracking

**Purpose:** Runs a command **without filtering** but records usage for tracking.

```bash
mycelium proxy <command...>
```

Useful for debugging: compare raw output with filtered output.

---

## Cloud and Databases

### `mycelium terraform` -- Terraform

| Command | Description |
|---------|-------------|
| `mycelium terraform plan [args...]` | Compact plan output |
| `mycelium terraform apply [args...]` | Compact apply output |
| `mycelium terraform init [args...]` | Progress-filtered init output |

Unsupported subcommands are passed directly (passthrough).

---

### `mycelium aws` -- AWS CLI

```bash
mycelium aws <service> [args...]
```

| Service | Description |
|---------|-------------|
| `mycelium aws sts [args...]` | Compact identity and caller output |
| `mycelium aws s3 [args...]` | Compact bucket and object summaries |
| `mycelium aws ec2 [args...]` | Compact instance summaries |
| `mycelium aws ecs [args...]` | Compact cluster, service, and task summaries |
| `mycelium aws rds [args...]` | Compact database summaries |
| `mycelium aws cloudformation [args...]` | Compact stack summaries |
| `mycelium aws <other-service> [args...]` | Generic JSON compression passthrough |

Known services get service-specific formatting; unsupported services still run through the generic JSON compressor.

---

### `mycelium atmos` -- Atmos

| Command | Description |
|---------|-------------|
| `mycelium atmos terraform [args...]` | Compact `atmos terraform` flows |
| `mycelium atmos describe [args...]` | Truncated structured output |
| `mycelium atmos validate [args...]` | Filtered validation issues |
| `mycelium atmos workflow [args...]` | Truncated workflow output |
| `mycelium atmos version [args...]` | Compact version output |

Unsupported subcommands are passed directly (passthrough).

---

### `mycelium psql` -- PostgreSQL

```bash
mycelium psql [args...]
```

Removes table borders and compresses the output.

---

## Stacked PRs (Graphite)

### `mycelium gt` -- Graphite

| Command | Description |
|---------|-------------|
| `mycelium gt log` | Compact stack log |
| `mycelium gt submit` | Compact submit |
| `mycelium gt sync` | Compact sync |
| `mycelium gt restack` | Compact restack |
| `mycelium gt create` | Compact create |
| `mycelium gt branch` | Compact branch info |

Unrecognized subcommands are passed directly or detected as git passthrough.

---

## Analytics and Context

### `mycelium gain` -- Token Savings Analytics

```bash
mycelium gain [--graph] [--history] [--daily|--weekly|--monthly|--all]
mycelium gain --format json
mycelium gain --diagnostics
mycelium gain --compare "git status"
```

Shows aggregate savings, history, exports, diagnostics, and side-by-side comparisons.

---

### `mycelium discover` -- Missed Opportunity Discovery

```bash
mycelium discover [--project PATH] [--all] [--since N] [--format text|json]
```

Scans Claude Code and Codex history for commands that could have used a Mycelium rewrite.

---

### `mycelium learn` -- CLI Correction Mining

```bash
mycelium learn [--project PATH] [--all] [--since N] [--write-rules]
```

Extracts recurring CLI correction patterns and can write `.claude/rules/cli-corrections.md`.

---

### `mycelium context` -- Hyphae Context Gathering

```bash
mycelium context <task...> [--project NAME] [--budget TOKENS] [--include SOURCES]
```

Pulls scoped context from Hyphae-backed sources such as memories, errors, sessions, and code.

---

### `mycelium economics` -- Spend vs Savings

```bash
mycelium economics [--project] [--daily|--weekly|--monthly|--all] [--format text|json|csv]
```

Compares Claude Code spend with Mycelium savings. `cc-economics` remains an alias.

---

## Setup and Utilities

| Command | Description |
|---------|-------------|
| `mycelium init [options]` | Install, inspect, or remove Mycelium-managed hook and guidance files |
| `mycelium config [--create]` | Show or create the config file |
| `mycelium doctor` | Run health checks |
| `mycelium verify` | Verify hook integrity |
| `mycelium self-update [--check]` | Check for or install updates |
| `mycelium completions <shell>` | Generate shell completions |
| `mycelium proxy <command...>` | Run a command raw while still tracking usage |
| `mycelium invoke <command...>` | Resolve and execute a shell command through rewrite logic |
| `mycelium benchmark [--ci]` | Measure savings across command fixtures |
| `mycelium plugin list|install` | Inspect the plugin directory and install shipped templates when a release includes them |
