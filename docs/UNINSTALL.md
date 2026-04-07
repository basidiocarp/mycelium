# Uninstalling Mycelium and Ecosystem Tools

This document covers how to cleanly remove Mycelium, Hyphae, Rhizome, and Cap from your system.

## Quick Uninstall (Everything)

To remove all Basidiocarp ecosystem tools and configuration:

```bash
# Remove Mycelium (and all its configuration)
mycelium init -g --uninstall

# Remove binaries
cargo uninstall mycelium-cli hyphae-cli rhizome-cli

# Remove data directories (see below for paths)
```

## Remove Mycelium Only

If you want to keep other tools but remove Mycelium:

```bash
mycelium init -g --uninstall
```

This removes:

1. Hook script: `~/.claude/hooks/mycelium-rewrite.sh`
2. Slim instructions: `~/.claude/MYCELIUM.md`
3. Reference from global CLAUDE.md: `@MYCELIUM.md` line
4. Hook entry in `~/.claude/settings.json`
5. Integrity hash file: `~/.claude/hooks/.mycelium-rewrite.sh.sha256`

The uninstall command will tell you what was removed:

```
Mycelium uninstalled:
  - Hook: ~/.claude/hooks/mycelium-rewrite.sh
  - MYCELIUM.md: ~/.claude/MYCELIUM.md
  - CLAUDE.md: removed @MYCELIUM.md reference
  - settings.json: removed Mycelium hook entry

Restart Claude Code to apply changes.
```

If you have a local `./CLAUDE.md` file in a project, you need to manually remove the Mycelium section:

```bash
# Remove the @MYCELIUM.md reference or Mycelium block
nano CLAUDE.md
# or
vi CLAUDE.md

# Find and delete lines containing:
# - @MYCELIUM.md
# - <!-- mycelium-instructions ... --> (old format)
```

## Remove Hyphae

Hyphae is standalone. To fully remove it:

```bash
# Remove binary
cargo uninstall hyphae-cli

# Remove data directory
# macOS
rm -rf ~/Library/Application\ Support/hyphae

# Linux
rm -rf ~/.local/share/hyphae

# Windows (WSL or native)
rm -rf %APPDATA%\hyphae

# Remove config directory
# macOS
rm -rf ~/.config/hyphae

# Linux
rm -rf ~/.config/hyphae
```

To unregister it from Claude Code:

```bash
claude mcp remove hyphae
```

Or manually edit `~/.claude/settings.json` and remove the Hyphae MCP server and any Hyphae-related hooks.

## Remove Rhizome

Rhizome is standalone. To fully remove it:

```bash
# Remove binary
cargo uninstall rhizome-cli

# Remove data directory
rm -rf ~/.rhizome

# Remove config directory (if it has one)
rm -rf ~/.config/rhizome
```

To unregister it from Claude Code:

```bash
claude mcp remove rhizome
```

Or manually edit `~/.claude/settings.json` and remove the Rhizome MCP server.

## Remove Cap

Cap is optional and standalone:

```bash
# If you cloned it into ~/projects/cap
rm -rf ~/projects/cap

# Or wherever you cloned it
rm -rf /path/to/cap

# No system-wide installation needed (it's a web app)
# No config files to remove
```

## Remove All Hooks and Captures

If you want to keep Mycelium but remove all hooks (command rewriting, error capture, etc.):

```bash
# Remove hook files
rm -rf ~/.claude/hooks/mycelium-rewrite.sh
rm -rf ~/.claude/hooks/mycelium-session-summary.sh
rm -rf ~/.claude/hooks/session-summary.sh  # legacy installs
rm -rf ~/.claude/hooks/basidiocarp/

# Remove hooks from settings.json manually
# Or reinstall with hook-only:
mycelium init -g --uninstall
mycelium init -g --hook-only
```

## Remove Ecosystem Configuration From Claude Code

### Remove All MCP Servers

```bash
claude mcp remove hyphae
claude mcp remove rhizome
```

Verify they're gone:

```bash
claude mcp list
# Should not show hyphae or rhizome
```

### Remove All Hooks From settings.json

If you want to manually edit `~/.claude/settings.json`:

```bash
# Backup first
cp ~/.claude/settings.json ~/.claude/settings.json.backup

# Edit and remove:
# 1. All "PreToolUse" hooks (mycelium-rewrite.sh)
# 2. All "Stop" hooks (mycelium-session-summary.sh, or legacy session-summary.sh)
# 3. All "PostToolUse" hooks (capture-*.js)

nano ~/.claude/settings.json

# Verify JSON is valid
jq . ~/.claude/settings.json > /dev/null && echo "Valid"
```

Example of what to remove:

```json
{
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "Bash",
        "hooks": [
          {
            "type": "command",
            "command": "/Users/alice/.claude/hooks/mycelium-rewrite.sh"
          }
        ]
      }
    ],
    "Stop": [
      {
        "matcher": "",
        "hooks": [
          {
            "type": "command",
            "command": "/Users/alice/.claude/hooks/mycelium-session-summary.sh"
          }
        ]
      }
    ],
    "PostToolUse": [
      {
        "matcher": "Bash",
        "hooks": [
          {
            "type": "command",
            "command": "/Users/alice/.claude/hooks/basidiocarp/capture-errors.js"
          }
        ]
      }
      // ... more capture hooks
    ]
  }
}
```

Remove the entire `"hooks"` section or just the specific entries.

## Remove Other Clients' Configuration

If you configured Hyphae/Rhizome in other MCP clients, remove them:

### Cursor

```bash
# Remove from ~/.cursor/mcp.json
nano ~/.cursor/mcp.json

# Remove entries for hyphae and rhizome
# Then restart Cursor
```

### Windsurf

```bash
# Remove from ~/.windsurf/mcp.json
nano ~/.windsurf/mcp.json

# Remove entries for hyphae and rhizome
# Then restart Windsurf
```

### Cline (VS Code)

```bash
# Remove from .vscode/cline_mcp_config.json (in your project)
nano .vscode/cline_mcp_config.json

# Remove entries for hyphae and rhizome
# Then restart VS Code
```

### Continue

```bash
# Remove from ~/.continue/config.json
nano ~/.continue/config.json

# Remove entries for hyphae and rhizome
# Then restart Continue
```

### Claude Desktop

```bash
# macOS
nano ~/Library/Application\ Support/Claude/claude_desktop_config.json

# Linux
nano ~/.config/Claude/claude_desktop_config.json

# Remove entries for hyphae and rhizome
# Then restart Claude Desktop
```

## Remove From Global Instructions

If you added Mycelium to `~/.claude/CLAUDE.md`:

```bash
nano ~/.claude/CLAUDE.md

# Find and remove lines mentioning:
# - @MYCELIUM.md
# - mycelium commands (gain, discover, proxy)
# - Hyphae/Rhizome integration notes
```

Or use the automatic uninstall:

```bash
mycelium init -g --uninstall
```

## Remove Data Completely

If you want to clear all data and start fresh:

```bash
# Mycelium
rm -f ~/.claude/MYCELIUM.md
rm -f ~/.claude/hooks/mycelium-rewrite.sh
rm -f ~/.claude/hooks/.mycelium-rewrite.sh.sha256

# Hyphae data
rm -rf ~/Library/Application\ Support/hyphae  # macOS
rm -rf ~/.local/share/hyphae                   # Linux

# Hyphae config
rm -rf ~/.config/hyphae

# Rhizome
rm -rf ~/.rhizome
rm -rf ~/.config/rhizome

# Hooks
rm -rf ~/.claude/hooks/mycelium-session-summary.sh
rm -rf ~/.claude/hooks/session-summary.sh  # legacy installs
rm -rf ~/.claude/hooks/basidiocarp/
```

## Restart After Uninstalling

After removing hooks or MCP servers, restart your AI coding client:

```bash
# Claude Code
# Close and reopen, or use:
# Cmd+Shift+P → Developer: Reload Window

# Cursor
# Close and reopen

# Windsurf
# Close and reopen

# VS Code with Cline
# Close and reopen

# Continue
# Close and reopen

# Claude Desktop
# Close and reopen
```

## Verify Uninstall

Check that everything was removed:

```bash
# No Mycelium files
ls ~/.claude/hooks/mycelium* 2>/dev/null || echo "Removed"

# No MYCELIUM.md
ls ~/.claude/MYCELIUM.md 2>/dev/null || echo "Removed"

# No Hyphae data
ls ~/Library/Application\ Support/hyphae 2>/dev/null || echo "Removed"  # macOS
ls ~/.local/share/hyphae 2>/dev/null || echo "Removed"                # Linux

# No Rhizome config
ls ~/.rhizome 2>/dev/null || echo "Removed"

# Binaries uninstalled
which mycelium || echo "Removed"
which hyphae || echo "Removed"
which rhizome || echo "Removed"
```

All should output "Removed".

## Rollback (If You Change Your Mind)

If you uninstall by mistake:

1. **Reinstall binaries**:

```bash
cargo install --locked mycelium-cli hyphae-cli rhizome-cli
```

2. **Restore from backups** (if you made them):

```bash
# Restore Claude Code settings
cp ~/.claude/settings.json.backup ~/.claude/settings.json
```

3. **Reconfigure**:

```bash
stipe init
```

Or just restore from version control if your dotfiles are tracked:

```bash
git checkout ~/.claude/settings.json ~/.claude/CLAUDE.md
```

## Troubleshooting

### "command not found" after uninstall

This is expected. Just don't use the command.

If you want to reinstall:

```bash
cargo install --locked mycelium-cli
stipe init
```

### Hooks still executing after uninstall

Claude Code might have the hooks cached. Solution:

1. Make sure the hook file is actually deleted:

```bash
ls ~/.claude/hooks/mycelium-rewrite.sh
# Should return "No such file or directory"
```

2. Edit `~/.claude/settings.json` and remove the hook entry manually

3. Validate JSON:

```bash
jq . ~/.claude/settings.json > /dev/null
```

4. Restart Claude Code completely

### settings.json corrupted during manual edit

If you broke the JSON syntax:

```bash
# Restore from backup
cp ~/.claude/settings.json.backup ~/.claude/settings.json

# Or use jq to validate
jq . ~/.claude/settings.json
# If it outputs "parse error", revert the backup
```

### Can't find configuration files

Configuration files are in `~/.claude/` (note: dot directory, hidden by default).

To view hidden files:

```bash
# macOS / Linux
ls -la ~/.claude/

# Or use your file manager
# macOS: Cmd+Shift+. (dot) togglees hidden files in Finder
# VS Code: "Files: Exclude" in settings
```

## After Uninstalling

Once Mycelium is uninstalled:

- Commands run without filtering (no token savings)
- No automatic error/correction capture
- Hyphae/Rhizome work independently (if you kept them)
- Claude Code itself is unaffected—the hooks added negligible overhead

You can reinstall anytime:

```bash
cargo install --locked mycelium-cli
stipe init
```

All your Hyphae memories and Rhizome code index are preserved (they're in separate data directories).
