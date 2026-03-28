# Mycelium Architecture

Mycelium sits between an LLM and system commands, filtering output to reduce token consumption. For adding new commands, see [EXTENDING.md](EXTENDING.md).

---

## Table of Contents

1. [System Overview](#system-overview)
2. [Command Lifecycle](#command-lifecycle)
3. [Module Organization](#module-organization)
4. [Filtering Strategies](#filtering-strategies)
5. [Python and Go Module Architecture](#python-and-go-module-architecture)
6. [Shared Infrastructure](#shared-infrastructure)
7. [Token Tracking System](#token-tracking-system)
8. [Global Flags Architecture](#global-flags-architecture)
9. [Error Handling](#error-handling)
10. [Configuration System](#configuration-system)
11. [Build Optimizations](#build-optimizations)
12. [Resources](#resources)
13. [Glossary](#glossary)

---

## System Overview

### Proxy Pattern Architecture

```mermaid
flowchart LR
    A["$ mycelium git log\n-v --oneline"] --> B["Clap Parser\n(main.rs)"]
    B --> C["Commands\nenum match"]
    C --> D["git::run()"]
    D --> E["Execute:\ngit log"]
    E --> F["Filter/Compress"]
    F --> G["Compact Stats\n(90% reduction)"]
    G --> H["tracking::track()"]
    H --> I["SQLite INSERT\nplatform data dir / history.db"]
    G --> J["Terminal Output\n3 commits +142/-89"]
```

### Key Components

| Component            | Location                                           | Responsibility                                     |
|----------------------|----------------------------------------------------|----------------------------------------------------|
| **CLI Parser**       | main.rs, commands.rs                               | Clap-based argument parsing, global flags          |
| **Command Router**   | main.rs, dispatch.rs                               | Dispatch to specialized modules                    |
| **Module Layer**     | src/*_cmd.rs, src/vcs/, src/js/, etc.              | Command execution + filtering                      |
| **Shared Utils**     | utils.rs                                           | Package manager detection, text processing         |
| **Filter Engine**    | filter.rs                                          | Language-aware code filtering                      |
| **Parser Framework** | parser/ (mod.rs, types.rs, formatter.rs)           | OutputParser trait, ParseResult<T>, TokenFormatter |
| **Tracking**         | tracking/ (mod.rs, queries.rs, timer.rs, utils.rs) | SQLite-based token metrics                         |
| **Analytics**        | gain/, discover/, learn/, cc_economics/            | Token savings analysis, economics reporting, and explain surfaces |
| **Config**           | config.rs, init/                                   | User preferences, LLM integration                  |

### Design Principles

1. Single responsibility: each module handles one command type
2. Minimal overhead: ~5-15ms proxy overhead per command
3. Exit code preservation: CI/CD reliability through proper exit code propagation
4. Fail-safe: if filtering fails, fall back to original output
5. Transparent: users can always see raw output with `-v` flags

### Hook Architecture (v0.9.5+)

The recommended deployment mode uses a Claude Code PreToolUse hook for 100% transparent command rewriting.

```mermaid
sequenceDiagram
    participant CC as Claude Code
    participant SJ as settings.json
    participant HK as mycelium-rewrite.sh
    participant MY as Mycelium binary

    CC->>SJ: Shell command: "git status"
    SJ->>HK: PreToolUse hook
    HK->>HK: detect: git → rewrite
    HK-->>SJ: updatedInput: "mycelium git status"
    SJ->>MY: execute: mycelium git status
    MY->>MY: run git, filter, track
    MY-->>CC: "3 modified, 1 untracked ✓" (~10 tokens vs ~200 raw)

    Note over CC: Claude never sees the rewrite —<br/>it only sees optimized output.
```

**Installed files:**
| File | Purpose |
|------|---------|
| `~/.claude/hooks/mycelium-rewrite.sh` | Thin delegator (calls `mycelium rewrite`) |
| `~/.claude/settings.json` | Hook registry (PreToolUse registration) |
| `~/.claude/Mycelium.md` | Minimal context hint (10 lines) |

Two hook strategies:

```mermaid
flowchart LR
    subgraph AR["Auto-Rewrite (default)"]
        A1["Hook intercepts command"] --> A2["Rewrites before execution"]
        A2 --> A3["100% adoption\nZero context overhead"]
    end
    subgraph SG["Suggest (non-intrusive)"]
        S1["Hook emits systemMessage hint"] --> S2["Claude decides autonomously"]
        S2 --> S3["~70-85% adoption\nMinimal context overhead"]
    end
```

---

## Command Lifecycle

### Six-Phase Execution Flow

```mermaid
flowchart TD
    P1["**Phase 1: PARSE**\n$ mycelium git log --oneline -5 -v\nClap extracts: Command=Git, Args, Flags"]
    P2["**Phase 2: ROUTE**\nmain.rs: match Commands::Git\n→ git::run(args, verbose)"]
    P3["**Phase 3: EXECUTE**\nCommand::new('git').args(['log','--oneline','-5'])\nCapture stdout (500 chars), stderr, exit_code"]
    P4["**Phase 4: FILTER**\ngit::format_git_output()\nStrategy: Stats Extraction\n→ '5 commits, +142/-89' (20 chars, 96% reduction)"]
    P5["**Phase 5: PRINT**\nTerminal: '5 commits, +142/-89 ✓'\n(verbose > 0 → debug on stderr)"]
    P6["**Phase 6: TRACK**\ntracking::track()\nSQLite INSERT: 125 input → 5 output = 96%\nplatform data dir / history.db"]

    P1 --> P2 --> P3 --> P4 --> P5 --> P6
```

### Verbosity Levels

```
-v (Level 1): Show debug messages
  Example: eprintln!("Git log summary:");

-vv (Level 2): Show command being executed
  Example: eprintln!("Executing: git log --oneline -5");

-vvv (Level 3): Show raw output before filtering
  Example: eprintln!("Raw output:\n{}", stdout);
```

---

## Module Organization

### Complete Module Map (30 Modules)

```mermaid
block-beta
    columns 4

    block:VCS:4
        columns 4
        vcs["VCS\nvcs/git/, vcs/gh_cmd/\nvcs/gt_cmd/"]
        fileops["File Ops\nfileops/ls, read\ngrep, diff, find"]
        exec["Execution\nrunner_cmd, summary_cmd\nlocal_llm"]
        data["Data\nstreaming, json_cmd\nenv_cmd, deps"]
    end

    block:LANG:4
        columns 4
        js["JS/TS Stack\njs/tsc, next, vitest\nlint_cmd/, pnpm/"]
        python["Python\npython/ruff, pytest\npip, mypy"]
        gomod["Go\ngo_eco/commands/\ngolangci"]
        rust["Rust\ncargo_eco/commands/\ncargo_filters/"]
    end

    block:INFRA:4
        columns 4
        containers["Containers\ncontainer_cmd/\ndocker, kubectl"]
        cloud["Cloud/Infra\naws_cmd/, psql_cmd\nterraform_cmd"]
        network["Network\ncurl_cmd, wget_cmd"]
        analytics["Analytics\ngain/, discover/\nlearn/, cc_economics/"]
    end

    block:SHARED:4
        columns 4
        utils["utils.rs\nShared helpers"]
        filter["filter.rs\nLanguage-aware\ncode filtering"]
        tracking["tracking/\nSQLite metrics"]
        parser["parser/\nOutputParser trait\ntee.rs"]
    end
```

| Category | Directory/Module | Commands | Savings |
|----------|-----------------|----------|---------|
| **Git (VCS)** | `vcs/git/`, `vcs/git_filters/` | status, diff, log, add, commit, push | 85-99% |
| **Code Search** | `fileops/grep_cmd.rs`, `diff_cmd.rs`, `find_cmd.rs` | grep, diff, find | 50-85% |
| **File Ops** | `fileops/ls.rs`, `read.rs` | ls, read | 40-90% |
| **Execution** | `runner_cmd.rs`, `summary_cmd.rs` | err, test, smart | 50-99% |
| **Logs/Data** | `streaming.rs`, `json_cmd.rs` | log, json | 70-95% |
| **JS/TS** | `js/`, `lint_cmd/` | tsc, next, vitest, lint, prettier, playwright, prisma, pnpm | 70-99% |
| **Python** | `python/` | ruff, pytest, pip, mypy | 70-90% |
| **Go** | `go_eco/` | go test/build/vet, golangci-lint | 75-90% |
| **Rust** | `cargo_eco/`, `cargo_filters/` | cargo build/test/clippy/check | 80-90% |
| **Containers** | `container_cmd/` | docker, kubectl | 60-80% |
| **Network** | `wget_cmd.rs`, `curl_cmd.rs` | wget, curl | 70-95% |
| **Infra** | `aws_cmd/`, `psql_cmd.rs`, `terraform_cmd.rs` | aws, psql, terraform | 75-80% |
| **GitHub CLI** | `vcs/gh_cmd/`, `vcs/gh_pr/` | gh pr/issue/run | 26-87% |
| **Analytics** | `gain/`, `discover/`, `learn/`, `cc_economics/` | gain, discover, learn, economics (`cc-economics` alias) | N/A |
| **Shared** | `utils.rs`, `filter.rs`, `tracking/`, `parser/`, `tee.rs` | (infrastructure) | N/A |

60+ modules across 17 directories

### Module Count Breakdown

- **Command Modules**: 40+ (directly exposed to users)
- **Infrastructure Modules**: 20+ (utils, filter, tracking, tee, config, init, gain, parser, etc.)
- **Git/VCS Commands**: 7 git operations + gh + gt (stacked PRs)
- **JS/TS Tooling**: 8 modules in `js/` + `lint_cmd/` (modern frontend/fullstack development)
- **Python Tooling**: 4 modules in `python/` + `lint_cmd/pylint.rs` (ruff, pytest, pip, mypy)
- **Go Tooling**: 2 modules in `go_eco/` (go test/build/vet, golangci-lint)
- **Analytics**: 4 modules (gain, discover, learn, economics / `cc-economics` alias)
- **Parser Framework**: `parser/` with OutputParser trait, ParseResult<T>, TokenFormatter

---

## Filtering Strategies

### Strategy Matrix

```mermaid
flowchart LR
    subgraph Strategies["12 Filtering Strategies"]
        direction TB
        S1["1. Stats Extraction\n90-99%"]
        S2["2. Error Only\n60-80%"]
        S3["3. Group by Pattern\n80-90%"]
        S4["4. Deduplication\n70-85%"]
        S5["5. Structure Only\n80-95%"]
        S6["6. Code Filtering\n0-90%"]
        S7["7. Failure Focus\n94-99%"]
        S8["8. Tree Compression\n50-70%"]
        S9["9. Progress Filtering\n85-95%"]
        S10["10. JSON/Text Dual\n80%+"]
        S11["11. State Machine\n90%+"]
        S12["12. NDJSON Streaming\n90%+"]
    end
```

| # | Strategy | Technique | Reduction | Used By |
|---|----------|-----------|-----------|---------|
| 1 | **Stats Extraction** | Count/aggregate, drop details | 90-99% | git status, git log, git diff, pnpm list |
| 2 | **Error Only** | stderr only, drop stdout | 60-80% | runner (err mode), test failures |
| 3 | **Grouping by Pattern** | Group by rule, count/summarize | 80-90% | lint, tsc, grep (by file/rule/error code) |
| 4 | **Deduplication** | Unique lines + count | 70-85% | log_cmd (pattern identification) |
| 5 | **Structure Only** | Keys + types, strip values | 80-95% | json_cmd (schema extraction) |
| 6 | **Code Filtering** | none/minimal/aggressive levels | 0-90% | read, smart (language-aware via filter.rs) |
| 7 | **Failure Focus** | Failures only, hide passing | 94-99% | vitest, playwright, runner (test mode) |
| 8 | **Tree Compression** | Tree hierarchy, aggregate dirs | 50-70% | ls (directory tree with counts) |
| 9 | **Progress Filtering** | Strip ANSI bars, final result | 85-95% | wget, pnpm install |
| 10 | **JSON/Text Dual** | JSON when available, text fallback | 80%+ | ruff (check->JSON, format->text), pip |
| 11 | **State Machine** | Track test lifecycle states | 90%+ | pytest (IDLE->TEST_START->PASSED/FAILED) |
| 12 | **NDJSON Streaming** | Line-by-line JSON parse, aggregate | 90%+ | go test (interleaved package events) |

### Code Filtering Levels (filter.rs)

```rust
// FilterLevel::None - Keep everything
fn calculate_total(items: &[Item]) -> i32 {
    // Sum all items
    items.iter().map(|i| i.value).sum()
}

// FilterLevel::Minimal - Strip noise comments, keep actionable ones (20-40% reduction)
// TODO: handle edge case           ← kept (actionable)
// ========================         ← stripped (noise separator)
fn calculate_total(items: &[Item]) -> i32 {
    items.iter().map(|i| i.value).sum()
}

// FilterLevel::Aggressive - Strip noise + fold large function bodies (60-90% reduction)
fn calculate_total(items: &[Item]) -> i32 {
    // small functions (≤30 lines) are kept inline
    items.iter().map(|i| i.value).sum()
}

fn large_function() {
    // ... (150 lines)              ← bodies >30 lines folded to a single line
}
```

The MinimalFilter distinguishes between noise comments (separators, auto-generated markers, pragma directives) and actionable comments (TODO, FIXME, HACK, SAFETY, NOTE, BUG, WARNING). License headers at the top of files are detected and stripped. The AggressiveFilter buffers function/impl bodies and folds those exceeding 30 lines to `// ... (N lines)` instead of dropping them entirely.

Language support: Rust, Python, JavaScript, TypeScript, Go, C, C++, Java

Detection is file extension-based with fallback heuristics

### Adaptive Output Sizing

Output sizing thresholds (`AdaptiveConfig`):

| Output Size | Action | Rationale |
|-------------|--------|-----------|
| <50 lines AND <2KB | Passthrough | Small outputs don't need filtering |
| 50-500 lines | Light filtering | Apply command-specific filters |
| >500 lines | Full compression | Structured filtering, dedup, truncation |

Thresholds are configurable in `config.toml` under `[filter.adaptive]`.

---

## Python and Go Module Architecture

### Design Rationale

**Added**: 2026-02-12 (v0.15.1)
**Motivation**: Complete language ecosystem coverage beyond JS/TS

Python and Go modules follow distinct architectural patterns optimized for their ecosystems:

```mermaid
flowchart TD
    subgraph PY["Python (Standalone Commands)"]
        direction TB
        PR["Commands::Ruff { args }"] --> PRF["python/ruff.rs"]
        PP["Commands::Pytest { args }"] --> PPF["python/pytest.rs"]
        PI["Commands::Pip { args }"] --> PIF["python/pip.rs"]
    end

    subgraph GO["Go (Sub-Enum Pattern)"]
        direction TB
        GC["Commands::Go { command }"] --> GT["GoCommand::Test"]
        GC --> GB["GoCommand::Build"]
        GC --> GV["GoCommand::Vet"]
        GL["Commands::GolangciLint { args }"] --> GLF["go_eco/golangci.rs"]
    end

    PY -.- M1["Mirrors: lint, prettier"]
    GO -.- M2["Mirrors: git, cargo"]
```

### Python Stack Architecture

```mermaid
flowchart TD
    subgraph Python["Python Commands (3 modules)"]
        direction TB
        R["python/ruff.rs\nJSON/Text Dual · 80%+"]
        P["python/pytest.rs\nState Machine · 90%+"]
        I["python/pip.rs\nJSON Parsing · 70-85%"]
    end

    R --> RC["ruff check → JSON API\nGroup by rule, count"]
    R --> RF["ruff format → Text\nExtract summary"]
    P --> PS["IDLE → TEST_START → PASSED/FAILED → SUMMARY\nFailures only"]
    I --> IL["pip list --format=json → Compact table"]
    I --> IS["pip show → Key fields only"]
    I --> IU["Auto-detect uv"]
```

### Go Stack Architecture

```mermaid
flowchart TD
    subgraph Go["Go Commands (2 modules)"]
        direction TB
        GE["go_eco/commands/\nSub-Enum Router · 75-90%"]
        GL["go_eco/golangci.rs\nJSON Parsing · 85%"]
    end

    GE --> GT["go test → NDJSON Streaming\nLine-by-line JSON, aggregate results"]
    GE --> GB["go build → Text Filtering\nErrors only with file:line"]
    GE --> GV["go vet → Text Filtering\nExtract file:line:message"]
    GL --> GLC["golangci-lint --out-format=json\nGroup by linter rule, count violations"]
```

### Sub-Enum Pattern (go_eco/commands/)

```rust
// main.rs enum definition
Commands::Go {
    #[command(subcommand)]
    command: GoCommand,
}

// go_eco/commands/ sub-enum
pub enum GoCommand {
    Test { args: Vec<String> },
    Build { args: Vec<String> },
    Vet { args: Vec<String> },
}

// Router
pub fn run(command: &GoCommand, verbose: u8) -> Result<()> {
    match command {
        GoCommand::Test { args } => run_test(args, verbose),
        GoCommand::Build { args } => run_build(args, verbose),
        GoCommand::Vet { args } => run_vet(args, verbose),
    }
}
```

### Format Strategy Decision Tree

```mermaid
flowchart TD
    A{"Output format known?"}
    A -->|"Tool provides JSON flag?"| B{"Structured data needed?"}
    B -->|Yes| C["Use JSON API\nruff check, pip list, golangci-lint"]
    B -->|No| D["Use text mode\nruff format, go build errors"]
    A -->|"Streaming events?"| E["Line-by-line NDJSON parse\ngo test (interleaved packages)"]
    A -->|"Plain text only?"| F{"Stateful parsing needed?"}
    F -->|Yes| G["State machine\npytest (test lifecycle tracking)"]
    F -->|No| H["Text filters\ngo vet, go build"]
```

### Performance Characteristics

| Command | Raw Time | Mycelium Time | Overhead | Savings |
|---------|----------|---------------|----------|---------|
| `ruff check` | 850ms | 862ms | +12ms | 83% |
| `pytest` | 1.2s | 1.21s | +10ms | 92% |
| `pip list` | 450ms | 458ms | +8ms | 78% |
| `go test` | 2.1s | 2.12s | +20ms | 88% |
| `go build` (errors) | 950ms | 961ms | +11ms | 80% |
| `golangci-lint` | 4.5s | 4.52s | +20ms | 85% |

---

## Shared Infrastructure

### Utilities Layer (utils.rs)

```mermaid
classDiagram
    class utils_rs {
        +truncate(s: &str, max: usize) String
        +strip_ansi(text: &str) String
        +execute_command(cmd, args) (stdout, stderr, exit_code)
    }
    note for utils_rs "Used by all 24+ command modules"
```

### Package Manager Detection Pattern

All 8 JS/TS modules use this detection pattern.

```mermaid
flowchart TD
    A{"pnpm-lock.yaml exists?"}
    A -->|Yes| B["pnpm exec -- tool"]
    A -->|No| C{"yarn.lock exists?"}
    C -->|Yes| D["yarn exec -- tool"]
    C -->|No| E["npx --no-install -- tool"]
```

Affects: lint, tsc, next, prettier, playwright, prisma, vitest, pnpm

---

## Token Tracking System

### SQLite-Based Metrics

```mermaid
flowchart TD
    E["**1. ESTIMATE**\nestimate_tokens(text)\n~4 chars = 1 token"]
    C["**2. CALCULATE**\ninput_tokens - output_tokens = saved\nsavings_pct = (saved / input) × 100"]
    R["**3. RECORD**\nINSERT INTO commands\n(timestamp, cmds, tokens, savings, exec_time_ms)"]
    S[("**4. STORAGE**\nplatform data dir / history.db")]
    CL["**5. CLEANUP**\nDELETE WHERE timestamp < now - 90 days\n(on every INSERT)"]
    RP["**6. REPORTING**\n$ mycelium gain\nSELECT COUNT, SUM, AVG FROM commands"]

    E --> C --> R --> S
    S --> CL
    S --> RP
```

**Schema: `commands` table**

| Column | Type | Description |
|--------|------|-------------|
| `id` | INTEGER PRIMARY KEY | Auto-increment |
| `timestamp` | TEXT NOT NULL | RFC3339 UTC |
| `original_cmd` | TEXT NOT NULL | e.g., "git log --oneline -5" |
| `mycelium_cmd` | TEXT NOT NULL | e.g., "mycelium git log --oneline -5" |
| `input_tokens` | INTEGER NOT NULL | Estimated input tokens |
| `output_tokens` | INTEGER NOT NULL | Actual output tokens |
| `saved_tokens` | INTEGER NOT NULL | input - output |
| `savings_pct` | REAL NOT NULL | (saved / input) x 100 |
| `exec_time_ms` | INTEGER DEFAULT 0 | Execution duration (added v0.7.1) |

### Thread Safety

```rust
// tracking.rs:9-11
lazy_static::lazy_static! {
    static ref TRACKER: Mutex<Option<Tracker>> = Mutex::new(None);
}
```

Single-threaded execution. The Mutex is for future-proofing.

---

## Global Flags Architecture

### Verbosity System

| Flag | Level | Behavior |
|------|-------|----------|
| (none) | 0 | Compact output only |
| `-v` | 1 | + Debug messages (`eprintln!` statements) |
| `-vv` | 2 | + Command being executed |
| `-vvv` | 3 | + Raw output before filtering |

### Ultra-Compact Mode (`-u`)

Activates maximum compression for LLM contexts:
- ASCII icons instead of words
- Inline formatting (single-line summaries)
- Maximum token reduction

```rust
// Example (gh_cmd.rs)
if ultra_compact {
    println!("✓ PR #{} merged", number);
} else {
    println!("Pull request #{} successfully merged", number);
}
```

---

## Error Handling

### anyhow::Result<()> Propagation Chain

```mermaid
flowchart TD
    M["main() → Result&lt;()&gt;"] -->|"?"| G["git::run(args, verbose)\n.context('Git command failed')"]
    G -->|"?"| E["execute_git_command()\n.context('Failed to execute git')"]
    E -->|"?"| C["Command::new('git').output()\n.context('Git process error')"]
    C -->|Error| A["anyhow::Error\nbubbles up through ?"]
    A --> D["eprintln!('Error: {:#}', err)"]
    D --> X["std::process::exit(1)"]
```

### Exit Code Preservation (Critical for CI/CD)

| Exit Code | Meaning |
|-----------|---------|
| `0` | Success |
| `1` | Mycelium internal error (parsing, filtering, etc.) |
| `N` | Preserved exit code from underlying tool (e.g., git=128, lint=1) |

---

## Configuration System

### Two-Tier Configuration

```mermaid
flowchart LR
    subgraph T1["Tier 1: User Settings"]
        CFG["config.toml\n~/.config/mycelium/config.toml"]
        CFG --> CFGD["filter_level, tracking,\nretention_days, adaptive"]
    end

    subgraph T2["Tier 2: LLM Integration"]
        CL["Mycelium.md"]
        CL --> CLG["Global: ~/.claude/Mycelium.md"]
        CL --> CLL["Local: ./CLAUDE.md"]
    end

    CFG --> |"Loaded by config.rs"| MAIN["main.rs"]
    CL --> |"Created by mycelium init"| LLM["Claude Code / LLM"]
```

---

## Build Optimizations

### Release Profile (Cargo.toml)

```toml
[profile.release]
opt-level = 3          # Maximum optimization
lto = true             # Link-time optimization
codegen-units = 1      # Single codegen unit for better optimization
strip = true           # Remove debug symbols
panic = "abort"        # Smaller binary size
```

### Performance Characteristics

Binary: ~4.1 MB stripped, ~5-10ms cold start, ~2-5 MB memory.

**Runtime overhead (estimated):**

| Operation | Mycelium Overhead | Total Time |
|-----------|-------------------|------------|
| `mycelium git status` | +8ms | 58ms |
| `mycelium grep "pattern"` | +12ms | 145ms |
| `mycelium read file.rs` | +5ms | 15ms |
| `mycelium lint` | +15ms | 2.5s |

```mermaid
pie title Overhead Breakdown (typical command)
    "Clap parsing" : 3
    "Command execution" : 2
    "Filtering/compression" : 5
    "SQLite tracking" : 2
```

---

## Resources

- [FEATURES.md](FEATURES.md): Feature overview and savings summary
- [COMMANDS.md](COMMANDS.md): Complete command reference
- [ANALYTICS.md](ANALYTICS.md): Analytics, hooks, configuration
- [EXTENDING.md](EXTENDING.md): Adding new commands, patterns, ADRs
- [PLUGINS.md](PLUGINS.md): Custom filter plugins
- [COST_ANALYSIS.md](COST_ANALYSIS.md): Token economics and accuracy
- Cargo.toml: Dependencies, build profiles, package metadata
- src/: Source code organized by module
- .github/workflows/: CI/CD automation (multi-platform builds, releases)

---

## Glossary

| Term | Definition |
|------|------------|
| **Token** | Unit of text processed by LLMs (~4 characters on average) |
| **Filtering** | Reducing output size while preserving essential information |
| **Proxy Pattern** | mycelium sits between user and tool, transforming output |
| **Exit Code Preservation** | Passing through tool's exit code for CI/CD reliability |
| **Package Manager Detection** | Identifying pnpm/yarn/npm to execute JS/TS tools correctly |
| **Verbosity Levels** | `-v/-vv/-vvv` for progressively more debug output |
| **Ultra-Compact** | `-u` flag for maximum compression (ASCII icons, inline format) |
