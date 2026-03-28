# Mycelium Internals

This document explains how Mycelium works internally: command dispatch, filtering, token tracking, and ecosystem integration.

## Table of Contents

1. [Entry Point and Command Dispatch](#entry-point-and-command-dispatch)
2. [Command Rewriting](#command-rewriting)
3. [Filter System](#filter-system)
4. [Output Routing](#output-routing)
5. [Test Runner Fixes](#test-runner-fixes)
6. [Token Tracking](#token-tracking)
7. [Ecosystem Integration](#ecosystem-integration)

---

## Entry Point and Command Dispatch

### Main Entry Point: `src/main.rs`

Mycelium starts at the `main()` function which:

1. **Parses CLI arguments** using Clap
2. **Handles parse failures** with fallback to raw command execution (for non-meta commands)
3. **Checks for hook warnings** (1/day, non-blocking)
4. **Dispatches to command handler**

```
main.rs: Parse CLI args with Clap
    ↓
Is it a Display (--help/--version) error? → exit
    ↓
Is it a parse error? → run_fallback()
    ↓
hook_check::maybe_warn() → optionally warn if installed hook is outdated
    ↓
dispatch::dispatch(cli) → route to command handler
```

### Fallback Execution: `src/dispatch.rs::run_fallback()`

When Clap fails to parse, fallback handles non-meta commands:

1. **No args**: Show Clap's error
2. **Meta command** (gain, discover, init, learn, config, proxy, hook-audit, economics / cc-economics): Always show Clap error (don't fall back)
3. **Plugin check**: Look for user plugin before raw passthrough
4. **Raw execution**: Run the command unchanged, with timing and stats tracking

```
run_fallback(parse_error)
    ├─ Is it a meta-command? → show error, exit
    ├─ Is it a plugin? → run plugin
    ├─ Otherwise → execute raw with tracking
```

### Command Dispatch: `src/dispatch.rs::dispatch()`

Routes parsed CLI commands to handlers:

```rust
dispatch(cli: Cli) → Result<()>
    │
    ├─ Cli::Git { .. }           → git::run()
    ├─ Cli::Cargo { .. }         → cargo_cmd::run()
    ├─ Cli::Test { .. }          → runner_cmd::test()
    ├─ Cli::Gain { .. }          → gain::run()
    ├─ Cli::Discover { .. }      → discover::run()
    ├─ Cli::Init { .. }          → init::run()
    ├─ Cli::Learn { .. }         → learn::run()
    ├─ Cli::Diff { .. }          → diff_cmd::run()
    ├─ Cli::Find { .. }          → find_cmd::run()
    ├─ Cli::Grep { .. }          → grep_cmd::run()
    ├─ Cli::Read { .. }          → read_cmd::run()
    ├─ Cli::Ls { .. }            → ls_cmd::run()
    ├─ Cli::Log { .. }           → log_cmd::run()
    ├─ Cli::Wc { .. }            → wc_cmd::run()
    ├─ Cli::Json { .. }          → json_cmd::run()
    ├─ Cli::Summary { .. }       → summary_cmd::run()
    ├─ Cli::Gh { .. }            → gh_cmd::run()
    ├─ Cli::Curl { .. }          → curl_cmd::run()
    ├─ Cli::Wget { .. }          → wget_cmd::run()
    ├─ Cli::Docker { .. }        → container_cmd::run()
    ├─ Cli::Env { .. }           → env_cmd::run()
    └─ ... and more
```

### Flow Diagram: `mycelium git status`

```
mycelium git status
    ↓
main(): Cli::try_parse() → Commands::Git { command: GitCommands::Status, ... }
    ↓
hook_check::maybe_warn() (1/day)
    ↓
dispatch::dispatch(cli)
    ↓
git::run(GitCommand::Status, args, None, 0, [])
    ↓
status::run_status(args, verbose, global_args)
    ├─ Run: git <global_args> status <args>
    ├─ Capture: stdout + stderr
    └─ Filter & Output
```

---

## Command Rewriting

The **hook** intercepts shell commands and rewrites them to Mycelium equivalents before execution. This is the mechanism that makes `git status` automatically become `mycelium git status`.

### Registry: `src/discover/registry.rs`

The registry classifies commands and rewrites them:

1. **Classification**: Check if a command has a Mycelium equivalent
2. **Rewriting**: Transform the raw command to its Mycelium equivalent
3. **Metadata**: Track category, estimated token savings, status

### How It Works

```
Input: "git log -10"
    ↓
classify_command("git log -10")
    ├─ Strip env prefix (sudo, env VAR=val, etc.)
    ├─ Use RegexSet for fast classification
    ├─ Find matching rule
    └─ Return Classification::Supported { mycelium_equivalent: "mycelium git", ... }
    ↓
rewrite_command("git log -10", excluded=[])
    ├─ Check for heredoc or arithmetic expansion (skip if found)
    ├─ Split compound commands (&&, ||, ;, |)
    ├─ Rewrite each segment with rewrite_segment()
    └─ Return Some("mycelium git log -10")
    ↓
Output: "mycelium git log -10"
```

### Command Chain Handling

```
Input: "git add . && cargo test"
    ↓
split_command_chain() → ["git add .", "cargo test"]
    ↓
rewrite_compound()
    ├─ Segment 1: "git add ." → rewrite_segment() → "mycelium git add ."
    ├─ && operator
    ├─ Segment 2: "cargo test" → rewrite_segment() → "mycelium cargo test"
    └─ Join with &&
    ↓
Output: "mycelium git add . && mycelium cargo test"
```

### Special Cases

1. **Pipes** (`|`): Only rewrite the first command. The downstream pipeline remains unchanged.
   ```
   Input: "git log -10 | grep commit"
   Output: "mycelium git log -10 | grep commit"
   ```

2. **head -N file**: Rewritten to `mycelium read file --max-lines N`
   ```
   rewrite_head_numeric() intercepts before generic logic
   ```

3. **gh with --json/--jq/--template**: Skip rewrite (output is JSON, not text to filter)
   ```
   Input: "gh pr view 123 --json title,body"
   Output: None (no rewrite)
   ```

4. **MYCELIUM_DISABLED=1**: Skip rewrite if environment variable is set
   ```
   MYCELIUM_DISABLED=1 git status  →  No rewrite
   ```

5. **Excluded commands** (#243): If a command is in the `excluded` list, don't rewrite
   ```
   Input: "git status" with excluded=["git"]
   Output: None (no rewrite)
   ```

---

## Filter System

Each command family (git, cargo, npm, etc.) has a **filter module** that compresses the raw command output.

### Filter Architecture

```
src/
├── vcs/
│   ├── git/          # git filters
│   │   ├── diff.rs   # git diff/show filtering
│   │   ├── log.rs    # git log filtering
│   │   └── ...
│   ├── gh_cmd/       # gh pr, gh issue filtering
│   └── ...
├── cargo_filters/    # cargo build, test, clippy
├── js.rs            # npm, pnpm, vitest, playwright
├── docker.rs        # docker ps, images, logs
└── ... other filters
```

### Filter Pattern: Git Log Example

```rust
// src/vcs/git/log.rs
pub fn run_log(args: &[String], max_lines: Option<usize>, verbose: u8, global_args: &[String]) -> Result<()> {
    let timer = tracking::TimedExecution::start();

    // Run: git log <args>
    let raw = capture_output(git_cmd(global_args), args)?;

    // Apply filter
    let filtered = filter_git_log(&raw);

    // Apply max_lines limit if specified
    let output = apply_max_lines(&filtered, max_lines);

    // Print (not println!; use stdout::write to avoid extra newlines)
    std::io::Write::write_all(&mut std::io::stdout(), output.as_bytes())?;

    // Track in database
    timer.track_filtered(
        "git log",
        &raw.len(),
        &output.len(),
        category,
    );

    Ok(())
}

fn filter_git_log(raw: &str) -> String {
    // Compact: "commit abc123 Author: John Doe, Date: ..., Subject"
    // Apply token compression regex patterns
    //
    // Regex patterns:
    // - Remove "commit " prefix, keep only hash
    // - Collapse "Author: X Author-Date: Y Commit: Z Date: W" to single line
    // - Remove blank lines
    //
    // Example:
    // Raw (400 tokens):
    //   commit abc123def456
    //   Author: John Doe <john@example.com>
    //   AuthorDate: Mon Mar 19 10:00:00 2025 +0000
    //   Commit: Jane Smith <jane@example.com>
    //   CommitDate: Mon Mar 19 10:10:00 2025 +0000
    //
    //   This is a commit message
    //   with multiple lines
    //
    // Filtered (80 tokens, 80% savings):
    //   abc123 John Doe | This is a commit message
}
```

### Adaptive Output Routing: `src/hyphae.rs::decide_action()`

Before filtering, check output size:

```
decide_action(output: &str) → OutputAction
    │
    ├─ Passthrough:  small output (<50 lines) → return unchanged
    ├─ Filter:       medium output (50-500 lines) → apply local filter
    └─ Chunk:        large output (>500 lines) + Hyphae available → store in Hyphae
```

Thresholds defined in `src/adaptive.rs::classify()`:
- **Passthrough**: < 50 lines or < 2KB
- **Filter**: 50-500 lines or 2KB-large
- **Chunk**: > 500 lines and > large threshold

---

## Output Routing

### The Three Paths: `src/hyphae.rs::route_or_filter()`

```
Command output
    │
    ├─ Small (< 50 lines)?
    │   └─ Passthrough → return unchanged
    │
    ├─ Hyphae available AND Large (> 500 lines)?
    │   ├─ Call hyphae_client::store_output()
    │   ├─ Get summary + document_id
    │   └─ Return: "[mycelium→hyphae] cargo test: 247 passed, 3 failed. Use hyphae_get_command_chunks(document_id="xyz") for details."
    │
    └─ Medium (50-500 lines) OR Hyphae unavailable?
        └─ Apply filter_fn() → return filtered output
```

### Hyphae Integration: `src/hyphae_client.rs`

Persistent MCP connection to Hyphae for chunked storage:

```rust
pub fn store_output(command: &str, output: &str, project: Option<&str>) -> Result<ChunkSummary>

// Uses static Mutex<Option<HyphaeConnection>> for persistent subprocess
//   ├─ First call: spawn `hyphae serve` subprocess
//   ├─ Subsequent calls: reuse same subprocess (avoid cold starts)
//   └─ On crash: respawn on next call
//
// Communicates via JSON-RPC over stdin/stdout (line-delimited)
// Calls: hyphae_store_command_output(command, output, project)
// Returns: ChunkSummary { summary: String, document_id: String, chunk_count: usize }
```

**Why persistent connection?**
- `hyphae serve` startup takes ~100ms
- Mycelium targets <10ms execution
- Persistent subprocess amortizes startup cost

---

## Test Runner Fixes

### Vitest and Playwright Passthrough Truncation

Vitest and Playwright test runners often emit large, structured output. Mycelium v0.4.3 increased passthrough truncation limits for these tools:

**Change:** Test runner passthrough output increased from 500 chars to 4000 chars.

**Rationale:**
- Allows more meaningful test failure context without destructive filtering
- Reduces token savings (shorter truncation) but improves debugging
- Large outputs still route to Hyphae when available, falling back to 4000-char summary

**Affected commands:**
- `mycelium vitest run`
- `mycelium playwright test`
- Any tool using test runner passthrough

**Configuration** (`~/.config/mycelium/config.toml`):
```toml
[filters.test]
passthrough_truncate_chars = 4000  # Increased from 500
```

### Stderr Fallback Parsing

When test runners output failures only on stderr (not stdout), Mycelium now parses stderr as fallback:

**Flow:**
1. Try stdout parsing first
2. If stdout is empty/non-matching, parse stderr
3. If both empty, return raw command output (passthrough)

**Affected runners:**
- Some Jest/Vitest configurations
- Playwright headless mode
- Custom test harnesses

This ensures test failures aren't missed due to output redirection quirks.

---

## Token Tracking

### Database: `src/tracking.rs` and `src/gain/`

Mycelium stores token savings in SQLite at the resolved tracking database path (`history.db` in the platform data directory by default). Use `mycelium gain --status` to inspect the exact path on the current machine.

```sql
CREATE TABLE commands (
    id INTEGER PRIMARY KEY,
    timestamp TEXT,              -- ISO8601 timestamp
    original_cmd TEXT,           -- raw command run (git status, cargo test, etc.)
    mycelium_cmd TEXT,           -- executed Mycelium form
    project_path TEXT,           -- canonical current working directory
    input_tokens INTEGER,        -- estimated raw output tokens
    output_tokens INTEGER,       -- filtered output tokens
    saved_tokens INTEGER,        -- input_tokens - output_tokens
    savings_pct REAL,            -- saved_tokens / input_tokens
    exec_time_ms INTEGER         -- wall-clock time including filter
);
```

### Tracking Flow: `src/tracking/timer.rs::TimedExecution`

```rust
let timer = tracking::TimedExecution::start();

// ... run command and filter ...

timer.track(
    "git log",                  // original command
    "mycelium git log",         // executed Mycelium command
    raw_output,                 // raw text
    filtered_output,            // filtered text
);

// Inserts row to SQLite with:
// - Current timestamp
// - Canonical project path (cwd)
// - Estimated raw/filtered token counts
// - Saved tokens and savings percentage
// - Actual runtime
```

### Display: `src/gain/display.rs`

`mycelium gain` command shows analytics:

```bash
mycelium gain                    # Summary dashboard
mycelium gain --history          # Last 20 commands with savings
mycelium gain --daily            # Hourly breakdown for today
mycelium gain --weekly           # Last 7 days breakdown
mycelium gain --monthly          # Last 30 days breakdown
mycelium gain --project          # Current project only
mycelium gain --projects         # Per-project breakdown
mycelium gain --graph            # ASCII bar chart
mycelium gain --quota            # Token quota usage (for Claude Code tier)
mycelium gain --failures         # Commands that failed filtering
mycelium gain --compare <cmd>    # Compare two commands' savings
mycelium gain --json             # JSON export
mycelium gain --csv              # CSV export
```

`mycelium economics` merges that tracking data with `ccusage`. When `--project` or `--project-path` is set, the Mycelium savings side is project-scoped while `ccusage` spend remains global.

Example output:
```
────────────────────────────────────────────────────────────────
Token Savings Summary — claude-mycelium
────────────────────────────────────────────────────────────────

Total tokens saved:          847,294  (65% reduction)
Commands run:                   2,347
Average savings per command:      361 tokens (62.3%)

Category breakdown:
  Cargo          789,234 tokens saved (45% of total)
  Git            187,923 tokens saved (21% of total)
  GitHub          78,234 tokens saved (9% of total)
  ...

Last 7 days:
  Today:         12,456 tokens
  Yesterday:      9,876 tokens
  ...
```

---

## Ecosystem Integration

### Initialization: `src/init/ecosystem.rs::run_ecosystem()`

`mycelium init --ecosystem` discovers sibling tools and configures them:

```
1. Detect installed tools
   ├─ mycelium (always installed — we're running it)
   ├─ hyphae (check spore::discover(Tool::Hyphae))
   ├─ rhizome (check spore::discover(Tool::Rhizome))
   └─ cap (run `cap --version`)

2. Print ecosystem status
   ├─ List each tool with version
   └─ Show availability status

3. Configure Claude Code
   ├─ Check if `claude` CLI is available
   ├─ For each tool, register MCP server:
   │   └─ `claude mcp add --scope user <name> -- <binary> <args>`
   ├─ Examples:
   │   - hyphae → `hyphae serve`
   │   - rhizome → `rhizome serve`
   └─ Show which MCPs were registered

4. Initialize tool databases
   ├─ hyphae: create SQLite database if missing
   ├─ rhizome: initialize code index if missing
   └─ mycelium: create tracking database if missing

5. Install capture hooks
   ├─ session-summary hook (Stop hook)
   ├─ capture-errors hook (PostToolUse)
   ├─ capture-corrections hook (PostToolUse)
   ├─ capture-code-changes hook (PostToolUse)
   └─ hooks stored in ~/.claude/hooks/basidiocarp/
```

### Hook Installation: `src/init/hook.rs`

Installs the shell hook that intercepts commands:

```bash
# Detect shell (zsh, bash, etc.)
# Inject hook initialization in ~/.zshrc, ~/.bashrc, etc.
#
# Hook code (in hooks/mycelium-hook.sh):
#   export MYCELIUM_HOOK_ENABLED=1
#   export HOOK_LAST_TIMESTAMP=$(date +%s)
#
#   mycelium_rewrite() {
#       local cmd="$1"
#       local result=$(mycelium rewrite "$cmd" 2>/dev/null)
#       [ $? -eq 0 ] && echo "$result" || echo "$cmd"
#   }
#
#   # Intercept commands before execution
#   eval "$(mycelium hook --install-code)"
```

The hook:
1. Intercepts every shell command
2. Calls `mycelium rewrite <cmd>` to check if rewrite is needed
3. If yes, executes the rewritten command
4. If no, executes the original command

---

## Data Flow Diagrams

### Complete Flow: `mycelium git log`

```
User: mycelium git log -10
    ↓
main.rs: Cli::try_parse()
    ↓ Succeeds
dispatch::dispatch(Cli { command: Commands::Git { .. } })
    ↓
git::run(GitCommand::Log, ["-10"], None, 0, [])
    ↓
git::log::run_log(["-10"], None, 0, [])
    ├─ timer.start()
    ├─ execute: git log -10 (capture stdout/stderr)
    ├─ raw_output = "commit abc123\nAuthor: John\nDate: ...\n..." (4000 tokens)
    ├─ decide_action(raw_output)
    │   └─ > 50 lines? Filter
    ├─ hyphae::route_or_filter(
    │       command="git log",
    │       raw=raw_output,
    │       filter_fn=|raw| filter_git_log(raw)
    │   )
    │   ├─ OutputAction::Filter
    │   ├─ filter_git_log(raw_output)
    │   │   └─ Apply regex to compress
    │   │   └─ filtered_output = "abc123 John | ...\ndef456 Jane | ...\n" (800 tokens)
    │   └─ Return filtered_output
    ├─ println!("{}", filtered_output)
    ├─ timer.track(...)
    │   └─ Insert to history.db in the resolved data directory
    └─ Ok(())
    ↓
Output: compact log to stdout
```

### Hook Rewriting Flow

```
User: git log
    ↓
Shell hook:
    ├─ Hook triggered before execution
    ├─ Call: mycelium rewrite "git log"
    │   └─ src/dispatch.rs::rewrite_cmd()
    │   └─ Call: discover::registry::rewrite_command("git log", [])
    │       ├─ classify_command("git log")
    │       └─ Classification::Supported { mycelium_equivalent: "mycelium git", ... }
    │       ├─ rewrite_segment("git log")
    │       ├─ Match "git" prefix
    │       └─ Return Some("mycelium git log")
    │   └─ Output: "mycelium git log"
    ├─ Hook executes rewritten command
    └─ mycelium git log runs (flow above)
    ↓
Output: compact log from mycelium
```

### Hyphae Chunking Flow (Large Output)

```
User: mycelium cargo test
    ↓
... test execution ...
    ↓
raw_output = "test result: ok. 247 passed..." (2000 lines, 50KB)
    ↓
decide_action(raw_output) → OutputAction::Chunk (Hyphae available)
    ↓
hyphae::route_or_filter(..., filter_fn)
    └─ OutputAction::Chunk
    └─ hyphae_client::store_output("cargo test", raw_output, project="mycelium")
        ├─ get_or_connect() → HYPHAE_PROCESS (persistent subprocess)
        │   ├─ First call: spawn `hyphae serve`
        │   └─ Subsequent: reuse
        ├─ build_request(...) → JSON-RPC: hyphae_store_command_output(...)
        ├─ conn.call(request) → send to hyphae stdin
        ├─ Parse response: ChunkSummary { summary: "247 passed, 0 failed", document_id: "xyz123", chunk_count: 8 }
        └─ Return ChunkSummary
    └─ format_chunk_summary(...) → "[mycelium→hyphae] cargo test: 247 passed, 0 failed. Use hyphae_get_command_chunks(document_id=\"xyz123\") for details."
    ↓
Output: summary to stdout
    ↓
Agent can later call: hyphae_get_command_chunks(document_id="xyz123")
    └─ Retrieves full output in chunks from Hyphae
```

---

## Key Design Decisions

### 1. Persistent Hyphae Connection

- **Why**: Avoid subprocess startup overhead per call (100ms × many calls = slow)
- **How**: Static `Mutex<Option<HyphaeConnection>>` with respawn on crash
- **Cost**: Single `hyphae serve` process running in background

### 2. Adaptive Output Routing

- **Why**: Don't destroy important output with filters
- **How**: Classify output size → passthrough small, filter medium, chunk large
- **Result**: 60-90% token savings on most commands; full output available for large commands via Hyphae

### 3. Registry-Based Rewriting

- **Why**: Centralized, testable command transformation logic
- **How**: Regex patterns + rules table in `discover/registry.rs`
- **Benefit**: Easy to add new commands, easy to test

### 4. Hook-Based Interception

- **Why**: User doesn't need to remember `mycelium` prefix
- **How**: Shell hook rewrites commands before execution
- **Result**: `git log` becomes `mycelium git log` transparently

### 5. SQLite Tracking Database

- **Why**: Persistent token savings analytics across sessions
- **How**: Insert row per filtered command (timestamp, project, category, tokens)
- **Benefit**: `mycelium gain` shows savings over time, per project, per category

---

## Development Notes

### Adding a New Filter

1. Create filter module: `src/filters/newcmd.rs`
2. Implement: `pub fn run_newcmd(args: &[String]) -> Result<()>`
3. Add to dispatch: `src/dispatch.rs::dispatch()`
4. Add to CLI: `src/commands.rs`
5. Add tests:
   - Unit tests in `src/filters/newcmd.rs`
   - Fixtures in `tests/fixtures/`
   - Snapshot tests with `insta`
   - Token accuracy tests (verify ≥60% savings)
6. Add to registry: `src/discover/registry.rs` (if hook rewriting needed)

### Testing Snapshot Changes

```bash
# After changing filter logic:
cargo test                    # Tests fail with snapshot mismatch
cargo insta review           # Interactive review of changes
cargo insta accept           # Accept if changes are correct
```

### Debugging

```bash
mycelium --verbose git log    # Verbose output (vvv = more verbose)
mycelium proxy git log        # Run without filtering (raw output)
mycelium discover             # Analyze Claude Code sessions
mycelium hook-audit           # Check hook installation status
```

---

## File Organization

```
src/
├── main.rs                    # Entry point
├── commands.rs                # Clap CLI definitions
├── dispatch.rs                # Command routing
├── discover/
│   ├── mod.rs
│   └── registry.rs            # Command classification & rewriting
├── hyphae.rs                  # Output routing logic
├── hyphae_client.rs           # Persistent Hyphae MCP connection
├── gain/                      # Token analytics
│   ├── mod.rs
│   ├── display.rs             # Dashboard views
│   ├── export.rs              # JSON/CSV export
│   ├── compare.rs             # Command comparison
│   └── helpers.rs
├── tracking.rs                # SQLite database operations
├── init/                      # Setup & installation
│   ├── mod.rs
│   ├── ecosystem.rs           # Tool discovery & MCP registration
│   ├── hook.rs                # Hook installation
│   ├── claude_md.rs           # CLAUDE.md injection
│   └── ...
├── filter.rs                  # Generic filter infrastructure
├── vcs/                       # Version control filters
│   ├── git/                   # Git command filters
│   │   ├── mod.rs             # Command dispatch
│   │   ├── log.rs             # git log filtering
│   │   ├── diff.rs            # git diff/show filtering
│   │   ├── mutations.rs        # git add/commit/push
│   │   ├── status.rs          # git status/branch/worktree
│   │   └── stash.rs           # git stash filtering
│   ├── gh_cmd/                # GitHub CLI filters
│   └── gt_cmd/                # Graphite CLI filters
├── cargo_filters/             # Cargo command filters
├── js.rs                      # JavaScript/Node.js filters
├── docker.rs                  # Docker filters
├── ... other filters ...
├── adaptive.rs                # Output size classification
├── config.rs                  # Configuration loading
└── utils.rs                   # Shared utilities
```

---

## Performance Characteristics

| Metric | Target | Mechanism |
|--------|--------|-----------|
| Startup time | <10ms | Single binary, zero runtime dependencies |
| Memory usage | <5MB | Minimal allocations, streaming where possible |
| Binary size | <5MB | Release profile: LTO, single codegen, stripped |
| Filter overhead | <2ms | Regex compilation cached in `OnceLock` |
| Hyphae overhead | ~100ms | Persistent subprocess (amortized) |
| Database insert | <5ms | SQLite write-ahead logging |

---

## Integration Points

### With Hyphae

- **Detection**: `spore::discover(Tool::Hyphae)`
- **Communication**: JSON-RPC over subprocess stdin/stdout
- **Fallback**: If unavailable, use local filtering
- **Configuration**: `~/.config/mycelium/config.toml` (optional override)

### With Rhizome

- Not yet integrated into Mycelium core, but planned for code reading filters

### With Claude Code

- **Hook**: Shell hook rewrites commands before execution
- **MCP servers**: Registered via `claude mcp add` (in `init --ecosystem`)
- **Data**: Hyphae chunks accessible to agents via `hyphae_get_command_chunks()`

### With Cap

- **Data source**: Reads the Mycelium tracking database (`history.db`)
- **Analytics**: Visualizes token savings over time

---

## Thread Safety

Mycelium is single-threaded for command execution. Thread safety appears in:

1. **`HYPHAE_PROCESS: Mutex<Option<HyphaeConnection>>`**: Persistent connection guarded by mutex
2. **Regex compilation: `OnceLock`**: Compiled once, shared read-only thereafter
3. **SQLite**: Single writer, multiple readers via WAL mode

---

## Error Handling Strategy

- **Library code** (`src/filters/`, `src/vcs/`): Use `anyhow::Result` with `.context()`
- **App code** (`src/dispatch.rs`, `src/main.rs`): Handle errors explicitly or propagate with context
- **Hyphae failures**: Non-fatal; fall back to local filtering
- **Hook failures**: Transparent; re-execute with raw command
- **Database errors**: Non-fatal; graceful degradation (skip tracking)
