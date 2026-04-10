# Mycelium Roadmap

This page is the Mycelium-specific backlog. The workspace [ROADMAP.md](../../docs/workspace/ROADMAP.md) keeps the ecosystem sequencing and cross-repo priorities.

## Recently Shipped

- Mycelium already covers a broad slice of the command surface people actually use. Git, Rust, JavaScript and TypeScript, Python, Go, infrastructure tooling, database clients, and general file and shell workflows all land behind one filtering and rewrite layer.
- The core architecture is in place. Hook-based transparent rewrites, parser-backed output shaping, token formatting, raw-output recovery, JSON envelopes, compact modes, and the shared rewrite command all now work as one system instead of separate utilities.
- Analytics and diagnostics are real product surfaces, not side notes. SQLite-backed tracking, parse-health, doctor, discover, learn, and CC-economics give operators a way to inspect quality and savings instead of trusting a black box.
- Distribution is mature enough for routine use. Mycelium ships cross-platform binaries, release automation, shell completions, and host-neutral onboarding and runtime wording across Claude and Codex.
- Platform convergence is farther along than it used to be. Shared path and shell helpers plus `spore`-backed config registration removed a large chunk of the old Unix-only assumptions.

## Next

### Codebase health

The codebase still needs the planned large-file split and a few more module boundaries. This is not cosmetic work; it keeps the rewrite and parser stack understandable enough to keep changing safely.

### Parser-backed consistency

Older formatter paths and bespoke output shaping still bypass the shared `OutputParser` model in places. Tightening that consistency is the fastest way to make rewrites more predictable and easier to verify.

### Project-scoped analytics

Mycelium should keep extending project-level tracking and push the operator-facing analytics into `cap` instead of growing a separate local dashboard. This item should stay aligned with the ecosystem roadmap because it affects how Cap explains savings and usage.

### Token-aware filtering

The next material quality jump is moving from mostly line and byte caps toward token-budget-aware routing, salience-aware compaction, and better task-shaped compression for debug, review, fix, and status workflows.

### Host-neutral runtime completion

Residual platform-specific and Unix-shaped behavior still shows up in less common command paths. The near-term goal is to finish the portability pass so rewrite, proxy, config discovery, and analytics collection behave the same way across hosts and operating systems.

## Later

### Plugin and customization layer

Mycelium can support user-defined filters and experimental adapters, but that layer should remain a fallback for extension. Built-in integrations still need to own the common paths.

### Selective tool expansion

Some commands should stay alias-backed until real usage proves otherwise. `rg`, `bun`, `bunx`, `podman`, and `diffsitter` only need first-class handling if their raw semantics diverge enough that the current rewrite path becomes misleading.

### Richer diagnostics

Better error messages, explain mode, and stronger rewrite-quality scoring belong here once the base parser consistency work is farther along. Those features are most useful when the underlying decision path is already stable.

### Local summarization and retrieval

A lightweight local reranker or summarizer plus richer chunk metadata may become worthwhile for very large outputs. That work should wait until the outer-loop token optimization path proves where the heuristics still fall short.

## Research

### Local model runtime support

If Mycelium ever becomes a local model runtime, the roadmap changes. KV-cache work, dynamic layer execution, and deeper runtime optimization only matter under that condition, not before it.

### Compression beyond heuristics

Heuristic compression has a lot of runway left. The open question is when a local summarizer or reranker starts paying for itself enough to justify the extra complexity in the filtering path.

## Not Planned

- A separate long-term analytics UI inside Mycelium: Cap is the operator surface for the ecosystem.
- First-class built-in coverage for every alias-backed tool before output semantics justify it: broad coverage is useful, but semantic fidelity matters more than command count.
- Model training, fine-tuning, or multimodal token-compression work under the current product shape: that only becomes relevant if Mycelium turns into a local inference runtime.
