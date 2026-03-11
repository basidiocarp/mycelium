# Plugins

Mycelium plugins are user-defined filter scripts that extend mycelium to support commands it doesn't handle natively. Plugins receive raw command output on stdin and write filtered output to stdout.

## How It Works

```
Command runs → raw output captured → piped to plugin stdin → plugin stdout printed
```

When mycelium encounters a command with no built-in handler, it checks the plugin directory for a matching script. If found, the raw command output is piped through the plugin before reaching the LLM.

## Quick Start

1. Create the plugin directory:
   ```bash
   mkdir -p ~/.config/mycelium/plugins
   ```

2. Create a plugin script (e.g., for a command called `mycommand`):
   ```bash
   cat > ~/.config/mycelium/plugins/mycommand.sh << 'EOF'
   #!/usr/bin/env bash
   # Filter output from "mycommand"
   # Receives raw command output on stdin, writes filtered output to stdout

   INPUT=$(cat)
   LINE_COUNT=$(echo "$INPUT" | wc -l)

   if [ "$LINE_COUNT" -lt 50 ]; then
     # Small output — pass through unchanged
     echo "$INPUT"
   else
     # Large output — keep first 50 lines + summary
     echo "$INPUT" | head -50
     echo ""
     echo "... ($LINE_COUNT total lines)"
   fi
   EOF
   ```

3. Make it executable:
   ```bash
   chmod 755 ~/.config/mycelium/plugins/mycommand.sh
   ```

4. Test it standalone:
   ```bash
   echo "some output" | ~/.config/mycelium/plugins/mycommand.sh
   ```

## Plugin Directory

Default locations:

| Platform | Path |
|----------|------|
| Linux | `~/.config/mycelium/plugins/` |
| macOS | `~/Library/Application Support/mycelium/plugins/` |

Override in `~/.config/mycelium/config.toml`:
```toml
[plugins]
enabled = true
directory = "/path/to/custom/plugins"
```

Set `enabled = false` to disable all plugins without deleting them.

## Discovery

When mycelium looks for a plugin for command `foo`, it checks two candidates in order:

1. `<plugin_dir>/foo.sh` (preferred)
2. `<plugin_dir>/foo` (bare executable)

The first match that passes all checks is used.

## Execution Contract

| Aspect | Behavior |
|--------|----------|
| **Input** | Raw command output piped to stdin |
| **Output** | Filtered output written to stdout |
| **Stderr** | Inherited — plugin stderr prints to the terminal |
| **Exit code 0** | Success — plugin output is used |
| **Exit code != 0** | Failure — mycelium falls back to raw output |
| **Timeout** | 10 seconds — plugin is killed if it exceeds this |
| **Empty stdin** | Plugin must handle gracefully (exit 0, no output) |

## Security

Plugins must pass two security checks on Unix systems:

1. **Not world-writable** — file permissions must not include the world-write bit (`o+w`). A plugin with mode `0777` is rejected; `0755` is accepted.

2. **Owned by current user** — the file's UID must match the `UID` environment variable. If `UID` is not set, this check is skipped and only the world-writable check applies.

These checks prevent execution of plugins that may have been tampered with by other users on shared systems.

On Windows, both checks are skipped (all plugins are trusted).

### Recommended permissions

```bash
chmod 755 ~/.config/mycelium/plugins/myplugin.sh   # rwxr-xr-x
```

## Writing Plugins

### Minimal template

```bash
#!/usr/bin/env bash
INPUT=$(cat)
# Filter logic here
echo "$INPUT"
```

### Pattern: Error-only filter

Show only errors and warnings, suppress success output:

```bash
#!/usr/bin/env bash
INPUT=$(cat)

# Check for errors/warnings
ERRORS=$(echo "$INPUT" | grep -iE '(error|warning|fail|fatal)')

if [ -n "$ERRORS" ]; then
  echo "$ERRORS"
else
  echo "ok"
fi
```

### Pattern: JSON schema extraction

For commands that produce large JSON, extract keys and types:

```bash
#!/usr/bin/env bash
INPUT=$(cat)

# If jq is available, extract schema
if command -v jq &>/dev/null; then
  echo "$INPUT" | jq '[paths(scalars) | join(".")] | unique | .[]' 2>/dev/null && exit 0
fi

# Fallback: truncate
LINE_COUNT=$(echo "$INPUT" | wc -l | tr -d ' ')
if [ "$LINE_COUNT" -gt 100 ]; then
  echo "$INPUT" | head -50
  echo ""
  echo "... ($LINE_COUNT total lines, truncated)"
else
  echo "$INPUT"
fi
```

### Pattern: Terraform-style filter

For commands that wrap Terraform (e.g., atmos, terragrunt):

```bash
#!/usr/bin/env bash
INPUT=$(cat)

# Extract resource changes and summary from plan output
if echo "$INPUT" | grep -q "Terraform will perform"; then
  echo "$INPUT" | grep -E '^\s+#|^Plan:|^Changes|^Apply complete'
  exit 0
fi

# Extract completion messages from apply output
if echo "$INPUT" | grep -q "Apply complete"; then
  echo "$INPUT" | grep -E '(complete|created|destroyed|modified|Apply complete)'
  exit 0
fi

# Default: passthrough
echo "$INPUT"
```

### Pattern: Deduplication

Collapse repeated log lines with counts:

```bash
#!/usr/bin/env bash
INPUT=$(cat)

echo "$INPUT" | sort | uniq -c | sort -rn | head -50 | awk '{
  count = $1
  $1 = ""
  line = substr($0, 2)
  if (count > 1) {
    printf "%5dx %s\n", count, line
  } else {
    printf "      %s\n", line
  }
}'
```

## Guidelines

- **Keep plugins fast.** The 10-second timeout is a hard kill. Most filters should complete in milliseconds.
- **Handle empty input.** Your plugin should exit 0 with no output when stdin is empty, not hang waiting for input.
- **Prefer passthrough over failure.** If your filter can't parse the output, print it unchanged rather than failing. A non-zero exit code means mycelium falls back to raw output, which works but skips savings tracking.
- **Use bash, not zsh.** Plugins run on Linux too. Use `#!/usr/bin/env bash` for portability.
- **Don't call mycelium from plugins.** This creates circular dependencies. Implement filtering in bash/awk/sed/jq directly.
- **Test standalone first.** Run `echo "test input" | ./myplugin.sh` before installing to the plugin directory.

## Debugging

### Test a plugin manually

```bash
# Pipe real command output through the plugin
somecommand --args | ~/.config/mycelium/plugins/somecommand.sh

# Or use a fixture file
cat tests/fixtures/somecommand_raw.txt | ~/.config/mycelium/plugins/somecommand.sh
```

### Check plugin discovery

```bash
# Verify the plugin exists and has correct permissions
ls -la ~/.config/mycelium/plugins/

# Verify plugins are enabled
cat ~/.config/mycelium/config.toml
```

### Common issues

| Problem | Cause | Fix |
|---------|-------|-----|
| Plugin not found | Wrong filename or directory | Name must match command: `foo.sh` for command `foo` |
| Plugin not found | Not executable | `chmod 755 plugin.sh` |
| Plugin not found | Plugins disabled | Set `enabled = true` in config.toml |
| Plugin rejected | World-writable | `chmod 755 plugin.sh` (not `777`) |
| Plugin killed | Exceeded 10s timeout | Optimize filter logic or use simpler processing |
| Fallback to raw | Plugin exited non-zero | Check stderr output, fix script errors |
| Plugin hangs | Reading stdin with no input | Add `INPUT=$(cat)` at top, don't use interactive reads |

## Configuration Reference

Full `[plugins]` section in `~/.config/mycelium/config.toml`:

```toml
[plugins]
# Enable or disable all plugins (default: true)
enabled = true

# Plugin directory (default: platform-specific config dir)
# Linux: ~/.config/mycelium/plugins
# macOS: ~/Library/Application Support/mycelium/plugins
directory = "~/.config/mycelium/plugins"
```

## Shipped Plugins

Mycelium ships example plugins in the `plugins/` directory of the repository. These are reference implementations you can install directly.

| Plugin | Command | What it filters |
|--------|---------|-----------------|
| `atmos.sh` | `atmos` | Terraform plan/apply output, `describe` JSON/YAML, validation errors |

### Real-world example: atmos

`plugins/atmos.sh` filters output from [atmos](https://atmos.tools/) — Cloud Posse's infrastructure orchestration tool that wraps Terraform with stacks and components.

It routes output by type:
- **`terraform plan`** output → extracts resource change lines (`# module.x`) and the summary (`Plan: N to add`)
- **`terraform apply`** output → extracts completion messages (`Apply complete!`, `Creation complete`)
- **`describe stacks/component`** output → extracts JSON/YAML keys with `jq` or truncates to 50 lines
- **`validate`** output → shows only errors and warnings, suppresses success lines
- **Other commands** → passes through unchanged

Install and test:
```bash
./scripts/install-plugin.sh atmos

# Test with sample plan output
echo "Plan: 3 to add, 1 to change, 0 to destroy." | ~/.config/mycelium/plugins/atmos.sh
```

## Installing Shipped Plugins

Use `scripts/install-plugin.sh` to copy plugins from the repo to your plugin directory:

```bash
# List available plugins
./scripts/install-plugin.sh --list

# Install a specific plugin
./scripts/install-plugin.sh atmos

# Install all shipped plugins
./scripts/install-plugin.sh --all

# Force overwrite if already installed
./scripts/install-plugin.sh --force atmos
```

The script creates `~/.config/mycelium/plugins/` if it doesn't exist and sets `chmod 755` on installed plugins.

**Verify installation**:
```bash
ls -la ~/.config/mycelium/plugins/
# -rwxr-xr-x  atmos.sh
```

## See Also

- [ARCHITECTURE.md](ARCHITECTURE.md) — System architecture
- [TRACKING.md](TRACKING.md) — How token savings are tracked
- [COST_ANALYSIS.md](COST_ANALYSIS.md) — Economics and accuracy of savings
