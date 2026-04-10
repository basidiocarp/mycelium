# Updating Mycelium

This page covers the Mycelium-owned update path.

Use it when you need to:

- update the `mycelium` binary
- verify that the installed binary and hook still agree
- refresh Mycelium-managed Claude guidance after an upgrade

Use `stipe init` only when the problem is wider than Mycelium itself, such as
shared ecosystem onboarding, MCP registration, or cross-tool repair.

## Quick Path

```bash
mycelium --version
cargo install --locked --git https://github.com/basidiocarp/mycelium
mycelium doctor
mycelium verify
```

If you use Claude Code hook integration on a supported platform, refresh the
Mycelium-owned setup after updating:

```bash
mycelium init -g
```

If you only want the guidance files refreshed:

```bash
mycelium init -g --claude-md
mycelium init --claude-md
```

## Update Methods

### Cargo from Git

This is the most direct update path:

```bash
cargo install --locked --git https://github.com/basidiocarp/mycelium
```

### Release Installer

If you originally installed with the repo installer, you can re-run it:

```bash
curl -fsSL https://raw.githubusercontent.com/basidiocarp/mycelium/refs/heads/master/install.sh | sh
```

### Built-In Update Check

Mycelium also exposes its own update check surface:

```bash
mycelium self-update --check
```

Use that to inspect whether a newer release exists before changing anything.

## After Updating

### Verify the Binary

```bash
mycelium --version
mycelium gain
mycelium doctor
mycelium verify
```

What you are checking:

- `mycelium --version`: the binary is on `PATH`
- `mycelium gain`: the installed command is the real Mycelium binary
- `mycelium doctor`: setup and health look sane
- `mycelium verify`: the installed hook baseline still matches

### Refresh Claude Code Integration

On supported platforms, `mycelium init -g` repairs the thin delegator hook,
`MYCELIUM.md`, and the guarded Claude settings patch:

```bash
mycelium init -g
```

If the hook is not the problem and you only need the guidance files updated,
use the docs-only modes instead:

```bash
mycelium init -g --claude-md
mycelium init --claude-md
```

### Re-Run Shared Ecosystem Repair Only When Needed

If Hyphae, Rhizome, or other shared ecosystem surfaces also drifted, use:

```bash
stipe init
```

That is not part of the Mycelium-only update path. It is the shared repair
path for the broader workspace.

## Common Problems

### `mycelium --version` works but `mycelium gain` fails

You likely have the wrong package installed under the `mycelium` name.

```bash
cargo uninstall mycelium
cargo install --locked --git https://github.com/basidiocarp/mycelium --root ~/.local
mycelium gain
```

### Commands stopped rewriting after an update

The binary updated, but the Claude hook adapter or settings registration
drifted.

```bash
mycelium doctor
mycelium verify
mycelium init -g
```

### Docs and CLI disagree after an update

Refresh Mycelium-managed guidance files:

```bash
mycelium init -g --claude-md
mycelium init --claude-md
```

### The problem is not Mycelium alone

If the failure includes MCP registration, Hyphae, Rhizome, or wider ecosystem
setup, switch to:

```bash
stipe init
```

## See Also

- [getting-started/installation.md](getting-started/installation.md)
- [getting-started/ecosystem-setup.md](getting-started/ecosystem-setup.md)
- [commands.md](commands.md)
- [troubleshooting.md](troubleshooting.md)
