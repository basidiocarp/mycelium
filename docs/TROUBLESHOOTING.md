# Mycelium Troubleshooting

Common issues and fast checks for Mycelium installation, rewrites, and companion integrations.

---

## Commands Are Not Rewriting

Check the current setup:

```bash
mycelium doctor
mycelium verify
mycelium init --show
```

If the Claude Code hook adapter is missing or stale, repair it with:

```bash
mycelium init -g
```

Use `stipe init` when you want to repair the wider ecosystem setup instead of Mycelium-only files.

---

## A Command Passed Through Raw

Some commands intentionally fall back to passthrough:
- unsupported subcommands
- shell shapes that are unsafe to rewrite
- commands whose parser or filter could not recover safely

Inspect recent behavior with:

```bash
mycelium gain --diagnostics
mycelium gain --failures
mycelium discover
```

---

## Hyphae or Rhizome Features Are Not Active

Check whether the companion binaries are available:

```bash
hyphae --version
rhizome --version
```

Then inspect Mycelium configuration:

```bash
mycelium config
```

If `[filters.hyphae] enabled = false` or `[filters.rhizome] enabled = false`, Mycelium will stay on local filtering.

---

## Output Looks Too Aggressive or Too Verbose

Start by inspecting current configuration:

```bash
mycelium config
```

Relevant controls include:
- compaction profile
- adaptive thresholds
- command-specific filter options

For file reads, adjust the filtering mode directly:

```bash
mycelium read path/to/file --level none
mycelium read path/to/file --level aggressive
```

---

## Compare Raw vs Filtered Output

Use passthrough or side-by-side comparison when debugging filter behavior:

```bash
mycelium proxy git status
mycelium gain --compare "git status"
```

---

## Updates and Version Drift

Check the installed version:

```bash
mycelium --version
```

Check for updates:

```bash
mycelium self-update --check
```

If docs and CLI behavior disagree after upgrading, regenerate local setup and re-run `mycelium doctor`.
