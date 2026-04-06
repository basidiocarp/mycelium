# Mycelium Troubleshooting

Start with `mycelium doctor`. Most failures trace back to a stale Claude hook adapter, the wrong `mycelium` binary on `PATH`, or a command that correctly fell back to passthrough.

## Fast Triage

| Symptom | First Command | What it usually means |
|---------|---------------|-----------------------|
| Commands are not rewriting in Claude Code | `mycelium doctor` | The hook adapter is missing, stale, or not registered |
| `mycelium --version` works but `mycelium gain` does not | `mycelium gain` | You likely installed the wrong package |
| One command passed through raw | `mycelium gain --diagnostics` | The command was unsupported, unsafe to rewrite, or could not be filtered safely |
| Hyphae or Rhizome behavior never shows up | `mycelium config` | Companion filters are disabled or the companion binaries are missing |
| Output feels too short or too verbose | `mycelium config` | The compaction profile or command-specific filter settings need adjustment |

## Installation and Registration Issues

### Commands are not rewriting

**Symptom:** Claude Code keeps running raw commands such as `git status` instead of the Mycelium rewrite path.

**Diagnosis:** The Claude hook adapter is missing, stale, or not registered in the active settings file.

**Fix:**

1. Check the current installation and hook state:
   ```bash
   mycelium doctor
   mycelium verify
   mycelium init --show
   ```
   A healthy setup shows the hook as installed and up to date.

2. Repair the Mycelium-owned Claude hook adapter:
   ```bash
   mycelium init -g
   ```

3. If the wider ecosystem setup looks wrong too, repair it from the installer side and then restart Claude Code:
   ```bash
   stipe init
   ```

### `mycelium --version` works but `mycelium gain` says `command not found`

**Symptom:** The binary exists, but `mycelium gain` fails even though `mycelium --version` prints a version.

**Diagnosis:** You likely installed the wrong package under the `mycelium` name.

**Fix:**

1. Confirm the mismatch:
   ```bash
   mycelium --version
   mycelium gain
   ```
   The bad case is simple: version works, but `gain` is missing.

2. Remove the wrong package:
   ```bash
   cargo uninstall mycelium
   ```

3. Install the real Mycelium CLI and verify it with `gain`:
   ```bash
   cargo install --locked --git https://github.com/basidiocarp/mycelium mycelium-cli --root ~/.local
   mycelium gain
   ```

### Commands stopped rewriting after an update

**Symptom:** Rewrites used to work, then stopped after upgrading Mycelium or Claude setup files.

**Diagnosis:** The installed hook path, executable bit, or Claude settings entry drifted during the upgrade.

**Fix:**

1. Check whether the hook file still exists:
   ```bash
   ls -la ~/.claude/hooks/mycelium-rewrite.sh
   ```

2. Reinstall the hook adapter:
   ```bash
   mycelium init -g
   ```

3. Verify the settings entry still points at the current hook and restart Claude Code:
   ```bash
   jq .hooks.PreToolUse ~/.claude/settings.json | grep mycelium-rewrite
   ```

## Rewrite and Filter Behavior

### A command passed through raw

**Symptom:** A command runs without Mycelium filtering even though you expected a compact result.

**Diagnosis:** The command was unsupported, unsafe to rewrite, or the parser could not recover safely enough to keep semantic fidelity.

**Fix:**

1. Inspect recent rewrite and fallback behavior:
   ```bash
   mycelium gain --diagnostics
   mycelium gain --failures
   ```
   These commands show whether Mycelium skipped the rewrite or failed after trying.

2. Review missed opportunities in recent history:
   ```bash
   mycelium discover
   ```

3. If you need the raw output for comparison, run it explicitly:
   ```bash
   mycelium proxy git status
   ```

### Output looks too aggressive or too verbose

**Symptom:** Filtered output drops too much detail, or it barely compresses anything useful.

**Diagnosis:** The current compaction profile or command-specific filter settings do not match the task or file size.

**Fix:**

1. Inspect the active configuration:
   ```bash
   mycelium config
   ```

2. Adjust file-read compression directly to see the difference:
   ```bash
   mycelium read path/to/file --level none
   mycelium read path/to/file --level aggressive
   ```

3. If the behavior still looks wrong, compare raw and filtered results for the exact command:
   ```bash
   mycelium gain --compare "git status"
   ```

## Companion Integration Issues

### Hyphae or Rhizome features are not active

**Symptom:** Mycelium stays on local filtering even though Hyphae or Rhizome is installed.

**Diagnosis:** The companion binary is missing from `PATH`, or the matching filter is disabled in config.

**Fix:**

1. Confirm the companion tools are actually available:
   ```bash
   hyphae --version
   rhizome --version
   ```

2. Inspect Mycelium's loaded config:
   ```bash
   mycelium config
   ```
   If `[filters.hyphae] enabled = false` or `[filters.rhizome] enabled = false`, Mycelium will stay on local-only behavior.

3. Re-run ecosystem setup if the whole stack looks out of sync:
   ```bash
   stipe init
   ```

## Updates and Version Drift

### Docs and CLI behavior disagree after upgrading

**Symptom:** The docs describe one flow, but the local CLI acts like an older or partially updated install.

**Diagnosis:** The binary, hook adapter, and guidance files were upgraded at different times.

**Fix:**

1. Check the installed version and whether an update is available:
   ```bash
   mycelium --version
   mycelium self-update --check
   ```

2. Refresh the local hook and guidance setup:
   ```bash
   mycelium init -g
   ```

3. Re-run health checks after the refresh:
   ```bash
   mycelium doctor
   mycelium verify
   ```

## Error Message Quick Reference

| Error | Cause | Fix |
|-------|-------|-----|
| `"command not found"` from `mycelium gain` | The wrong package is installed as `mycelium` | Reinstall the real CLI and verify with `mycelium gain` |
| Commands do not rewrite in Claude Code | Hook adapter is missing, stale, or not registered | Run `mycelium doctor`, then `mycelium init -g` |
| Raw output appears instead of filtered output | The command was unsupported, unsafe to rewrite, or could not be filtered safely | Check `mycelium gain --diagnostics` and `mycelium gain --failures` |
| Hyphae or Rhizome behavior never appears | Companion binary missing or companion filter disabled | Check `hyphae --version`, `rhizome --version`, and `mycelium config` |
| Docs and CLI disagree after upgrade | Binary, hook, and guidance files are out of sync | Run `mycelium self-update --check`, then `mycelium init -g` |

## Diagnostic Commands

**Debug and diagnostics:**
```bash
# Mycelium does not currently expose a dedicated debug-only mode.
# Start with the built-in health and diagnostics commands:
mycelium doctor
mycelium verify
mycelium gain --diagnostics
mycelium gain --failures
```

**Check version:**
```bash
mycelium --version
mycelium self-update --check
```

**Inspect current configuration:**
```bash
mycelium config
cat ~/.config/mycelium/config.toml
```

**Check state and health:**
```bash
mycelium doctor
mycelium verify
mycelium init --show
mycelium gain --status
```

## See also

- [INSTALL.md](../INSTALL.md)
- [docs/COMMANDS.md](COMMANDS.md)
- [docs/ANALYTICS.md](ANALYTICS.md)
- [docs/UPDATE.md](UPDATE.md)
