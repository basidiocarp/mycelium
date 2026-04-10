# Installation

Install the Mycelium binary first, then use the wider ecosystem setup only if
you want shared onboarding and repair.

## Check the Current State

Before installing, check whether the correct binary is already on your path:

```bash
mycelium --version
mycelium gain
which mycelium
```

If `mycelium gain` works, you already have the right binary. If
`mycelium --version` works but `mycelium gain` does not, you likely have the
wrong package installed under the `mycelium` name.

## Install the Binary

### Recommended: Cargo from Git

```bash
cargo install --locked --git https://github.com/basidiocarp/mycelium
```

### Alternative: Release Installer

On macOS or Linux you can use the repo installer script:

```bash
curl -fsSL https://raw.githubusercontent.com/basidiocarp/mycelium/refs/heads/master/install.sh | sh
```

## Verify the Install

After installing, confirm that the binary on your path is the Basidiocarp
Mycelium binary:

```bash
mycelium --version
mycelium gain
mycelium doctor
```

`mycelium gain` should report savings information rather than failing with
`command not found`.

## Next Step

Installing the binary does not do shared ecosystem setup by itself.

- For wider ecosystem onboarding, MCP registration, or shared repair, use
  [ecosystem-setup.md](ecosystem-setup.md) and run `stipe init`.
- For Mycelium-owned Claude setup and hook repair, use `mycelium init -g`,
  `mycelium init -g --claude-md`, or `mycelium init --claude-md` as described
  in [ecosystem-setup.md](ecosystem-setup.md).

## Installation Verification

```bash
# Basic test
mycelium ls .

# Test with git
mycelium git status

# Test with pnpm (fork only)
mycelium pnpm list

# Test with Vitest (feat/vitest-support branch only)
mycelium vitest run
```

## Uninstalling

### Complete Removal (Global Installations Only)

```bash
# Complete removal (global installations only)
mycelium init -g --uninstall

# What gets removed:
#   - Hook: ~/.claude/hooks/mycelium-rewrite.sh
#   - Context: ~/.claude/Mycelium.md
#   - Reference: @Mycelium.md line from ~/.claude/CLAUDE.md
#   - Registration: Mycelium hook entry from settings.json

# Restart Claude Code after uninstall
```

For local projects: manually remove Mycelium block from `./CLAUDE.md`

### Binary Removal

```bash
# If installed via cargo
cargo uninstall mycelium

# If installed via package manager
brew uninstall mycelium          # macOS Homebrew
sudo apt remove mycelium         # Debian/Ubuntu
sudo dnf remove mycelium         # Fedora/RHEL
```

### Restore from Backup (if needed)

```bash
cp ~/.claude/settings.json.bak ~/.claude/settings.json
```

## Essential Commands

### Files
```bash
mycelium ls .              # Compact tree view
mycelium read file.rs      # Optimized reading
mycelium grep "pattern" .  # Grouped search results
```

### Git
```bash
mycelium git status        # Compact status
mycelium git log -n 10     # Condensed logs
mycelium git diff          # Optimized diff
mycelium git add .         # → "ok ✓"
mycelium git commit -m "msg"  # → "ok ✓ abc1234"
mycelium git push          # → "ok ✓ main"
```

### Pnpm (fork only)
```bash
mycelium pnpm list         # Dependency tree (-70% tokens)
mycelium pnpm outdated     # Available updates (-80-90%)
mycelium pnpm install pkg  # Silent installation
```

### Tests
```bash
mycelium test cargo test   # Failures only (-90%)
mycelium vitest run        # Filtered Vitest output (-99.6%)
```

### Statistics
```bash
mycelium gain              # Token savings
mycelium gain --graph      # With ASCII graph
mycelium gain --history    # With command history
```

## Validated Token Savings

### Production T3 Stack Project
| Operation | Standard | Mycelium | Reduction |
|-----------|----------|-----|-----------|
| `vitest run` | 102,199 chars | 377 chars | -99.6% |
| `git status` | 529 chars | 217 chars | -59% |
| `pnpm list` | ~8,000 tokens | ~2,400 | -70% |
| `pnpm outdated` | ~12,000 tokens | ~1,200-2,400 | -80-90% |

### Typical Claude Code Session (30 min)
- Without Mycelium: ~150,000 tokens
- With Mycelium: ~45,000 tokens
- Savings: 70% reduction

## Troubleshooting

### Mycelium command not found after installation
```bash
# Check PATH
echo $PATH | grep -o '[^:]*\.cargo[^:]*'

# Add to PATH if needed (~/.bashrc or ~/.zshrc)
export PATH="$HOME/.cargo/bin:$PATH"

# Reload shell
source ~/.bashrc  # or source ~/.zshrc
```

### Mycelium command not available (e.g., vitest)
```bash
# Check branch
cd /path/to/mycelium
git branch

# Switch to feat/vitest-support if needed
git checkout feat/vitest-support

# Reinstall
cargo install --path . --force
```

### Compilation error
```bash
# Update Rust
rustup update stable

# Clean and recompile
cargo clean
cargo build --release
cargo install --path . --force
```
## AI Assistant Checklist

Before each session:

- [ ] Verify Mycelium is installed: `mycelium --version`
- [ ] If not installed → follow "Install from fork"
- [ ] If project not initialized → `mycelium init`
- [ ] Use `mycelium` for ALL git/pnpm/test/vitest commands
- [ ] Check savings: `mycelium gain`

Always prefix shell commands with `mycelium` when they produce verbose output (git, pnpm, npm, cargo test, vitest, docker, kubectl).
