# New Features Implementation Plan

This plan turns the findings in [NEW-FEATURES.md](./NEW-FEATURES.md) into a small, opportunistic backlog for a solo maintainer.

It is intentionally biased toward:

- high leverage
- low coordination overhead
- clear verification
- work that compounds future command support

## Priority Order

### P1. Protect core signal

Goal:

- reduce the chance that Mycelium hides the exact output an agent needs during debugging, review, and repair

Tasks:

1. Audit and tune Git truncation budgets
2. Add regression tests for GitHub CLI edge modes
3. Add signed-commit regression coverage
4. Improve hook PATH resilience

Why first:

- These are the easiest ways to regress trust in the tool
- They map directly to RTK’s most painful issue themes

Verification:

- large `git diff` fixtures preserve more actionable lines without emptying the summary
- large `git status` fixtures preserve enough filenames for commit/review workflows
- `gh issue view --comments` and `gh pr diff --name-only` have explicit tests
- `git commit -S` argument flow is covered by tests
- hook diagnostics clearly explain PATH failures

### P2. Expand effective coverage

Goal:

- make Mycelium work in more real-world workflows without requiring users to abandon their current command habits

Tasks:

1. Add task-runner support for `mise`
2. Add task-runner support for `just`
3. Add task-runner support for `task`
4. Evaluate generic `ssh` support

Why second:

- This expands usable command coverage faster than adding many one-off wrappers

Verification:

- rewrite classification works for representative `mise run lint`, `just test`, and `task build` patterns
- passthrough remains safe when a task definition cannot be resolved
- task-runner commands still record sensible tracking metadata

### P3. Make behavior user-tunable

Goal:

- let users pick the amount of compression they want for debugging versus speed

Tasks:

1. Add named compaction profiles
2. Wire profiles into diff/status/test fallback budgets
3. Document how profiles affect behavior

Why third:

- the underlying filters are already here
- profiles mostly improve ergonomics and reduce future issue churn

Verification:

- profile selection changes effective budgets in tests
- docs show which knobs move under each profile

### P4. Improve reuse and introspection

Goal:

- make Mycelium easier to embed elsewhere in the ecosystem and easier to diagnose in multi-environment setups

Tasks:

1. Add a small library surface
2. Add DB-path introspection / tracking status checks

Verification:

- a minimal `lib.rs` exposes curated modules cleanly
- `tracking status` or equivalent reports the active DB path and config source

## Issue Candidates

These are written so they can be copied into GitHub with minor editing.

### Issue 1. Audit Git truncation budgets for agent workflows

Problem:

- Mycelium still uses fixed budgets for Git-heavy views like diff and status.
- This is safer than RTK’s older defaults, but large refactors can still hide important context.

Current references:

- [src/vcs/git_filters/diff.rs](./src/vcs/git_filters/diff.rs)
- [src/vcs/git_filters/status.rs](./src/vcs/git_filters/status.rs)

Scope:

- review current limits
- add fixtures for large refactors and many-file working trees
- tune defaults or make them profile-aware

Done when:

- large diff/status fixtures no longer lose critical review/debugging context
- tests lock the new behavior down

### Issue 2. Add regression tests for `gh` edge modes

Problem:

- RTK repeatedly hit bugs where special `gh` flags changed output shape and the wrong filter still ran.
- Mycelium already handles some of this, but the risky modes need explicit coverage.

Current references:

- [src/vcs/gh_pr/view.rs](./src/vcs/gh_pr/view.rs)
- [src/vcs/gh_cmd/issue.rs](./src/vcs/gh_cmd/issue.rs)
- [docs/INTERNALS.md](./docs/INTERNALS.md)

Scope:

- add tests for `gh issue view --comments`
- add tests for `gh pr diff --name-only`
- confirm `--json`, `--jq`, and `--template` still bypass rewrite/filtering correctly

Done when:

- those modes are covered by tests and no longer rely on assumptions

### Issue 3. Make hook PATH failures explicit and recoverable

Problem:

- the rewrite hook currently exits quietly when `mycelium` or `jq` is not found
- verification commands are better now, but the runtime failure mode is still easy to miss

Current references:

- [hooks/mycelium-rewrite.sh](./hooks/mycelium-rewrite.sh)
- [src/init/hook.rs](./src/init/hook.rs)
- [src/init/mod.rs](./src/init/mod.rs)

Scope:

- decide whether to use an absolute Mycelium path at install time or a generated launcher
- improve hook audit logging for dependency failures
- surface clearer repair guidance in verification output

Done when:

- Homebrew/nonstandard PATH installs are less fragile
- users can tell why rewriting is not happening

### Issue 4. Add signed-commit regression coverage

Problem:

- RTK had a real failure mode around proxied `git commit` and SSH signing
- Mycelium shells through correctly for common commit flags, but signing is not explicitly covered

Current references:

- [src/vcs/git/mutations.rs](./src/vcs/git/mutations.rs)

Scope:

- add tests for `git commit -S`
- add tests for combinations like `git commit -S -m ...` and `git commit --amend -S`

Done when:

- commit proxy argument preservation for signing is locked down by tests

### Issue 5. Add task-runner-aware rewrite support

Problem:

- command rewriting misses workflows hidden behind `mise`, `just`, and `task`

Current references:

- [src/discover/registry.rs](./src/discover/registry.rs)
- [src/discover/rules.rs](./src/discover/rules.rs)

Scope:

- add a preprocessing stage for task-runner commands
- resolve simple task names to underlying commands where possible
- keep passthrough as the fallback when resolution is ambiguous

Done when:

- representative task-runner commands rewrite into existing Mycelium flows
- ambiguous tasks fail safe and still execute raw

### Issue 6. Add compaction profiles

Problem:

- current tuning exists, but the user-facing model is still mostly raw thresholds and special cases

Current references:

- [docs/ARCHITECTURE.md](./docs/ARCHITECTURE.md)
- [docs/INTERNALS.md](./docs/INTERNALS.md)

Scope:

- add `debug`, `balanced`, and `aggressive` profiles
- map profiles onto diff/status/test fallback thresholds
- update docs and config examples

Done when:

- users can switch behavior without hand-editing multiple low-level settings

### Issue 7. Expose a curated library API

Problem:

- other Basidiocarp tools may want Mycelium’s filters and classification logic without shelling out

Scope:

- add `lib.rs`
- expose only stable, useful modules
- avoid freezing internals that still need freedom to move

Done when:

- a downstream crate can depend on Mycelium for classification/filtering helpers

### Issue 8. Add tracking DB introspection

Problem:

- different environments can still end up writing to different tracking DBs

Current references:

- [src/tracking/utils.rs](./src/tracking/utils.rs)

Scope:

- add a command to print the resolved DB path and source of truth
- optionally warn when environment/config overrides are active

Done when:

- users can quickly see where Mycelium is writing analytics data

## Recommended Implementation Sequence

If I were doing this in order, I would take:

1. Issue 2: `gh` edge-mode regression tests
2. Issue 4: signed-commit regression coverage
3. Issue 1: Git truncation audit
4. Issue 3: hook PATH resilience
5. Issue 5: task-runner support
6. Issue 6: compaction profiles
7. Issue 8: tracking DB introspection
8. Issue 7: curated library API

That order keeps the early work defensive and low-risk, then moves into expansion and architecture.

## Smallest Good Next Step

If you want the highest signal-to-effort move first:

- implement Issue 2 and Issue 4 together

Why:

- both are test-heavy
- both reduce “silent wrong behavior”
- neither requires a major architecture decision
