# Mycelium Roadmap

## Completed

### Language Ecosystem Coverage
- **VCS**: Git, GitHub CLI (gh), Graphite (gt) stacked PRs
- **Rust**: cargo test/build/clippy/check/install/nextest
- **JavaScript/TypeScript**: vitest, playwright, tsc, eslint, biome, prettier, next.js, prisma, pnpm, npm, npx
- **Python**: ruff check/format, pytest, pip/uv, mypy
- **Go**: go test/build/vet, golangci-lint
- **Infrastructure**: docker, kubectl, aws (sts/s3/ec2/ecs/rds/cloudformation), terraform (plan/apply/init)
- **Databases & APIs**: psql, curl (auto-JSON + schema), wget
- **General**: ls, tree, read, peek, find, grep, diff, json, log, err, summary, env, deps, wc, format

### Architecture
- Hook-based transparent command rewriting (PreToolUse)
- Parser framework (`OutputParser` trait, `ParseResult<T>`, `TokenFormatter`)
- SQLite token tracking with daily/weekly/monthly breakdowns + JSON/CSV export
- Tee system for raw output recovery on failure
- Discover: analyze Claude Code history for missed optimization opportunities
- Learn: detect recurring CLI error patterns and generate correction rules
- CC-Economics: compare Claude Code spending with Mycelium savings
- Modular directory structure (vcs/, js/, python/, go_eco/, fileops/)
- Categorized CLI help output (11 command groups)
- Self-update from GitHub releases (archive extraction, cross-platform)
- Release script for version bumping + quality gates (`scripts/release.sh`)
- Rewrite command (single source of truth for hook rewrites)
- Proxy command with metric tracking
- JSON envelope output mode (`--json` global flag)
- Ultra-compact mode (`-u` flag)
- Shell completions (bash, zsh, fish)

### Quality
- 1000+ unit tests across 25+ files
- Smoke test suite (scripts/test-all.sh, 600+ lines)
- Hook integrity verification (SHA-256)
- Parse-health command for parser diagnostics
- Doctor command for installation health checks

### Distribution
- Pre-compiled binaries for all platforms (macOS x86/ARM, Linux x86/ARM, Windows)
- GitHub Actions release pipeline (DEB/RPM packages, checksums)
- Quick install script (`install.sh`)

---

## In Progress

### Codebase Health
- Splitting large files (>400 lines) into focused modules (see `.plans/split-large-files-v2.md`)
- Parser migration: remaining filters to OutputParser trait (tsc, lint, gh)

---

## Planned

### New Filters
- diffsitter (AST-aware diffs)
- ripgrep (rg) dedicated filter
- bun package manager support
- podman container support

### Analytics
- Per-project tracking isolation
- Web dashboard (localhost) for visualizing trends
- Prometheus/OpenMetrics export format

### Developer Experience
- Plugin system for user-defined filters
- Config-driven filter customization
- Better error messages and diagnostics
