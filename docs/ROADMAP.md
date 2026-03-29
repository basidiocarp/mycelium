# Mycelium Roadmap

## Completed

### Language Ecosystem Coverage
- **VCS**: Git, GitHub CLI (gh), Graphite (gt) stacked PRs
- **Rust**: cargo test/build/clippy/check/install/nextest
- **JavaScript/TypeScript**: vitest, playwright, tsc, eslint, biome, prettier, next.js, prisma, pnpm, npm, npx
- **Python**: ruff check/format, pytest, pip/uv, mypy
- **Go**: go test/build/vet, golangci-lint
- **Infrastructure**: docker, kubectl, aws (sts/s3/ec2/ecs/rds/cloudformation), terraform (plan/apply/init), atmos (terraform/describe/validate/workflow/version)
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
- Shared platform-aware path and shell helpers instead of scattered Unix-only assumptions
- More host-neutral onboarding and runtime wording across Claude and Codex flows
- `spore`-backed editor/config registration for the shared host overlap set

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
- Parser migration: finish normalizing the remaining mixed parser paths in lint and `gh` so parser-heavy commands consistently use shared `OutputParser` types and formatters

### Portability and Runtime
- Finish removing residual Unix-shaped runtime behavior from less common command paths
- Keep aligning host setup flows behind `stipe` while preserving Mycelium-specific diagnostics and explainability

---

## Planned

### Tool Coverage
- Decide whether alias-backed coverage is sufficient for `bun`, `podman`, `diffsitter`, and `rg`, or whether any of them need first-class commands beyond the current rewrite mappings
- Add dedicated `diffsitter` formatting only if plain diff-style output is not good enough in practice
- Add dedicated `rg` formatting only if grep-style grouping proves materially worse than real ripgrep semantics

### Analytics
- Per-project tracking isolation
- Feed Mycelium diagnostics and trend views into `cap` instead of maintaining a separate local dashboard

### Developer Experience
- Plugin system for user-defined filters and experimental adapters
- Keep the plugin system as a custom fallback layer, not the primary implementation path for built-in integrations
- Config-driven filter customization
- Better error messages and diagnostics
- Deeper host-neutral runtime integration so Claude, Codex, and future hosts map cleanly into the same measurement model

### Competitive Priorities

#### Copy
- Add operator-facing quality metrics for rewrites and passthroughs, not just token savings
- Improve onboarding and setup ergonomics across supported hosts
- Expand ecosystem coverage selectively where CLI parity and raw-output semantics are well understood
- Track missed-opportunity and passthrough cases explicitly so rewrite rules can be tuned from real usage

#### Avoid
- Do not trade semantic fidelity for headline token savings
- Do not broaden hook or rewrite behavior faster than it can be verified with regression tests
- Do not infer shell safety from string matching alone when parser-backed validation is available
- Do not expand host and tool support faster than exact-output and exit-code parity can be preserved

#### Watch
- Cross-agent host support, where the integration value is high but the hook and permission surface grows quickly
- Windows support, where installation and runtime compatibility tend to regress first
- User-configurable rewrite and filter policy, especially where trust boundaries are unclear
- Release velocity around core rewrite logic, so rapid iteration does not reintroduce semantic-loss bugs

#### Near-Term Order
1. Add rewrite quality scoring and passthrough diagnostics
2. Improve onboarding, install, and init flows
3. Expand ecosystem coverage only after command-parity tests exist
4. Keep growing the semantic-fidelity regression suite for rewritten commands
