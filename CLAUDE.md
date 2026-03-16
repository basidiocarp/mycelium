# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

Mycelium is a high-performance CLI proxy that filters and compresses command output to minimize LLM token consumption. Single Rust binary, zero runtime dependencies, 60-90% token savings on common dev operations. Integrates with Claude Code via hooks that transparently rewrite commands.

## Build & Test Commands

```bash
cargo build --release           # Optimized build
cargo install --path .          # Install locally
cargo test                      # All unit tests
cargo test --ignored            # Integration tests (requires installed binary)

cargo clippy                    # Lint (pedantic)
cargo fmt --check               # Format check
cargo fmt                       # Auto-format
cargo insta review              # Review snapshot test changes
```

## Architecture

Single-crate binary with filter modules per command family:

```
src/
├── main.rs          # CLI entry point, command routing
├── lib.rs           # Library root
├── filters/         # Output filter implementations
│   ├── git.rs       # git log, status, diff, show, branch, etc.
│   ├── cargo.rs     # cargo build, test, clippy, check
│   ├── gh.rs        # gh pr, issue, run
│   ├── docker.rs    # docker ps, images, logs
│   ├── js.rs        # pnpm, npm, vitest, playwright
│   └── ...          # Other filter modules
├── gain.rs          # Token savings tracking (SQLite)
├── discover.rs      # Analyze Claude Code sessions for missed usage
├── init.rs          # CLAUDE.md injection
└── snapshots/       # Insta snapshot files
```

**How it works**: Mycelium intercepts a command, runs it, captures stdout/stderr, applies the matching filter to compress the output, tracks token savings in SQLite, and returns the filtered result. If no filter matches, output passes through unchanged.

## Key Design Decisions

- **SQLite** for token savings tracking (via `rusqlite`, bundled)
- **Regex-based filters** for output compression (via `regex` crate)
- **ANSI stripping** before filtering, re-application after (via `strip-ansi-escapes`)
- **Release profile**: LTO, single codegen unit, stripped symbols for minimal binary (<5MB target)

## Performance Targets

| Metric | Target |
|--------|--------|
| Startup time | <10ms |
| Memory usage | <5MB |
| Binary size | <5MB |

## Ecosystem Integration

Mycelium integrates with sibling tools when they're installed:

- **Hyphae**: Large outputs (>500 lines) are chunked and stored in Hyphae instead of being destructively filtered. Agents get a summary + `hyphae_get_command_chunks()` retrieval key. Falls back to local filtering when Hyphae is unavailable.
- **Rhizome**: `mycelium read` on code files (>=200 lines) delegates to Rhizome for structured symbol outlines instead of filtered text. Falls back to local filtering when Rhizome is unavailable.

Configure via `~/.config/mycelium/config.toml`:
```toml
[filter.hyphae]
enabled = true   # auto-detect (default), true (force), false (disable)

[filter.rhizome]
enabled = true
```

### Ecosystem Setup

```bash
mycelium init --ecosystem    # Detect tools, configure Claude Code MCP servers, initialize databases
```

## Testing Strategy

- **Snapshot tests** (`insta`): Primary strategy for filter output validation
- **Token accuracy tests**: Every filter verifies >= 60% savings with real fixtures
- **Integration tests** (`#[ignore]`): End-to-end with installed binary
- **Fixtures**: Real command output in `tests/fixtures/`, never synthetic data

---

<!-- mycelium-instructions v2 -->
# Mycelium - Token-Optimized Commands

## Golden Rule

**Always prefix commands with `mycelium`**. If Mycelium has a dedicated filter, it uses it. If not, it passes through unchanged. This means Mycelium is always safe to use.

**Important**: Even in command chains with `&&`, use `mycelium`:
```bash
# Wrong
git add . && git commit -m "msg" && git push

# Correct
mycelium git add . && mycelium git commit -m "msg" && mycelium git push
```

## Mycelium Commands by Workflow

### Build & Compile (80-90% savings)
```bash
mycelium cargo build         # Cargo build output
mycelium cargo check         # Cargo check output
mycelium cargo clippy        # Clippy warnings grouped by file (80%)
mycelium tsc                 # TypeScript errors grouped by file/code (83%)
mycelium lint                # ESLint/Biome violations grouped (84%)
mycelium prettier --check    # Files needing format only (70%)
mycelium next build          # Next.js build with route metrics (87%)
```

### Test (90-99% savings)
```bash
mycelium cargo test          # Cargo test failures only (90%)
mycelium vitest run          # Vitest failures only (99.5%)
mycelium playwright test     # Playwright failures only (94%)
mycelium test <cmd>          # Generic test wrapper - failures only
```

### Git (59-80% savings)
```bash
mycelium git status          # Compact status
mycelium git log             # Compact log (works with all git flags)
mycelium git diff            # Compact diff (80%)
mycelium git show            # Compact show (80%)
mycelium git add             # Ultra-compact confirmations (59%)
mycelium git commit          # Ultra-compact confirmations (59%)
mycelium git push            # Ultra-compact confirmations
mycelium git pull            # Ultra-compact confirmations
mycelium git branch          # Compact branch list
mycelium git fetch           # Compact fetch
mycelium git stash           # Compact stash
mycelium git worktree        # Compact worktree
```

Note: Git passthrough works for ALL subcommands, even those not explicitly listed.

### GitHub (26-87% savings)
```bash
mycelium gh pr view <num>    # Compact PR view (87%)
mycelium gh pr checks        # Compact PR checks (79%)
mycelium gh run list         # Compact workflow runs (82%)
mycelium gh issue list       # Compact issue list (80%)
mycelium gh api              # Compact API responses (26%)
```

### JavaScript/TypeScript Tooling (70-90% savings)
```bash
mycelium pnpm list           # Compact dependency tree (70%)
mycelium pnpm outdated       # Compact outdated packages (80%)
mycelium pnpm install        # Compact install output (90%)
mycelium npm run <script>    # Compact npm script output
mycelium npx <cmd>           # Compact npx command output
mycelium prisma              # Prisma without ASCII art (88%)
```

### Files & Search (60-75% savings)
```bash
mycelium ls <path>           # Tree format, compact (65%)
mycelium read <file>         # Code reading with filtering (60%), or Rhizome structure outline when available
mycelium grep <pattern>      # Search grouped by file (75%)
mycelium find <pattern>      # Find grouped by directory (70%)
```

### Analysis & Debug (70-90% savings)
```bash
mycelium err <cmd>           # Filter errors only from any command
mycelium log <file>          # Deduplicated logs with counts
mycelium json <file>         # JSON structure without values
mycelium deps                # Dependency overview
mycelium env                 # Environment variables compact
mycelium summary <cmd>       # Smart summary of command output
mycelium diff                # Ultra-compact diffs
```

### Infrastructure (85% savings)
```bash
mycelium docker ps           # Compact container list
mycelium docker images       # Compact image list
mycelium docker logs <c>     # Deduplicated logs
mycelium kubectl get         # Compact resource list
mycelium kubectl logs        # Deduplicated pod logs
```

### Network (65-70% savings)
```bash
mycelium curl <url>          # Compact HTTP responses (70%)
mycelium wget <url>          # Compact download output (65%)
```

### Meta Commands
```bash
mycelium gain                # View token savings statistics
mycelium gain --history      # View command history with savings
mycelium discover            # Analyze Claude Code sessions for missed Mycelium usage
mycelium proxy <cmd>         # Run command without filtering (for debugging)
mycelium init                # Add Mycelium instructions to CLAUDE.md
mycelium init --global       # Add Mycelium to ~/.claude/CLAUDE.md
```

### Hyphae Integration (Optional)

When [Hyphae](https://github.com/basidiocarp/hyphae) is installed, Mycelium automatically routes large command outputs (>500 lines) through Hyphae's chunked storage instead of applying destructive local filtering. This preserves the full output for later retrieval while still giving you a concise summary.

**How it works:**
- Small outputs (<50 lines): pass through unchanged
- Medium outputs (50–500 lines): filtered locally (existing behavior)
- Large outputs (>500 lines + Hyphae available): stored in Hyphae, summary returned

**Summary format:**
```
[mycelium→hyphae] cargo test: 247 tests passed, 3 failed. Use hyphae_get_command_chunks(document_id="abc123") for details.
```

**Without Hyphae**: All existing behavior is unchanged. Mycelium remains fully standalone.

**Configuration** (`~/.config/mycelium/config.toml`):
```toml
[filters.hyphae]
# enabled = true   # Force Hyphae on (default: auto-detect)
# enabled = false  # Force Hyphae off (always use local filtering)
```

### Rhizome Integration (Optional)

When [Rhizome](https://github.com/basidiocarp/rhizome) is installed, `mycelium read` automatically uses Rhizome's code intelligence (tree-sitter + LSP) for code files ≥200 lines instead of applying destructive comment/body filtering.

**How it works:**
- Non-code files (md, json, toml, yaml, etc.): always use existing filter
- Small code files (<200 lines): pass through unchanged (existing behavior)
- Large code files (≥200 lines + Rhizome available): structural outline via Rhizome

**Output format:**
```
[rhizome] main.rs — use get_definition("main.rs", "<symbol>") for full source

fn main() -> Result<()>
mod config
pub struct FilterConfig
pub fn classify(content: &str) -> AdaptiveLevel
...
```

**Without Rhizome**: All existing behavior is unchanged.

**Configuration** (`~/.config/mycelium/config.toml`):
```toml
[filters.rhizome]
# enabled = true   # Force Rhizome on (default: auto-detect)
# enabled = false  # Force Rhizome off (always use local filtering)
```

## Token Savings Overview

| Category | Commands | Typical Savings |
|----------|----------|-----------------|
| Tests | vitest, playwright, cargo test | 50–99% |
| Build | next, tsc, lint, prettier | 70–87% |
| Git | status, log, diff, add, commit | 40–80% |
| GitHub | gh pr, gh run, gh issue | 26–87% |
| Package Managers | pnpm, npm, npx | 70–90% |
| Files | ls, read, grep, find | 30–75% |
| Infrastructure | docker, kubectl | 60–90% |
| Network | curl, wget | 40–85% |

Overall average: **60-90% token reduction** on common development operations.

Small outputs (<50 lines / <2KB) pass through unfiltered. Larger outputs are adaptively compressed.
<!-- /mycelium-instructions -->
