# Ecosystem Setup

This document describes what `mycelium init --ecosystem` does and how to verify it works correctly.

## Overview

`mycelium init --ecosystem` detects which Basidiocarp tools you have installed and automatically configures them across your AI coding clients (Claude Code, Cursor, Windsurf, Cline, Continue, Claude Desktop).

It's idempotent — you can run it multiple times without problems, and it's safe to re-run after installing new tools.

## What It Does

### 1. Detects Installed Tools

Mycelium checks which tools are available in your PATH:

```
mycelium (already running this, so it's always installed)
hyphae   — persistent agent memory system
rhizome  — code intelligence (tree-sitter + LSP)
cap      — optional web dashboard for memory browser + analytics
```

Example output:
```
Basidiocarp Ecosystem Status
───────────────────────────────────────────────────────────────────────────

  mycelium      v0.5.2       ✓ installed
  hyphae        v0.3.0       ✓ installed
  rhizome       v0.4.0       ✓ installed
  cap           ─            ✗ not installed
```

### 2. Configures Claude Code (If Installed)

If the `claude` CLI is in your PATH, Mycelium registers MCP servers and installs hooks:

#### MCP Servers

- **Hyphae MCP** — Registers `hyphae serve` so Claude Code can access your agent memories, retrieve command outputs, and search the knowledge graph.
- **Rhizome MCP** — Registers `rhizome serve --expanded` so Claude Code can query code symbols, get LSP completions, and browse the codebase structure.

#### Hooks

Mycelium installs these into `~/.claude/hooks/` and registers them in `~/.claude/settings.json`:

1. **mycelium-rewrite.sh** (PreToolUse) — Rewrites commands to use `mycelium` for 60-90% token savings. Auto-detected on Bash, zsh, PowerShell.

2. **session-summary.sh** (Stop) — Captures session metrics (duration, tokens used, errors) and stores them in Hyphae when the Claude Code session ends.

3. **capture-errors.js** (PostToolUse, Bash) — Captures error messages from failed commands and stores them in Hyphae for later analysis.

4. **capture-corrections.js** (PostToolUse, Write/Edit) — Captures code corrections and file edits and stores them in Hyphae as learnings.

5. **capture-code-changes.js** (PostToolUse, Write/Edit/Bash) — Captures all code changes (new files, edits, deletions) and tracks them as memoirs in Hyphae.

#### Hyphae Database Initialization

If Hyphae is installed but the database doesn't exist, Mycelium initializes it by running:

```bash
hyphae stats
```

This creates the SQLite database at:

- **macOS**: `~/Library/Application Support/hyphae/hyphae.db`
- **Linux**: `~/.local/share/hyphae/hyphae.db`
- **Windows**: `%APPDATA%\hyphae\hyphae.db`

#### CLAUDE.md Configuration

Mycelium patches your `~/.claude/CLAUDE.md` (or creates it) with:

- A reference to `@MYCELIUM.md` (a slim 10-line file with golden rules)
- Information about token savings and Hyphae/Rhizome integration

If an old 137-line Mycelium block is present, it's replaced with the new slim reference.

### 3. Configures Other MCP Clients

If you have other MCP clients installed, Mycelium auto-detects and configures them:

| Client | Config Location | MCP Servers Registered |
|--------|-----------------|------------------------|
| **Cursor** | `~/.cursor/mcp.json` | hyphae, rhizome |
| **Windsurf** | `~/.windsurf/mcp.json` | hyphae, rhizome |
| **Cline** | `.vscode/cline_mcp_config.json` (project-local) | hyphae, rhizome |
| **Continue** | `~/.continue/config.json` | hyphae, rhizome |
| **Claude Desktop** | `~/Library/Application Support/Claude/claude_desktop_config.json` (macOS) or `~/.config/Claude/claude_desktop_config.json` (Linux) | hyphae, rhizome |

You can also configure a specific client manually:

```bash
mycelium init --ecosystem --client cursor
mycelium init --ecosystem --client windsurf
mycelium init --ecosystem --client cline
```

### 4. Reports Missing Tools

If tools are missing, Mycelium shows install commands:

```
Missing tools:
  hyphae    → cargo install --git https://github.com/basidiocarp/hyphae hyphae-cli --no-default-features
  rhizome   → cargo install --git https://github.com/basidiocarp/rhizome rhizome-cli
  cap       → git clone https://github.com/basidiocarp/cap && cd cap && npm i && npm run dev:all

Or install all: curl -sSfL https://raw.githubusercontent.com/basidiocarp/.github/main/install.sh | sh
```

## Running the Setup

### Interactive Wizard (Recommended for First-Time Setup)

```bash
mycelium init --onboard
```

This walks you through 5 steps:

1. **Detect tools** — Shows what's installed
2. **Configure Claude Code** — Asks if you want to register MCP servers and install hooks
3. **Store first memory** — Asks if you want to create a welcome memory in Hyphae
4. **Scan project** — Asks if you want to scan your current directory with Rhizome
5. **Export code graph** — Asks if you want to export code symbols to Hyphae

Each step prompts for confirmation (default is yes, just press Enter).

If stdin is not a TTY (e.g., running in CI or a non-interactive shell), `--onboard` automatically falls back to `--ecosystem`.

### Non-Interactive Setup

```bash
mycelium init --ecosystem
```

This runs all configuration steps without prompts. Good for:

- CI/CD pipelines
- Scripted setup
- Re-running after installing new tools

### Configure Specific Client

```bash
mycelium init --ecosystem --client claude-code
mycelium init --ecosystem --client cursor
mycelium init --ecosystem --client generic
```

The `--client generic` flag prints a JSON snippet you can use to manually configure any MCP client:

```json
{
  "mcpServers": {
    "hyphae": {
      "command": "hyphae",
      "args": ["serve"]
    },
    "rhizome": {
      "command": "rhizome",
      "args": ["serve", "--expanded"]
    }
  }
}
```

## Verification

### Check Configuration

To see what's installed and configured:

```bash
mycelium show config
```

Example output:

```
mycelium Configuration:

ok Hook: /Users/alice/.claude/hooks/mycelium-rewrite.sh (thin delegator, version 0.5.2)
ok MYCELIUM.md: /Users/alice/.claude/MYCELIUM.md (slim mode)
ok Integrity: hook hash verified
ok Global (~/.claude/CLAUDE.md): @MYCELIUM.md reference
ok Local (./CLAUDE.md): mycelium enabled
ok settings.json: Mycelium hook configured
```

### Run Diagnostics

```bash
mycelium doctor      # Mycelium checks
hyphae doctor        # Hyphae database + configuration
rhizome doctor       # Rhizome LSP servers + code intelligence
```

### Test the Setup

After running `mycelium init --ecosystem`, test it in Claude Code:

1. Open Claude Code
2. Try a simple command: `git status`
3. You should see filtered, condensed output (not raw git output)
4. Run: `mycelium gain` to see token savings

Example:

```bash
# In Claude Code terminal
git status
# Output:
# On branch main, working tree clean

# Raw git status would be ~150 tokens
# Filtered: ~20 tokens (87% savings)
```

### Check Token Savings

After a few commands, check accumulated savings:

```bash
mycelium gain
```

Example output:

```
Token Savings Report
────────────────────────────────────────────────────────────────

Session:     15 commands, 2,847 → 658 tokens (76.9% savings, saved 2,189 tokens)
Total:       342 commands, 98,234 → 16,542 tokens (83.2% savings, saved 81,692 tokens)

Top commands by savings:
  cargo test           87.3% savings (saved 456 tokens)
  gh pr view           79.2% savings (saved 234 tokens)
  git log              81.4% savings (saved 892 tokens)
```

### Check Capture Hooks

Once you've run a few commands in Claude Code, verify that captures are working:

```bash
hyphae recall "correction"   # Find code corrections
hyphae recall "error"        # Find captured errors
hyphae recall "session"      # Find session summaries
```

## Re-Running Setup

You can safely re-run `mycelium init --ecosystem` at any time:

- **After installing a new tool** — It will detect and configure the new tool, keeping existing configurations intact.
- **After updating tools** — It will re-register MCP servers with any new version information.
- **To fix broken configuration** — It's idempotent; it won't duplicate hooks or settings.

Example workflow:

```bash
# Initial setup
mycelium init --ecosystem

# Later, install another tool
cargo install --git https://github.com/basidiocarp/cap cap-cli

# Re-run to configure the new tool
mycelium init --ecosystem

# Now cap is configured everywhere
```

## Configuration Files Created/Modified

Here's what `mycelium init --ecosystem` touches:

| File | Action | Purpose |
|------|--------|---------|
| `~/.claude/hooks/mycelium-rewrite.sh` | Created/Updated | PreToolUse hook for command rewriting |
| `~/.claude/hooks/session-summary.sh` | Created/Updated | Stop hook for session capture |
| `~/.claude/hooks/basidiocarp/capture-*.js` | Created/Updated | PostToolUse hooks for error/correction/change capture |
| `~/.claude/MYCELIUM.md` | Created/Updated | Slim instructions file (10 lines) |
| `~/.claude/CLAUDE.md` | Patched | Adds `@MYCELIUM.md` reference, migrates old blocks |
| `~/.claude/settings.json` | Patched | Registers hooks in Claude Code settings |
| `~/.cursor/mcp.json` | Patched (if Cursor installed) | Registers hyphae, rhizome MCP servers |
| `~/.windsurf/mcp.json` | Patched (if Windsurf installed) | Registers hyphae, rhizome MCP servers |
| `~/.continue/config.json` | Patched (if Continue installed) | Registers hyphae, rhizome MCP servers |
| `~/.config/Claude/claude_desktop_config.json` | Patched (if Claude Desktop installed) | Registers hyphae, rhizome MCP servers |
| `~/Library/Application Support/hyphae/hyphae.db` | Created (if missing) | Hyphae SQLite database |
| `~/.local/share/hyphae/hyphae.db` | Created (if missing) | Hyphae SQLite database (Linux) |

All changes preserve existing configuration. Nothing is overwritten unless it's outdated Mycelium config.

## Troubleshooting

### Claude Code not found

```
! claude not found in PATH — skipping Claude Code configuration.
  Install Claude Code first, then re-run: mycelium init --ecosystem
```

**Solution**: Install Claude Code, then re-run the command.

### MCP registration failed

```
! Failed to register hyphae MCP
```

**Solution**:

1. Verify `claude` CLI is installed: `claude --version`
2. Verify Hyphae is installed: `hyphae --version`
3. Try manually: `claude mcp add --scope user hyphae -- hyphae serve`
4. Check `~/.claude/settings.json` for syntax errors

### Hooks not executing in Claude Code

**Solution**:

1. Check hook is executable: `ls -l ~/.claude/hooks/mycelium-rewrite.sh`
2. Check settings.json is valid JSON: `jq . ~/.claude/settings.json`
3. Verify hook path in settings.json matches actual file location
4. Restart Claude Code

### Hyphae database not initializing

```
! Hyphae database failed to initialize
```

**Solution**:

```bash
# Manually initialize
hyphae stats

# Or set data dir explicitly
export XDG_DATA_HOME=~/.local/share  # Linux
# or
export HYPHAE_DATA_DIR=~/Library/Application\ Support/hyphae  # macOS
hyphae stats
```

### Captures not working

If you're not seeing captured errors, corrections, or code changes in Hyphae:

1. Verify Hyphae is installed: `hyphae --version`
2. Check database exists: `ls -la ~/Library/Application\ Support/hyphae/hyphae.db` (macOS)
3. Try manually storing: `hyphae store --topic test --content "test" `
4. Check hook output: Look for errors in Claude Code's output/error panel

## What Happens Next

Once setup is complete:

- **Mycelium** intercepts every command you run in Claude Code and filters output for 60-90% token savings
- **Hyphae** stores errors, corrections, code changes, and session summaries for later recall
- **Rhizome** provides code intelligence (symbol lookup, LSP completions, codebase structure)
- **Token usage** drops significantly on common dev operations

Example session improvements:

| Operation | Before | After | Savings |
|-----------|--------|-------|---------|
| `git log` (20 commits) | 487 tokens | 92 tokens | 81% |
| `cargo test` (10 failures) | 654 tokens | 78 tokens | 88% |
| `gh pr view` | 234 tokens | 30 tokens | 87% |
| `pnpm list` | 456 tokens | 137 tokens | 70% |

Over a typical development session, this adds up to 75-85% fewer tokens used overall.
