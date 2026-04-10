# Uninstalling Mycelium

This page covers the Mycelium-owned uninstall path.

Use it when you want to:

- remove the `mycelium` binary
- remove Mycelium-managed Claude hook and guidance files
- remove Mycelium's local config or tracking data

This page does not try to fully uninstall the rest of the Basidiocarp
ecosystem. If you also want to remove Hyphae, Rhizome, or broader shared setup,
handle those tools separately.

## Quick Path

Remove Mycelium-managed Claude setup:

```bash
mycelium init -g --uninstall
```

Remove the binary:

```bash
cargo uninstall mycelium
```

Then verify:

```bash
which mycelium
```

If `which mycelium` still resolves, you likely have another installation on
your `PATH`.

## What `mycelium init -g --uninstall` Removes

On supported platforms, the uninstall command removes the Mycelium-owned Claude
integration:

- `~/.claude/hooks/mycelium-rewrite.sh`
- `~/.claude/MYCELIUM.md`
- the `@MYCELIUM.md` reference in global `CLAUDE.md`
- the Mycelium hook entry in `~/.claude/settings.json`
- the stored hook integrity hash

If you have a project-local `CLAUDE.md`, remove any Mycelium-specific guidance
there manually if it still remains.

## Remove Local State

If you also want to remove Mycelium's local files, inspect the active paths
first:

```bash
mycelium config
```

Common locations include:

- tracking database under `~/.local/share/mycelium/`
- config under `~/.config/mycelium/`

Remove them only if you want a full local reset:

```bash
rm -rf ~/.local/share/mycelium
rm -rf ~/.config/mycelium
```

## If You Only Want to Remove the Hook

If you want to keep the binary but remove the Claude integration:

```bash
mycelium init -g --uninstall
```

You can later restore only the docs surfaces:

```bash
mycelium init -g --claude-md
mycelium init --claude-md
```

Or restore the full supported hook path:

```bash
mycelium init -g
```

## If the Wider Ecosystem Is Also Installed

Mycelium uninstall does not remove Hyphae, Rhizome, or shared ecosystem setup.
If you also want those gone, remove those tools separately.

If you only need to repair the shared ecosystem after partial removal, use:

```bash
stipe init
```

## Common Problems

### Hooks still seem active after uninstall

Run the uninstall command again, then inspect the remaining Claude state:

```bash
mycelium init -g --uninstall
mycelium doctor
```

If needed, inspect `~/.claude/settings.json` directly and remove any remaining
Mycelium hook entries.

### `mycelium` is still on `PATH`

You likely have another installed copy.

```bash
which mycelium
```

Remove the binary from whichever install root `which` reports.

### I removed too much

Reinstall the binary and then restore the Mycelium-managed surfaces:

```bash
cargo install --locked --git https://github.com/basidiocarp/mycelium
mycelium init -g
```

## See Also

- [getting-started/installation.md](getting-started/installation.md)
- [getting-started/ecosystem-setup.md](getting-started/ecosystem-setup.md)
- [troubleshooting.md](troubleshooting.md)
