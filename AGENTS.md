# Mycelium Agent Notes

## Purpose

Mycelium owns token optimization for shell output. Work here should keep the router thin, keep command-family behavior in the owning modules, and keep sibling integrations isolated behind adapters. If a change starts looking like memory, code intelligence, or shared install policy, it probably belongs in `hyphae`, `rhizome`, or `stipe` instead.

---

## Source of Truth

- `src/dispatch.rs`: routing backbone and fallback behavior.
- `src/vcs/`, `src/cargo_filters/`, `src/js/`, `src/python/`, `src/fileops/`, `src/container_cmd/`: command-family behavior.
- `src/gain/` and `src/tracking/`: savings metrics, persistence, and local analytics.
- `src/init/`: Mycelium-owned setup and guidance injection.
- `tests/` and `src/snapshots/`: output and behavior expectations.
- `../septa/`: authoritative schema and fixture for outbound Hyphae payloads.
- `../ecosystem-versions.toml`: shared dependency pins.

If Mycelium code and a shared contract disagree, update `../septa/` first.

---

## Before You Start

Before writing code, verify:

1. **Owning module**: keep routing in `dispatch`, command logic in the command family, and integrations in their adapters.
2. **Contracts**: if Hyphae payload shape changes, read `../septa/README.md` first.
3. **Versions**: check `../ecosystem-versions.toml` before changing shared dependencies.
4. **Validation target**: decide whether the change needs snapshots, focused tests, ignored integration tests, or all of them.
5. **Docs impact**: if command behavior changes, update the relevant page under `docs/`.

---

## Preferred Commands

Use these for most work:

```bash
cargo build --release
cargo test
```

For targeted work:

```bash
cargo test <module_or_test_name>
cargo test --ignored
cargo clippy
cargo fmt --check
cargo insta review
```

---

## Repo Architecture

Mycelium is healthiest when command routing, filter behavior, analytics, and integrations stay in separate layers.

Key boundaries:

- `src/dispatch.rs`: route and fall back; do not let it absorb domain policy.
- command-family modules: own command-specific filtering and summaries.
- `src/tracking/` and `src/gain/`: own metrics and local persistence.
- `src/init/`: own Mycelium-specific setup and uninstall behavior.
- sibling integrations: improve specific flows, but Mycelium still works without them.

Current direction:

- Keep passthrough safe when filtering fails or a command is unsupported.
- Keep output-shaping behavior snapshot-tested.
- Keep large-output routing explicit through owned adapters instead of leaking sibling-tool logic into command filters.

---

## Working Rules

- Do not change command semantics; only change what reaches the model.
- Keep `dispatch` small. If behavior is command-specific, move it into the owning module.
- Prefer real fixtures and snapshots over hand-wavy output examples.
- When changing tracked savings behavior, check metrics and persistence, not just display text.
- When changing setup or uninstall behavior, keep Mycelium-specific flows separate from shared Stipe policy.
- Update docs when a public command, setup path, or troubleshooting path changes.
- Validate septa contracts after changing any cross-project payload: `cd septa && bash validate-all.sh`

---

## Multi-Agent Patterns

For substantial Mycelium work, default to two agents:

**1. Primary implementation worker**
- Owns the touched command family, adapter, or helper layer
- Keeps the write scope inside Mycelium unless a real contract change requires `../septa/`

**2. Independent validator**
- Reviews the broader shape instead of redoing the implementation
- Specifically looks for dispatch bloat, contract drift, snapshot gaps, fallback regressions, and setup-policy leakage from `stipe`

Add a docs worker when `README.md`, `CLAUDE.md`, `AGENTS.md`, or public docs changed materially.

---

## Skills to Load

Use these for most work in this repo:

- `basidiocarp-rust-repos`: repo-local Rust workflow and validation habits
- `systematic-debugging`: before fixing unexplained test failures or behavior drift
- `writing-voice`: when touching README or docs prose

Use these when the task needs them:

- `test-writing`: when behavior changes need stronger coverage
- `tool-preferences`: when exploring the codebase and keeping reads tight matters
- `basidiocarp-workspace-router`: when the change may spill into `septa`, `stipe`, `hyphae`, or `rhizome`

---

## Done Means

A task is not complete until:

- [ ] The change is in the right module or adapter layer
- [ ] The narrowest relevant validation has run, when practical
- [ ] Related docs, snapshots, or contract files are updated if they should move together
- [ ] Any skipped validation or follow-up work is stated clearly in the final response

If validation was skipped, say so clearly and explain why.
