# Updating Mycelium and Ecosystem Tools

This document covers how to update Mycelium, Hyphae, Rhizome, and Cap, and what to do after updating.

## Quick Update

For all ecosystem tools in one command:

```bash
curl -sSfL https://raw.githubusercontent.com/basidiocarp/.github/main/update.sh | sh
```

This script updates:
- Mycelium
- Hyphae
- Rhizome
- Cap (if installed)

## Individual Tool Updates

### Mycelium

Check current version:

```bash
mycelium --version
# Output: mycelium 0.5.2
```

Update via Cargo:

```bash
cargo install --locked mycelium-cli
# or from source
cargo install --locked --git https://github.com/basidiocarp/mycelium mycelium-cli
```

Update via Homebrew (if you use it):

```bash
brew upgrade mycelium
```

### Hyphae

Check current version:

```bash
hyphae --version
# Output: hyphae 0.3.0
```

Update via Cargo:

```bash
cargo install --locked --git https://github.com/basidiocarp/hyphae hyphae-cli --no-default-features
```

### Rhizome

Check current version:

```bash
rhizome --version
# Output: rhizome 0.4.0
```

Update via Cargo:

```bash
cargo install --locked --git https://github.com/basidiocarp/rhizome rhizome-cli
```

### Cap

Check current version:

```bash
cap --version
# Output: cap 1.2.0
```

Update via Git + npm:

```bash
cd ~/path/to/cap  # wherever you cloned it
git pull origin main
npm install
npm run build
npm run dev:all   # or use your package manager
```

Or clone fresh:

```bash
git clone https://github.com/basidiocarp/cap cap-latest
cd cap-latest
npm install
npm run dev:all
```

## After Updating

After updating any ecosystem tool, re-run:

```bash
stipe init
```

This will:

1. Detect the new versions
2. Repair onboarding state if needed
3. Re-register MCP servers if needed
4. Repair shared ecosystem integration managed by Stipe

If you want to refresh Mycelium-owned Claude surfaces specifically, use the retained Mycelium init modes:

```bash
mycelium init -g
mycelium init -g --claude-md
mycelium init --claude-md
```

Use `mycelium init -g` to repair the hook adapter on supported platforms, the docs-only global form for user-wide guidance, and the local form for a project `CLAUDE.md`.

Example output:

```
Basidiocarp Ecosystem Status
───────────────────────────────────────────────────────────────────────────

  mycelium      v0.5.3       ✓ installed (new)
  hyphae        v0.3.1       ✓ installed (new)
  rhizome       v0.4.1       ✓ installed (new)
  cap           v1.2.1       ✓ installed (new)

Repairing ecosystem setup...

  ✓ Configured:
    - hyphae MCP
    - rhizome MCP
    - shared setup and client wiring
```

## Restarting Claude Code

After updating Mycelium or refreshing Mycelium-managed Claude Code setup, restart Claude Code to pick up changes:

1. Close Claude Code entirely
2. Wait 2 seconds
3. Re-open Claude Code

Or use the Claude Code command palette:

```
Cmd+Shift+P  (macOS) or Ctrl+Shift+P (Windows/Linux)
Developer: Reload Window
```

## Version Compatibility Matrix

Mycelium is compatible with these Hyphae and Rhizome versions:

| Mycelium | Min Hyphae | Min Rhizome | Notes |
|----------|-----------|-----------|-------|
| 0.5.x | 0.3.0 | 0.4.0 | Current stable |
| 0.4.x | 0.2.0 | 0.3.0 | Earlier version |
| 0.3.x | 0.1.0 | 0.2.0 | Deprecated |

In practice, you don't need to match exact versions. Just keep all tools within the last 2-3 releases.

To check compatibility:

```bash
mycelium --version && hyphae --version && rhizome --version
```

If there's a big version gap (e.g., Mycelium 0.5.x with Hyphae 0.1.x), update the older tool.

## Troubleshooting Updates

### "command not found" after update

The binary path might have changed. Check where it was installed:

```bash
which mycelium
which hyphae
which rhizome
which cap
```

If `which` returns nothing, the binary isn't in your PATH.

**Solution**: Reinstall with proper PATH setup.

```bash
# Remove old installation
cargo uninstall mycelium-cli

# Reinstall with explicit path
cargo install --locked --git https://github.com/basidiocarp/mycelium mycelium-cli --root ~/.local

# Verify it's in PATH
which mycelium
```

### Update script fails

If the bulk update script fails:

```bash
curl -sSfL https://raw.githubusercontent.com/basidiocarp/.github/main/update.sh | sh
# Error: ...
```

**Solution**: Update manually, one tool at a time:

```bash
cargo install --locked mycelium-cli
cargo install --locked --git https://github.com/basidiocarp/hyphae hyphae-cli --no-default-features
cargo install --locked --git https://github.com/basidiocarp/rhizome rhizome-cli
```

### Hooks broken after update

If Claude Code stops filtering commands after an update on macOS/Linux:

1. Check that hooks still exist:

```bash
ls -la ~/.claude/hooks/mycelium-rewrite.sh
```

2. Re-run `mycelium init -g` to repair the Claude hook adapter:

```bash
mycelium init -g
```

3. Check hook is executable:

```bash
chmod +x ~/.claude/hooks/mycelium-rewrite.sh
```

4. Verify settings.json hook path is correct:

```bash
jq .hooks.PreToolUse ~/.claude/settings.json | grep mycelium-rewrite
```

5. Restart Claude Code

If you're on Windows or another non-Unix environment, skip the `chmod` and hook-file checks above. Use the docs-only fallback instead:

```bash
mycelium init -g --claude-md
# or, for project-local instructions only
mycelium init --claude-md
```

### Database schema changes

If Hyphae updates with schema changes, the database might need migration:

```bash
hyphae migrate   # If this command exists
# or
hyphae stats     # Usually auto-migrates on first run
```

Check Hyphae release notes at https://github.com/basidiocarp/hyphae/releases for migration steps.

### Setup still looks wrong after update

If the issue is onboarding, shared repair, or MCP registration, run:

```bash
stipe init
```

If the issue is only Mycelium guidance files, refresh them directly:

```bash
mycelium init -g --claude-md
mycelium init --claude-md
```

Inspect the current Mycelium state with:

```bash
mycelium config
```

## Checking for Updates

### Check for newer versions

```bash
mycelium --version   # Shows current version
cargo search mycelium-cli --limit 1  # Shows latest on crates.io
```

Or visit the GitHub release pages:

- [Mycelium releases](https://github.com/basidiocarp/mycelium/releases)
- [Hyphae releases](https://github.com/basidiocarp/hyphae/releases)
- [Rhizome releases](https://github.com/basidiocarp/rhizome/releases)
- [Cap releases](https://github.com/basidiocarp/cap/releases)

### Enable automatic update checks

Some tools support checking for updates. Check their documentation:

```bash
mycelium --check-updates
hyphae --check-updates
```

## Breaking Changes

### From Mycelium 0.4.x to 0.5.x

- Hook format changed from inline logic to "thin delegator" pattern
- MYCELIUM.md now in `~/.claude/` instead of `./`
- `@MYCELIUM.md` references replace 137-line blocks in CLAUDE.md

**Action**: Run `mycelium init -g --claude-md` to refresh the global Mycelium guidance files. Old configs are backed up.

### From Hyphae 0.2.x to 0.3.x

- Database schema changed (FTS5 + sqlite-vec support)
- Auto-migrates on first `hyphae stats` run

**Action**: Just run `hyphae stats` after updating.

### From Rhizome 0.3.x to 0.4.x

- LSP auto-installation now works for more languages
- Config file location changed from `~/.config/rhizome/` to `~/.rhizome/`

**Action**: Run `rhizome doctor` to validate configuration.

## Downgrading (If Needed)

If an update breaks something, you can downgrade:

```bash
# Downgrade to a specific version
cargo install --locked mycelium-cli@0.4.2
cargo install --locked --git https://github.com/basidiocarp/hyphae hyphae-cli@0.2.0 --no-default-features
```

Then re-run:

```bash
stipe init
```

Report the issue on GitHub so it can be fixed.

## What to Expect After Update

After updating and re-running `stipe init` plus any needed Mycelium hook refresh, you should see:

1. **Improved token savings** — New filter logic usually means better compression
2. **New Hyphae features** — Latest memory recall features, better full-text search
3. **Better code intelligence** — Rhizome updates usually add support for more languages/features
4. **No breaking changes** — The update script maintains backward compatibility

If you notice degraded performance or missing features, check GitHub issues or run:

```bash
mycelium config
```

And report what you find.
