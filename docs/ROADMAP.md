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
- Keep tightening parser-backed consistency where older formatter logic or bespoke output shaping still bypasses shared `OutputParser` types and formatters

### Portability and Runtime
- Finish removing residual Unix-shaped runtime behavior from less common command paths
- Keep aligning host setup flows behind `stipe` while preserving Mycelium-specific diagnostics and explainability

---

## Planned

### Tool Coverage
- Keep `rg` alias-backed through `mycelium grep`; only add a separate first-class `rg` command if grouped grep output proves materially worse than true ripgrep semantics in practice
- Keep supported `bun run ...` / `bunx ...` wrappers and `podman` alias-backed unless their output diverges enough from the current rewrite targets to justify dedicated built-in commands
- Keep `diffsitter` alias-backed to `mycelium diff` for now; only add dedicated semantic-diff handling if real usage shows plain diff-style condensation is not good enough

### Analytics
- Extend per-project tracking beyond the current rewrite and parse-failure diagnostics so more analytics views can be scoped cleanly by workspace
- Feed Mycelium diagnostics and trend views into `cap` instead of maintaining a separate local dashboard

### Developer Experience
- Plugin system for user-defined filters and experimental adapters
- Keep the plugin system as a custom fallback layer, not the primary implementation path for built-in integrations
- Config-driven filter customization
- Better error messages and diagnostics
- Deeper host-neutral runtime integration so Claude, Codex, and future hosts map cleanly into the same measurement model

### Adaptive Filtering Roadmap

#### Near-Term: Outer-Loop Token Optimization
- Replace mostly line/byte-based routing with token-budget-aware routing using existing token estimation and command-family heuristics
- Add salience-aware compaction for diffs, logs, and test output so truncation preserves the most actionable context instead of only applying fixed caps
- Expand the shared parser degradation model (`Full`, `Degraded`, `Passthrough`) across more command families so fallback behavior is more consistent
- Tune compaction thresholds from tracking and parse-failure telemetry instead of relying only on static profile defaults
- Improve Hyphae summaries so large-output chunking returns structured retrieval hints, not just a generic summary line
- Add task-shaped compression modes for common intents such as debug, review, fix, and status

#### Later: Local Summarization and Retrieval
- Add a lightweight local reranker or summarizer for large outputs before chunking when heuristic filters are not specific enough
- Attach richer metadata to chunked output so follow-up retrieval can target failures, changed files, warnings, or hot sections directly
- Keep these additions outside the model runtime itself unless local inference becomes a primary Mycelium feature

#### Conditional: Only If Mycelium Becomes a Local Model Runtime
- Evaluate plug-and-play KV cache compression techniques such as FastGen before attempting deeper runtime changes
- Consider dynamic layer execution techniques only after local inference is real, profiled, and bottlenecked on model execution rather than output shaping
- Treat multimodal token-compression work such as ACT-IN-LLM or AdaTok as out of scope unless image or screenshot workflows become a core product path
- Ignore token-level pretraining data filtering unless Mycelium starts training or fine-tuning its own models

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
