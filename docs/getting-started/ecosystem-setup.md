# Ecosystem Setup

`stipe init` owns ecosystem onboarding, shared repair, and MCP registration.

This document covers the remaining Mycelium-specific setup surfaces:
- Claude Code docs-only setup
- configuration inspection
- uninstall and cleanup

## Overview

Use `stipe init` for first-time setup and shared ecosystem repair:

```bash
stipe init
```

Use Mycelium directly only for Mycelium-owned guidance files, hook adapter repair, and configuration inspection or cleanup.

## Mycelium-Owned Setup Modes

### Global Claude Code Guidance

Install global docs-only guidance without shell hook setup:

```bash
mycelium init -g --claude-md
```

This writes or updates Mycelium guidance in the user Claude Code directory. Use it when you want global instructions but do not want to run full ecosystem setup from Mycelium.

### Claude Code Hook Adapter Repair

On supported platforms, reinstall or repair the Mycelium Claude Code hook adapter with:

```bash
mycelium init -g
```

This refreshes the thin delegator hook, `MYCELIUM.md`, and the Claude settings patch that Mycelium still owns.

### Project-Local Claude Code Guidance

Write project-local guidance only:

```bash
mycelium init --claude-md
```

Use this when you want a `CLAUDE.md` in the current repository without changing global setup.

## Verification

### Check Current Configuration

Inspect the current Mycelium-managed state:

```bash
mycelium config
```

Example output:

```
mycelium Configuration:

ok MYCELIUM.md: /Users/alice/.claude/MYCELIUM.md
ok Global (~/.claude/CLAUDE.md): @MYCELIUM.md reference
ok Local (./CLAUDE.md): mycelium enabled
```

### Check Token Savings

```bash
mycelium gain
```

### Confirm CLAUDE.md Output

After running one of the docs-only init modes, verify the expected file exists:

```bash
ls ~/.claude/MYCELIUM.md
ls ./CLAUDE.md
```

Use the global path for `mycelium init -g --claude-md` and the local path for `mycelium init --claude-md`.

## Re-Running Setup

If setup, onboarding, shared repair, or MCP registration is the problem, rerun:

```bash
stipe init
```

You can re-run the Mycelium docs-only commands whenever you want to refresh guidance files:

```bash
mycelium init -g --claude-md
mycelium init --claude-md
```

## Uninstall

Remove Mycelium-managed setup:

```bash
mycelium init -g --uninstall
```

Use `mycelium config` before and after uninstall if you want to confirm what changed.

## Troubleshooting

### I need onboarding or repair

Use `stipe init` for ecosystem onboarding, shared repair, and MCP registration. Use `mycelium init -g` if the Claude hook adapter itself needs repair.

### My Claude Code guidance files look stale

Refresh them directly:

```bash
mycelium init -g --claude-md
mycelium init --claude-md
```

### I need to inspect what Mycelium changed

Run:

```bash
mycelium config
```

## After Setup

With setup complete, Mycelium filters and compresses command output, routes large output to Hyphae when available, and uses Rhizome structural reads for large code files when available.

Example session improvements:

| Operation | Before | After | Savings |
|-----------|--------|-------|---------|
| `git log` (20 commits) | 487 tokens | 92 tokens | 81% |
| `cargo test` (10 failures) | 654 tokens | 78 tokens | 88% |
| `gh pr view` | 234 tokens | 30 tokens | 87% |
| `pnpm list` | 456 tokens | 137 tokens | 70% |

Over a typical development session, this adds up to 75-85% fewer tokens used overall.
