# Changelog

All notable changes to Mycelium are documented in this file.

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
