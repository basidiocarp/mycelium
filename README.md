# Mycelium

Token-optimized CLI proxy. Filters and compresses command output before it
reaches your LLM context, typically with low proxy overhead and large savings on
common developer workflows.

Named after fungal mycelium, the hidden network that routes and transforms
resources before they surface elsewhere.

Part of the [Basidiocarp ecosystem](https://github.com/basidiocarp).

---

## The Problem

Most agent sessions waste the majority of their context window on raw terminal
output: verbose test logs, long diffs, repetitive file listings, and boilerplate
command results that the model does not need in full.

## The Solution

Mycelium sits between the agent and the shell. It filters and compresses
command output before it reaches the model, changes strategy by command and
output size, and hands especially large results off to Hyphae or Rhizome when
that gives a better result than dumping raw text.

---

## The Ecosystem

| Tool | Purpose |
|------|---------|
| **[mycelium](https://github.com/basidiocarp/mycelium)** | Token-optimized command output |
| **[cap](https://github.com/basidiocarp/cap)** | Web dashboard for the ecosystem |
| **[cortina](https://github.com/basidiocarp/cortina)** | Lifecycle signal capture and session attribution |
| **[hyphae](https://github.com/basidiocarp/hyphae)** | Persistent agent memory |
| **[lamella](https://github.com/basidiocarp/lamella)** | Skills, hooks, and plugins for coding agents |
| **[rhizome](https://github.com/basidiocarp/rhizome)** | Code intelligence via tree-sitter and LSP |
| **[stipe](https://github.com/basidiocarp/stipe)** | Ecosystem installer and manager |
| **[volva](https://github.com/basidiocarp/volva)** | Execution-host runtime layer |

> **Boundary:** `mycelium` owns token optimization, command filtering, and its
> local guidance surfaces. `stipe` owns onboarding and shared repair. `hyphae`
> and `rhizome` stay isolated behind their own adapters; `dispatch` stays a
> router, not a policy dump.

---

## Quick Start

```bash
# Full ecosystem setup
curl -fsSL https://raw.githubusercontent.com/basidiocarp/.github/main/install.sh | sh
stipe init
```

```bash
# Mycelium-only install
cargo install --git https://github.com/basidiocarp/mycelium

# Useful inspection surfaces
mycelium config
mycelium init -g
```

---

## How It Works

```text
Agent                Mycelium                    Shell / ecosystem
─────                ─────────                   ─────────────────
run command    ─►    proxy command        ─►     shell tool
raw output     ◄──   filter or compress   ◄──    command result
large output   ─►    route to Hyphae      ─►     retrievable chunk
large code     ─►    ask Rhizome          ─►     structural summary
```

1. Inspect command type: choose a filter strategy based on the command and result size.
2. Reduce noise: strip boilerplate, group similar lines, and deduplicate repeated output.
3. Adapt by scale: pass through small results, filter medium results, and compress large ones aggressively.
4. Route when needed: store large outputs in Hyphae or use Rhizome-backed structural reads for big source files.

---

## Savings

| Operation | Frequency | Standard | Mycelium | Savings |
|-----------|-----------|----------|----------|---------|
| `ls` or `tree` | 10x | 2,000 | 400 | -80% |
| `cat` or `read` | 20x | 40,000 | 12,000 | -70% |
| `grep` or `rg` | 8x | 16,000 | 3,200 | -80% |
| `git status` | 10x | 3,000 | 600 | -80% |
| `git diff` | 5x | 10,000 | 2,500 | -75% |
| `cargo test` or `npm test` | 5x | 25,000 | 2,500 | -90% |
| **Total** |  | **~118,000** | **~23,900** | **-80%** |

---

## What Mycelium Owns

- Command-output filtering and compression
- Command-specific display helpers and retained guidance
- Mycelium-specific init, config, and uninstall flows
- Optional routing of large outputs to downstream tools
- Deterministic telemetry summary surfaces built from local tracking aggregates

## What Mycelium Does Not Own

- Long-term memory storage: handled by `hyphae`
- Code intelligence: handled by `rhizome`
- Shared ecosystem onboarding: handled by `stipe`
- Lifecycle capture: handled by `cortina`

---

## Key Features

- Adaptive filtering: changes strategy by output size and command category.
- Token savings: consistently reduce noisy shell output before it reaches the model.
- Hyphae routing: can store large outputs as retrievable chunks.
- Rhizome-backed reads: can summarize large code files structurally instead of dumping raw text.
- Local setup surface: still provides Mycelium-specific init and config commands when needed.

---

## Architecture

```text
mycelium (single binary)
├── src/dispatch.rs   routing shell and fallback orchestration
├── src/parser/       command parsing and dispatch helpers
├── src/gain/         savings and economics output
├── src/init/         setup and uninstall flows
├── src/tracking/     capture and session-related tracking
├── src/vcs/          git-aware filters
├── src/fileops/      file and read helpers
├── src/python/       Python-oriented filters
├── src/js/           JavaScript and Node-oriented filters
└── tests/            fixture and snapshot coverage
```

One package does not mean one file. The router stays intentionally thin, the
command-family modules own their own behavior, and sibling integrations such as
Hyphae and Rhizome remain in their dedicated surfaces instead of being folded
into dispatch.

## Deterministic Telemetry Summary Surface

`mycelium` now names one reusable summary boundary in `src/tracking/`:
the deterministic telemetry summary surface. It is local-first, machine-readable,
and built from the existing tracking and gain aggregates instead of a separate
remote telemetry path.

The ownership split is explicit:

- `cortina` captures normalized edge events
- `mycelium` summarizes them into deterministic telemetry and usage summaries
- `cap` consumes and renders those summaries instead of recomputing them in the UI

Today the stable consumer seam is the gain JSON export, which now includes a
named telemetry summary block derived from the tracking database. Downstream
tools can either consume that JSON surface or call the tracking summary API
directly through the library.

---

## Documentation

- [docs/README.md](docs/README.md): docs index and reading order
- [docs/getting-started/README.md](docs/getting-started/README.md): install and setup path
- [docs/features.md](docs/features.md): feature overview and savings summary
- [docs/commands.md](docs/commands.md): public command reference
- [docs/analytics.md](docs/analytics.md): token savings analytics and hooks
- [docs/architecture.md](docs/architecture.md): technical architecture
- [docs/extending.md](docs/extending.md): adding new commands and filters
- [docs/plugins.md](docs/plugins.md): custom filter plugins
- [docs/troubleshooting.md](docs/troubleshooting.md): common issues
- [docs/roadmap.md](docs/roadmap.md): planned work
- [docs/plans/README.md](docs/plans/README.md): active planning docs

## Development

```bash
cargo build --release
cargo test
cargo clippy
cargo fmt
```

## Logging

Mycelium writes diagnostic logs to stderr through Spore's shared logger.

- Use `MYCELIUM_LOG` for repo-specific logging, for example
  `MYCELIUM_LOG=mycelium=debug mycelium git status`.
- `RUST_LOG` still works as the wider Rust fallback, but `MYCELIUM_LOG` is the
  intended operator knob for this binary.
- Logging is separate from Mycelium's normal product behavior: filtered command
  output still goes to stdout, while passthrough command stderr, warnings, and
  audit-style diagnostics continue to use stderr as part of the CLI surface.

## License

MIT
