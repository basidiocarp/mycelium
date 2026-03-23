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

After updating any tool, re-run `stipe init` to refresh onboarding and repair flows. If you changed hooks or client wiring, follow it with `mycelium init --ecosystem` to re-apply the lower-level integration:

```bash
stipe init
mycelium init --ecosystem
```

This will:

1. Detect the new versions
2. Repair onboarding state if needed
3. Re-register MCP servers if the lower-level integration changed
4. Update hook scripts if they're outdated
5. Initialize any new databases or features

Example output:

```
Basidiocarp Ecosystem Status
───────────────────────────────────────────────────────────────────────────

  mycelium      v0.5.3       ✓ installed (new)
  hyphae        v0.3.1       ✓ installed (new)
  rhizome       v0.4.1       ✓ installed (new)
  cap           v1.2.1       ✓ installed (new)

Configuring Claude Code...

  ✓ Configured:
    - hyphae MCP (already registered)
    - rhizome MCP (already registered)
    - mycelium hooks + CLAUDE.md
```

## Restarting Claude Code

After updating Mycelium hooks or re-running `mycelium init --ecosystem`, restart Claude Code to pick up changes:

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

If Claude Code stops filtering commands after an update:

1. Check that hooks still exist:

```bash
ls -la ~/.claude/hooks/mycelium-rewrite.sh
```

2. Re-run `stipe init` to repair onboarding state:

```bash
stipe init
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

### Database schema changes

If Hyphae updates with schema changes, the database might need migration:

```bash
hyphae migrate   # If this command exists
# or
hyphae stats     # Usually auto-migrates on first run
```

Check Hyphae release notes at https://github.com/basidiocarp/hyphae/releases for migration steps.

### MCP server registration fails

If MCP registration fails after update:

```bash
mycelium init --ecosystem
# ! Failed to register hyphae MCP
```

**Solution**:

1. Check the tool is actually installed:

```bash
hyphae serve --help
rhizome serve --help
```

2. Check Claude Code CLI is working:

```bash
claude --version
claude mcp list
```

3. Try manual registration:

```bash
claude mcp add --scope user hyphae -- hyphae serve
claude mcp add --scope user rhizome -- rhizome serve --expanded
```

4. Check Claude Code settings.json for syntax errors:

```bash
jq . ~/.claude/settings.json > /dev/null && echo "Valid JSON" || echo "Invalid JSON"
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

**Action**: Run `mycelium init -g` to migrate automatically. Old configs are backed up.

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
mycelium init --ecosystem
```

Report the issue on GitHub so it can be fixed.

## What to Expect After Update

After updating and re-running `mycelium init --ecosystem`, you should see:

1. **Improved token savings** — New filter logic usually means better compression
2. **New Hyphae features** — Latest memory recall features, better full-text search
3. **Better code intelligence** — Rhizome updates usually add support for more languages/features
4. **No breaking changes** — The update script maintains backward compatibility

If you notice degraded performance or missing features, check GitHub issues or run:

```bash
mycelium init --ecosystem -vv   # Verbose output
```

And report what you find.
