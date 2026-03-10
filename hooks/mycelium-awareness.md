# Mycelium

**Usage**: Token-optimized CLI proxy (60-90% savings on dev operations)

## Meta Commands (always use mycelium directly)

```bash
mycelium gain              # Show token savings analytics
mycelium gain --history    # Show command usage history with savings
mycelium discover          # Analyze Claude Code history for missed opportunities
mycelium proxy <cmd>       # Execute raw command without filtering (for debugging)
```

## Installation Verification

```bash
mycelium --version         # Should show: mycelium X.Y.Z
mycelium gain              # Should work (not "command not found")
which mycelium             # Verify correct binary
```

## Hook-Based Usage

All other commands are automatically rewritten by the Claude Code hook.
Example: `git status` → `mycelium git status` (transparent, 0 tokens overhead)

Refer to CLAUDE.md for full command reference.
