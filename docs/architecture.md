# Mycelium Architecture

Mycelium is a single-binary CLI proxy with a wide module surface. It sits
between an agent and the shell, then trades raw terminal output for structured,
lower-token summaries while preserving command behavior and exit codes. This
document covers the routing pipeline, parser and filter seams, and the parts of
the system that affect extension work.

One package does not mean one file. The router is deliberately small, the
command-family modules own the concrete behavior, and sibling integrations stay
behind dedicated adapters so they do not become hidden policy in dispatch.

---

## Design Principles

- **Fail safe, not clever** — if a parser or filter does not understand the
  output, Mycelium falls back instead of inventing structure.
- **Exit code preservation** — compression is only useful if CI and agent
  workflows still observe the real command result.
- **Command-specific filtering beats generic truncation** — Git, test runners,
  linters, and package managers each get strategies tuned to their output.
- **Adaptive compression** — small output passes through, medium output is
  filtered lightly, and large output gets the heavier treatment.
- **Token tracking as a first-class feature** — the proxy measures what it
  saved so contributors can tune filters against real gains.

---

## System Boundary

### Mycelium owns

- Command dispatch and proxy execution
- Output parsers and filter strategies
- Tracking and savings analytics
- Rewrite guidance and local init flows
- Optional routing of oversized output to sibling tools

Dispatch should stay a routing layer, not the place where Hyphae, Rhizome, or
other integration policy is invented.

### Hyphae owns

- Long-lived storage for large outputs and recalled context

### Rhizome owns

- Structural code intelligence when reading a file is better than dumping it

### Stipe owns

- Shared ecosystem installation and host repair

Mycelium should summarize shell output and decide when to hand work off. It
should not become a memory store, an IDE indexer, or a general-purpose
orchestrator.

---

## Workspace Structure

```text
src/
├── main.rs          # CLI parse entry point
├── dispatch.rs      # Central router for every command family
├── commands.rs      # Clap command model
├── parser/          # OutputParser trait and parse result tiers
├── filter.rs        # Filter strategies and language-aware code filtering
├── tracking/        # SQLite token history and timers
├── fileops/         # ls, read, grep, diff, and related commands
├── vcs/             # git, gh, gt filters
├── js/ python/ go_eco/  # Language-ecosystem handlers
└── gain/ discover/ learn/ cc_economics/  # Analytics and explain surfaces
```

Mycelium compiles into one binary, but the internal shape is intentionally
modular because command families differ sharply in output style. The router
remains a hotspot to protect, not a place to accumulate new policy.

- **`dispatch.rs`**: The traffic director. Every parsed command lands here
  before it reaches a family-specific handler.
- **`parser/`**: Structured parsing with a three-tier degradation model.
- **`filter.rs`**: Code and text compaction primitives shared across command
  handlers.
- **`tracking/`**: Savings history, timer helpers, and database path
  resolution.
- **Command families**: Own the actual transformation logic for Git, test
  tools, package managers, and file operations.

---

## Core Abstraction

```rust
pub trait OutputParser: Sized {
    type Output;

    fn parse(input: &str) -> ParseResult<Self::Output>;
}
```

Parsers are expected to degrade cleanly:

- `ParseResult::Full` means the output was understood and structured cleanly.
- `ParseResult::Degraded` means partial structure was extracted with warnings.
- `ParseResult::Passthrough` means Mycelium could not safely parse the output
  and returned raw content with a marker.

That contract matters more than any single parser implementation. It keeps
filters from silently lying when tool output shifts.

---

## Request Flow

When a command reaches Mycelium:

1. **Parse CLI input** (`main::main`)
   Clap parses the command surface. If parsing fails for a non-meta command,
   Mycelium may fall back to raw passthrough instead of blocking execution.
   Example: malformed args for an unknown external command still run through
   `run_fallback`.

2. **Route by command family** (`dispatch::dispatch`)
   The central dispatcher chooses the right module for Git, file ops, JS, Go,
   Python, infra tools, or analytics.
   Example: `Commands::Git` lands in `dispatch_git_commands`.

3. **Execute the underlying tool** (family-specific `run(...)`)
   The target command runs with inherited semantics and captured output.
   Example: a Git handler runs `git`, a Ruff handler may choose JSON mode, and
   a read flow may use Rhizome for structure instead of raw file dumps.

4. **Filter or parse output** (`parser/`, `filter.rs`, command modules)
   Command-specific logic extracts counts, errors, summaries, or structural
   detail.
   Example: pytest uses a stateful parser; code reads use language-aware
   comment and body folding.

5. **Track savings** (`tracking::TimedExecution::track`)
   Input and output token counts are estimated and written to the SQLite
   history store.
   Example: `mycelium gain` later reports aggregate savings from those rows.

6. **Return compact output**
   The terminal sees the filtered result, but the original exit code is still
   preserved.

---

## Filtering Pipeline

File: `src/filter.rs` and `src/parser/`

### How It Works

1. **Classify the output** — command family and output size determine which
   strategy applies.
2. **Pick the right parser** — JSON if the tool supports it, text extraction if
   it does not, passthrough if neither is safe.
3. **Apply code-aware filtering when needed** — language-specific comment and
   body folding are used for source reads.
4. **Emit quality metadata** — filters can report full, degraded, or
   passthrough quality.

### Strategy Matrix

| Strategy | Typical Use | Behavior |
|----------|-------------|----------|
| Stats extraction | `git status`, `git log`, dependency lists | Count and summarize, drop line-by-line detail |
| Error focus | build and test failures | Keep actionable failures, hide passing noise |
| Grouping | linters and type-checkers | Group by rule, file, or error class |
| Structure-only | JSON or code reads | Keep keys, headings, and shape while dropping bulk values |
| Progress filtering | installers and download tools | Strip bars and transient progress output |

### Adding a New Parser-Backed Command

1. Add the command surface to `commands.rs`.
2. Route it through `dispatch.rs`.
3. Implement the handler and, if needed, an `OutputParser`.
4. Add token-tracking coverage and a failure-mode test before calling it done.

If the new behavior is large enough to obscure the router or adapter module,
move the higher-level regression coverage into `tests/` instead of keeping it
inline with the hotspot.

---

## Configuration

Config file: `~/.config/mycelium/config.toml`

```toml
[tracking]
enabled = true
history_days = 90

[filters]
compaction_profile = "balanced"
show_filter_header = true

[filters.adaptive]
small_lines = 50
small_bytes = 2048
large_lines = 500
```

Environment variables override config:

- `MYCELIUM_DB_PATH` — use a non-default tracking database path
- `MYCELIUM_PROJECT_PATH` — override the project path used for tracking scope
- `MYCELIUM_DISABLED=1` — disable rewrite for a single command invocation
- `MYCELIUM_HOOK_AUDIT=1` — enable rewrite audit capture

---

## Testing

```bash
cargo test
cargo build --release
cargo clippy
```

| Category | Count | What's Tested |
|----------|-------|---------------|
| Unit | 700+ | Parsers, filters, routing branches, config loading, tracking helpers |
| Integration and snapshots | 200+ | Command family behavior, passthrough fallbacks, snapshot output, rewrite behavior |
| Edge cases | 100+ | Unicode, multibyte truncation, malformed JSON, parser degradation, env overrides |
| Analytics and tracking | 50+ | Token estimation, history persistence, gain reporting, DB path resolution |

Fixtures live under `tests/fixtures/` and snapshots under `tests/snapshots/`.
When filter behavior changes intentionally, update snapshots with the same care
you would apply to CLI-facing contract tests.

---

## Key Dependencies

- **`clap`** — command parsing across a very large CLI surface.
- **`rusqlite`** — bundled SQLite keeps savings history and tracking analytics local and portable; that tradeoff is intentional.
- **`tree-sitter`** — powers structural filtering for code reads.
- **`spore`** — shared token estimation and ecosystem primitives.
- **`insta`** — snapshot testing for noisy CLI outputs that would otherwise be
  hard to review.
