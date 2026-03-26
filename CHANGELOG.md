# Changelog

All notable changes to Mycelium are documented in this file.

## v0.7.2 - 2026-03-26

### Fixed

- **Platform-aware hook audit log path**: `mycelium hook-audit` now uses Mycelium's shared platform data directory instead of falling back to a Unix-shaped `HOME` path and `/tmp` when `HOME` is unavailable.
- **Unified shell resolution**: Runtime shell execution now goes through one shared platform-aware resolver, avoiding direct `sh` assumptions and respecting the configured shell on non-Windows systems plus `COMSPEC` on Windows.

## v0.7.1 - 2026-03-26

### Changed

- **Cross-platform runtime plumbing**: Added a shared platform layer for config/data path resolution, shell dispatch, PATH parsing, and command lookup so Mycelium no longer assumes Unix-only `which`, `:`-separated PATH values, or direct `sh -c` execution in key runtime paths.
- **Shared editor/config registration**: `mycelium init` now uses `spore`'s shared editor/config model for Codex CLI, Cursor, Windsurf, and Claude Desktop registration instead of maintaining a separate local TOML/JSON mutation path.
- **Host-neutral setup guidance**: Onboarding, ecosystem setup, doctor output, and related CLI text now present Claude Code and Codex CLI as peer host adapters while still calling out Claude-specific hook setup where it actually exists.

## v0.7.0 - 2026-03-23

### Added

- **Codex-aware ecosystem setup**: `mycelium init --ecosystem` now recognizes Codex CLI as a first-class host client and can register Hyphae and Rhizome MCP servers into `~/.codex/config.toml`.

### Changed

- **Host-aware onboarding**: `mycelium init --onboard` now guides users through configuring the detected host client instead of assuming Claude Code is mandatory.
- **Host-specific next steps**: Onboarding summaries and help text now show Claude and Codex follow-up steps based on the clients actually detected on the machine.

## v0.6.0 - 2026-03-23

### Added

- **Task-runner-aware rewrite support**: Explicit wrapper forms like `mise exec -- <command>`, `just -- <command>`, and `task -- <command>` now unwrap to the underlying command for rewrite/classification, while ambiguous task names still fail safe to raw execution.
- **Tracking DB status view**: `mycelium gain --status` now reports the active tracking database path, where that path came from, and basic health details.
- **Curated library API**: `src/lib.rs` now exposes rewrite, filter, compaction, and tracking helpers for downstream Basidiocarp tools.

### Changed

- **Broader `gh` edge-mode passthrough**: Issue, PR, run, and repo views now pass through more output-shaping modes like `--json`, `--jq`, `--template`, `--web`, `--comments`, and related browser/watch variants instead of trying to filter the wrong output shape.
- **Named compaction profiles**: Added `debug`, `balanced`, and `aggressive` compaction profiles and wired them into adaptive classification plus git diff/status compression budgets.
- **Hook install diagnostics**: Installed rewrite hooks now carry a stamped Mycelium version, and `mycelium init --show-config` can distinguish current hooks from stale or unknown ones with direct repair guidance.

### Fixed

- **Hook repair visibility**: Rewrite hooks now explain skipped rewrites more clearly when `mycelium`, `jq`, or embedded paths are stale or missing.
- **Tracking path drift diagnosis**: Config, doctor output, and tracking utilities now surface whether the active DB path came from an override, env var, config file, or platform default.

## v0.5.1 - 2026-03-23

### Changed

- **`gh` passthrough edge modes**: `mycelium gh issue view` now defers to the real GitHub CLI for `--comments`, `--json`, `--jq`, `--template`, and `--web`, and `mycelium gh pr diff` now passes through `--name-only`, `--web`, and `--no-compact` instead of forcing compacted output.
- **Larger Git compaction budgets**: Increased the retained diff hunk budget and status file budget so routine repository state is less likely to be truncated.
- **Rewrite hook installation**: Installed hooks now embed resolved `mycelium` and `jq` paths while still falling back to `PATH` when needed.

### Fixed

- **Signed commit coverage**: Added regression tests around `git commit -S` flows so signed commit and signed amend behavior stays intact.
- **Hook PATH fragility**: Rewrite hooks now emit explicit skip diagnostics when `mycelium` or `jq` are unavailable, and `mycelium init --show-config` reports stale embedded paths and missing runtime dependencies instead of failing silently.

## v0.5.0 - 2026-03-22

### Added

- **`mycelium rewrite --explain`**: Shows whether a command rewrites through the built-in registry, a learned correction, or not at all, with a short reason for the decision.

### Changed

- **Onboarding handoff to Stipe**: Updated setup and update docs so `stipe init` is the primary onboarding and repair entry point, while `mycelium init --ecosystem` remains the lower-level integration path.

## v0.4.5 - 2026-03-22

### Fixed

- **Plugin PID race condition**: Timeout thread now checks a cancellation flag before sending SIGTERM, preventing signals to recycled PIDs.
- **Plugin ownership check**: Replaced unreliable `$UID` environment variable with `libc::getuid()` for correct user detection in all shell contexts.
- **smart_truncate count**: Omission markers now report the actual section size instead of total remaining lines.

### Changed

- **dispatch() refactor**: Decomposed 852-line monolithic function into 13 per-family helpers (git, gh, cargo, docker, etc.).
- **Tracker reuse**: `record_parse_failure_silent` accepts optional `&Tracker` to avoid double SQLite opens on fallback paths.
- **Deprecated hooks removed**: JS/shell capture hooks replaced by cortina (v0.4.4).
- **Spore v0.4.0**: Self-update and token estimation use shared spore modules.

## v0.3.2

### Features

- **`mycelium init --onboard`**: Interactive onboarding wizard that guides new users through ecosystem setup, tool detection, and configuration in one step.
- **`mycelium init --ecosystem --client <name>`**: Multi-client support for `init --ecosystem`, allowing separate MCP configurations per client (Claude Code, Cursor, etc.).
- **`mycelium context <task>`**: New CLI command that gathers relevant context for a task description by querying Hyphae memories, Rhizome code intelligence, and local project state.
- **Session-summary Stop hook**: Automatically generates a session summary when Claude Code exits, capturing key decisions and outcomes for Hyphae storage.

### Refactoring

- **Spore adoption for tool discovery**: Migrated remaining manual binary detection to the shared `spore` crate's `discover()` API for consistent cross-tool resolution.

### Tests

- **Config deserialization tests**: Added test coverage for TOML config parsing, including edge cases for missing fields and invalid values.

### Bug Fixes

- **Clippy fixes**: Resolved pedantic clippy warnings across the codebase.

## v0.2.2

### Features

- **`mycelium init --ecosystem`**: Detect sibling Basidiocarp tools (Hyphae, Rhizome, Cap) and register their MCP servers with Claude Code in one command.

### Refactoring

- **Spore crate for tool discovery**: Replaced manual `which`/`where` binary detection in Hyphae and Rhizome modules with the shared `spore` crate's `discover()` API.

### Bug Fixes

- **Fixed Hyphae tests on machines with Hyphae installed**: Tests now adapt to the environment instead of assuming Hyphae is absent.

### CI

- Updated CI workflow configuration.

## v0.2.1

### Features

- **Hyphae integration**: When [Hyphae](https://github.com/basidiocarp/hyphae) is installed, large command outputs (>500 lines) are automatically chunked and stored in Hyphae instead of being destructively filtered. A summary with retrieval key is returned to the agent. Fully optional — Mycelium remains standalone without Hyphae. Configurable via `[filter.hyphae]` in config.toml.

- **Rhizome integration**: When [Rhizome](https://github.com/basidiocarp/rhizome) is installed, `mycelium read` delegates to Rhizome for structured symbol extraction (functions, types, imports) instead of applying MinimalFilter/AggressiveFilter. Non-code files fall back to existing filters. Configurable via `[filter.rhizome]` in config.toml.

### CI

- Removed cargo audit workflow.
- Added concurrency groups, pre-built tool installs, merged performance jobs, fixed double coverage run, bumped upload-artifact to v7.

## v0.2.0

### Features

- **Adaptive filtering**: Size-aware output compression — small outputs (<50 lines / <2KB) pass through unfiltered, medium outputs get light filtering, large outputs (>500 lines) get full structured compression. Configurable via `[filter.adaptive]` in config.toml (`small_lines`, `small_bytes`, `large_lines`).

- **Intelligent comment classification**: MinimalFilter now distinguishes actionable comments (TODO, FIXME, HACK, SAFETY, NOTE, BUG, WARNING) from noise (separator lines, auto-generated markers, pragma directives). Noise is stripped; actionable comments are preserved.

- **License header detection**: File preambles with >3 consecutive comment lines before any code are detected as license headers and stripped by the MinimalFilter.

- **Function body folding**: AggressiveFilter folds function/impl bodies exceeding 30 lines to `// ... (N lines)` instead of dropping them entirely. Small functions are kept inline.

### Improvements

- **curl**: Preserve real JSON values instead of schematizing. Error responses (4xx/5xx) and small JSON (<5KB) pass through with full values. Only large responses get value truncation.

- **docker logs**: Default `--tail` raised from 100 to 500. Respects user-specified `--tail` value. Shows deduplication count in output.

- **git diff**: Hunk line cap raised from 30 to 100 lines for more complete diffs.

- **git log**: Removed automatic `--no-merges` injection — users get the log they asked for.

- **git status**: Line truncation raised from 80 to 120 characters.

- **ls**: Removed `dist/`, `build/`, `.vscode/` from default noise directories — these are commonly needed by agents.

- **Test formatters**: Show full error context for cargo test, vitest, and playwright failures. Added `passed_names` tracking to `TestResult`.

### Documentation

- Updated ARCHITECTURE.md with adaptive filtering details and output sizing tiers.
- Updated FEATURES.md with adaptive filtering strategy and revised savings ranges.
- Updated COMMANDS.md with new curl and docker logs behavior.
- Updated README.md with five filtering strategies and `[filter.adaptive]` config.
- Documented optional Hyphae integration in CLAUDE.md.
- Rewrote `.plans/hyphae-integration.md` and `.plans/rhizome-integration.md` in fleet format.

### CI

- Added concurrency groups to ci, coverage, performance workflows (cancel stale runs on new pushes).
- Added `CARGO_INCREMENTAL=0` to all CI workflows for smaller cache.
- Switched to `taiki-e/install-action` for cross, cargo-deb, cargo-generate-rpm, hyperfine (pre-built binaries instead of compiling from source).
- Merged performance binary-size and startup-time into a single job (one build instead of two).
- Fixed coverage workflow running `cargo llvm-cov` twice — now runs once.
- Bumped `upload-artifact` to v7 across all workflows.
- Removed cargo audit workflow.

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
