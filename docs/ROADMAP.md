# Mycelium Roadmap

## Completed

### Language Ecosystem Coverage
- Git, GitHub CLI, Graphite (stacked PRs)
- Rust (cargo test/build/clippy/check/nextest)
- JavaScript/TypeScript (vitest, playwright, tsc, eslint, biome, prettier, next.js, prisma, pnpm, npm)
- Python (ruff, pytest, pip/uv, mypy, pylint)
- Go (go test/build/vet, golangci-lint)
- Infrastructure (docker, kubectl, aws, terraform, psql)

### Architecture
- Hook-based transparent command rewriting (PreToolUse)
- Parser framework (`OutputParser` trait, `ParseResult<T>`, `TokenFormatter`)
- SQLite token tracking with daily/weekly/monthly breakdowns + JSON/CSV export
- Tee system for raw output recovery on failure
- Discover: analyze Claude Code history for missed optimization opportunities
- Learn: detect recurring CLI error patterns and generate correction rules
- CC-Economics: compare Claude Code spending with Mycelium savings
- Modular directory structure (vcs/, js/, python/, go_eco/, etc.)

### Quality
- 105+ unit tests across 25+ files
- Smoke test suite (69 assertions)
- Hook integrity verification (SHA-256)
- Parse-health command for parser diagnostics

---

## In Progress

### Parser Migration (Phase 4-5)
- Migrate remaining filters to OutputParser trait (tsc, lint, gh)
- Add parse_tier tracking to SQLite schema
- Parse-health diagnostics command

### Codebase Health
- Splitting large files (>400 lines) into focused modules
- Discover pattern gap coverage (bun, podman, python3 -m, etc.)

---

## Planned

### Stability & Distribution
- Homebrew formula for one-click install
- Pre-compiled binaries for all platforms
- Automated release pipeline improvements

### New Filters
- diffsitter (AST-aware diffs)
- ripgrep (rg) dedicated filter
- cargo nextest improvements
- bun package manager support

### Analytics
- Per-project tracking isolation
- Web dashboard (localhost) for visualizing trends
- Prometheus/OpenMetrics export format

### Developer Experience
- Plugin system for user-defined filters
- Config-driven filter customization
- Better error messages and diagnostics
