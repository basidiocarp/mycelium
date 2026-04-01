# Mycelium

Token-optimized CLI proxy. Filters and compresses command output before it reaches your LLM context. Single Rust binary with no external runtime dependencies, typically ~5-15ms proxy overhead. 60-90% token savings on common developer workflows.

Part of the [Basidiocarp ecosystem](https://github.com/basidiocarp) — see the [Technical Overview](https://github.com/basidiocarp/.github/blob/main/profile/README.md#technical-overview) for how Mycelium fits with Hyphae, Rhizome, Cap, and Lamella.

`stipe` owns ecosystem onboarding, shared repair, and MCP registration. Mycelium owns token optimization plus its retained Claude Code guidance, hook adapter, config display, and uninstall flows.

## The Ecosystem

- **mycelium** — Filters and compresses command output (this project). See [Token Optimization](https://github.com/basidiocarp/.github/blob/main/profile/README.md#token-optimization--mycelium).
- **[hyphae](https://github.com/basidiocarp/hyphae)** — Persistent memory with [RAG pipeline](https://github.com/basidiocarp/.github/blob/main/profile/README.md#retrieval-augmented-generation-rag--hyphae--lamella), [vector search](https://github.com/basidiocarp/.github/blob/main/profile/README.md#vector-database--hybrid-search--hyphae), and [feedback loop](https://github.com/basidiocarp/.github/blob/main/profile/README.md#feedback-loop--lesson-extraction--hyphae--lamella).
- **[rhizome](https://github.com/basidiocarp/rhizome)** — Code intelligence with [tree-sitter](https://github.com/basidiocarp/.github/blob/main/profile/README.md#tree-sitter-code-parsing--rhizome) and [LSP auto-management](https://github.com/basidiocarp/.github/blob/main/profile/README.md#lsp-auto-management--rhizome).
- **[cap](https://github.com/basidiocarp/cap)** — Web dashboard for memory browsing, token analytics, and code exploration.
- **[lamella](https://github.com/basidiocarp/lamella)** — Skills, hooks, and [feedback capture](https://github.com/basidiocarp/.github/blob/main/profile/README.md#feedback-loop--lesson-extraction--hyphae--lamella) for Claude Code.

## Savings (30-min Claude Code Session)

| Operation                 | Frequency | Standard     | mycelium    | Savings  |
|---------------------------|-----------|--------------|-------------|----------|
| `ls` / `tree`             | 10x       | 2,000        | 400         | -80%     |
| `cat` / `read`            | 20x       | 40,000       | 12,000      | -70%     |
| `grep` / `rg`             | 8x        | 16,000       | 3,200       | -80%     |
| `git status`              | 10x       | 3,000        | 600         | -80%     |
| `git diff`                | 5x        | 10,000       | 2,500       | -75%     |
| `git log`                 | 5x        | 2,500        | 500         | -80%     |
| `git add/commit/push`     | 8x        | 1,600        | 120         | -92%     |
| `cargo test` / `npm test` | 5x        | 25,000       | 2,500       | -90%     |
| `ruff check`              | 3x        | 3,000        | 600         | -80%     |
| `pytest`                  | 4x        | 8,000        | 800         | -90%     |
| `go test`                 | 3x        | 6,000        | 600         | -90%     |
| `docker ps`               | 3x        | 900          | 180         | -80%     |
| **Total**                 |           | **~118,000** | **~23,900** | **-80%** |

## Installation

```bash
# Quick install (all ecosystem tools)
curl -fsSL https://raw.githubusercontent.com/basidiocarp/.github/main/install.sh | sh

# Onboarding, shared repair, and MCP setup
stipe init

# Install Mycelium
cargo install --git https://github.com/basidiocarp/mycelium
```

## How It Works

```mermaid
flowchart LR
    subgraph Without["Without mycelium"]
        C1["Claude"] -->|"git status"| S1["shell"] --> G1["git"]
        G1 -->|"~2,000 tokens (raw)"| C1
    end

    subgraph With["With mycelium"]
        C2["Claude"] -->|"git status"| M["Mycelium"] --> G2["git"]
        G2 --> M
        M -->|"~200 tokens (filtered)"| C2
    end
```

### Core Filtering and Routing

1. **Smart filtering** — Removes noise (comments, whitespace, boilerplate)
2. **Grouping** — Aggregates similar items (files by directory, errors by type)
3. **Truncation** — Keeps relevant context, cuts redundancy
4. **Deduplication** — Collapses repeated log lines with counts
5. **Adaptive sizing** — Small (<50 lines) pass through, medium get filtered, large (>500 lines) get full compression

Optional integrations:
- **Hyphae routing** — Large outputs stored as retrievable chunks in Hyphae (when installed)
- **Rhizome code intelligence** — `mycelium read` uses tree-sitter structural outlines for large code files (when installed)

### Mycelium Setup Surface

Use `stipe init` for ecosystem setup, onboarding, shared repair, and client registration.

Mycelium still provides Mycelium-specific init and inspection modes when you need them:
- `mycelium init -g` — install or repair the Claude Code hook adapter, `MYCELIUM.md`, and settings patch on supported platforms
- `mycelium init -g --claude-md` — install global Claude Code docs-only guidance without hook setup
- `mycelium init --claude-md` — write project-local Claude Code guidance only
- `mycelium config` — inspect current Mycelium configuration
- `mycelium init -g --uninstall` — remove Mycelium-managed setup

Older setup flags such as `mycelium init --ecosystem`, `mycelium init --onboard`, and `mycelium init --client ...` have been removed. Use `stipe init` or `stipe init --client ...` instead.

When installed, Mycelium can work with Hyphae and Rhizome for:
- Hyphae-backed storage of large outputs and capture data
- Rhizome-powered structural reads for large code files
- Claude Code guidance files such as `MYCELIUM.md` and `CLAUDE.md` references where supported

## Documentation

- [FEATURES.md](docs/FEATURES.md) — Feature overview and savings summary
- [COMMANDS.md](docs/COMMANDS.md) — Public command reference
- [ANALYTICS.md](docs/ANALYTICS.md) — Token savings analytics and hooks
- [ARCHITECTURE.md](docs/ARCHITECTURE.md) — Technical architecture
- [EXTENDING.md](docs/EXTENDING.md) — Adding new commands
- [PLUGINS.md](docs/PLUGINS.md) — Custom filter plugins
- [TROUBLESHOOTING.md](docs/TROUBLESHOOTING.md) — Common issues

## License

MIT
