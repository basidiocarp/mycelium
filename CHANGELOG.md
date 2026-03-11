# Changelog

All notable changes to Mycelium are documented in this file.

## v0.1.6

### Security

- **Fixed plugin PID reuse race condition**: The plugin timeout thread could kill an unrelated process if the OS recycled the child's PID. Now uses `Child::kill()` via `Arc<Mutex<Option<Child>>>` instead of raw PID signaling.

- **Fixed UID-based ownership check**: Plugin security check used `$UID` env var (bash-only, missing in zsh/macOS). Now uses `id -u` for portable UID detection and fails closed on error instead of silently passing.

### Bug Fixes

- **Fixed operator precedence bug in summary command**: `detect_output_type` misclassified any command containing "test" as test results due to `||`/`&&` precedence. Now requires both a test command AND test result markers in output.

### Performance

- **Cleanup amortized to once per day**: `cleanup_old()` previously ran 2 DELETE queries after every single command. Now checks a `meta` table timestamp and only runs if >24 hours since last cleanup.

- **Schema init cached via PRAGMA user_version**: Migrations now skip entirely when the schema version is current, eliminating redundant `CREATE TABLE IF NOT EXISTS` calls on every invocation.

- **Git show consolidated to 1 subprocess**: Previously ran 4 separate `git show` commands (raw, summary, stat, diff). Now uses a single call with combined format, splitting output in-memory.

- **Removed walkdir dependency**: Replaced with `ignore::WalkBuilder` (already a direct dependency), reducing the dependency tree.

### Code Quality

- **Eliminated 63 bare `.unwrap()` on regex**: All `Regex::new(...).unwrap()` in production code replaced with `.expect("valid regex")` to prevent silent panics.

- **Removed 21 dead code annotations**: Audited 32 `#[allow(dead_code)]` sites — deleted unused functions, gated test-only code with `#[cfg(test)]`, kept justified annotations for serde structs and builder APIs.

- **Extracted shell dispatch utility**: Deduplicated cross-platform shell dispatch (`sh -c` / `cmd /C`) from `runner_cmd.rs` and `summary_cmd.rs` into `utils::run_shell_command()`.

- **Replaced 5-element tuple with `CommandStats` struct**: `by_command: Vec<(String, usize, usize, f64, u64)>` replaced with named fields for readability.

- **Safe string slicing**: `truncate_iso_date` now uses `.get(..10).unwrap_or(date)` instead of panicking `&date[..10]`.

- **Unified `has_json_flag` functions**: Merged two duplicate functions into a single generic implementation.

- **Removed no-op schema migration**: Deleted `ALTER TABLE commands RENAME COLUMN mycelium_cmd TO mycelium_cmd`.

### Tests

- **Added ~30 token savings tests**: Coverage for cargo, Python, JavaScript, container, fileops, AWS, Go, and git filter modules — all verifying ≥60% savings with realistic fixtures.

- **21 new test fixtures**: Real-world command output for cargo build/clippy/install/nextest, pytest, ruff, pip, mypy, tsc, vitest, prettier, playwright, npm, pnpm, docker, diff, ls, tree, and wc.

## v0.1.5

### New Features

- **Built-in benchmark command**: Run `mycelium benchmark` to measure token savings across all available commands. Includes `--ci` mode that fails if less than 80% of tests show savings.

- **Plugin management**: Install and manage filter plugins with `mycelium plugin list` and `mycelium plugin install <name>` — no need to clone the repo or run shell scripts.

- **Per-project analytics**: View token savings scoped to a project with `mycelium gain --project` or see a breakdown across all projects with `mycelium gain --projects`.

- **Enhanced doctor checks**: `mycelium doctor` now verifies Claude Code settings.json hook registration, plugin directory status, and PATH configuration.

### Improvements

- **Safer error handling**: Replaced risky `.unwrap()` calls in production code with proper error propagation or safe defaults. Regex patterns now use descriptive `.expect()` messages.

- **Git stash filter**: Added output filtering for `git stash list` — strips verbose branch/date prefixes, keeping stash index and commit message.

- **Hook template v3**: Fixed version guard that blocked rewrites, added jq error handling, heredoc safety, and opt-in audit logging (`MYCELIUM_HOOK_AUDIT=1`).

- **CI coverage tracking**: New workflow enforces 60% minimum code coverage using `cargo-llvm-cov`.

- **Relaxed binary size limit**: Performance CI guard bumped from 5MB to 8MB to accommodate bundled SQLite.

### Documentation

- Split oversized docs into focused files: COMMANDS.md (full command reference), ANALYTICS.md (tracking/hooks), EXTENDING.md (adding new commands), PLUGINS.md, COST_ANALYSIS.md.

### Fixes

- Fixed malformed jq handling in hook script that caused silent failures on invalid JSON input
- Fixed formatting inconsistencies across multiple source files

## v0.1.4

### Improvements

- **Self-update command**: Overhauled `mycelium self-update` with improved error handling and release detection.

### Fixes

- Fixed self-updater failing to detect latest release from GitHub

## v0.1.3

### New Features

- **Release script**: Added `scripts/release.sh` for automated version bumping, tagging, and GitHub release creation.

- **Improved CLI output**: Enhanced help text and command display formatting.

### Fixes

- Fixed release script version handling

## v0.1.2

### Improvements

- **CLI command cleanup**: Reorganized and standardized all CLI subcommands and help text.

- **Cross-platform fixes**: Resolved Windows build failures (dead code errors, ETXTBSY on Linux CI).

- **Installation improvements**: Updated install script and verification checks.

### Fixes

- Fixed Windows build errors
- Fixed installation path detection

## v0.1.1

### Improvements

- **Learning system**: Fixed error correction detection in `mycelium learn`.

- **CI hardening**: Bumped GitHub Actions dependencies, fixed clippy warnings, standardized shell script formatting.

### Fixes

- Fixed Windows dead code warnings and Linux ETXTBSY errors in CI

## v0.1.0

Initial release. Token-optimized CLI proxy for Claude Code with 60-90% savings on dev operations.

- 45+ command filters across 11 categories (Git, GitHub, Cargo, Docker, AWS, Terraform, and more)
- Automatic hook-based command rewriting for Claude Code
- Token savings analytics with `mycelium gain`
- Opportunity discovery with `mycelium discover`
- Self-update support
- Cross-platform support (macOS, Linux, Windows)
- CI pipeline with formatting, linting, testing, performance guards, and security audits
