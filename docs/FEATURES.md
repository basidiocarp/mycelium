# Mycelium — Feature Overview

Mycelium filters and compresses command output before it reaches your LLM. Single Rust binary with no external runtime dependencies, typically ~5-15ms proxy overhead. 60–90% token reduction on typical dev operations.

---

## Documentation

| Document | Contents |
|----------|----------|
| **[COMMANDS.md](COMMANDS.md)** | Public command reference |
| **[ANALYTICS.md](ANALYTICS.md)** | Analytics, hook system, configuration, tee system |
| **[ARCHITECTURE.md](ARCHITECTURE.md)** | System design, modules, filtering strategies |
| **[EXTENDING.md](EXTENDING.md)** | Adding new commands, development patterns, ADRs |
| **[PLUGINS.md](PLUGINS.md)** | Custom filter plugins |
| **[COST_ANALYSIS.md](COST_ANALYSIS.md)** | Token economics and accuracy |

---

## Overview

mycelium acts as a proxy between an LLM (Claude Code, Gemini CLI, etc.) and system commands. Mycelium uses five core filtering strategies:

| Strategy | Description | Example |
|----------|-------------|---------|
| Smart filtering | Removes noise (comments, whitespace, boilerplate) | `ls -la` -> compact tree |
| Grouping | Aggregation by directory, error type, or rule | Tests grouped by file |
| Truncation | Keeps relevant context, removes redundancy | Condensed diff |
| Deduplication | Merges repeated log lines with counters | `error x42` |
| Adaptive filtering | Size-aware compression with actionable content preservation | Small outputs pass through, large outputs get full compression |

Companion integrations:
- Hyphae routing for large-output chunk storage and retrieval
- Rhizome structural reads for large code files

### Adaptive filtering

Outputs are classified by size before filtering:
- Small (<50 lines / <2KB): pass through unfiltered
- Medium (50-500 lines): light command-specific filtering
- Large (>500 lines): full structured compression

The code filter (`filter.rs`) preserves actionable comments (TODO, FIXME, HACK, SAFETY, NOTE, BUG, WARNING) while stripping noise (separators, auto-generated markers, pragma directives). License headers are detected and removed. The aggressive filter folds function bodies >30 lines to `// ... (N lines)` instead of removing them entirely.

### Fallback mechanism

If mycelium does not recognize a subcommand, it executes the raw command unchanged and records the event in the tracking database.

---

## Global Flags

These flags apply to all subcommands:

| Flag | Short | Description |
|------|-------|-------------|
| `--verbose` | `-v` | Increase verbosity (-v, -vv, -vvv). Shows filtering details. |
| `--ultra-compact` | `-u` | Ultra-compact mode: ASCII icons, inline format. Additional savings. |
| `--skip-env` | -- | Sets `SKIP_ENV_VALIDATION=1` for child processes (Next.js, tsc, lint, prisma). |

```bash
mycelium -v git status          # Compact status + filtering details on stderr
mycelium -vvv cargo test        # Maximum verbosity (debug)
mycelium -u git log             # Ultra-compact log, ASCII icons
mycelium --skip-env next build  # Disable Next.js env validation
```

---

## Savings Summary by Category

| Category | Commands | Typical Savings |
|----------|----------|----------------|
| Files | ls, tree, read, find, grep, diff | 30-80% |
| Git | status, log, diff, show, add, commit, push, pull | 40-92% |
| GitHub | pr, issue, run, api | 26-87% |
| Tests | cargo test, vitest, playwright, pytest, go test | 50-99% |
| Build/Lint | cargo build, tsc, eslint, prettier, next, ruff, clippy | 70-87% |
| Packages | pnpm, npm, pip, deps, prisma | 60-80% |
| Containers | docker, kubectl | 60-90% |
| Data | json, env, log, curl, wget | 40-85% |
| Analytics | gain, discover, learn, economics | N/A (meta) |

---

## Total Command Count

50+ public top-level commands across 11 categories, plus tool-specific subcommands. Unrecognized subcommands pass through unchanged.
