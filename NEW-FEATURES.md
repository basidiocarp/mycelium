# New Features and Pitfalls from RTK Issues

Snapshot date: March 23, 2026

I reviewed the current RTK issue tracker via GitHub API and filtered out pull requests. The snapshot I used contained 143 open issues and 143 closed issues in `rtk-ai/rtk`.

This document is not a line-by-line porting plan. It is a short list of:

- RTK ideas that look worth stealing for Mycelium
- RTK failure modes that Mycelium already avoids
- RTK failure modes that Mycelium may still share

## High-value features worth borrowing

### 1. Task runner visibility

RTK issue: [#607](https://github.com/rtk-ai/rtk/issues/607)

Why it matters:

- Task runners like `mise`, `just`, and `task` hide the underlying tool from the rewrite hook.
- That means the system sees `mise run lint` instead of `biome`, `cargo`, `pytest`, or `pnpm`.

Why this is useful for Mycelium:

- Mycelium has strong direct command coverage, but I do not see task-runner-aware rewrite rules in the current tree.
- This is a real leverage feature because it expands effective support without adding one-off wrappers for every team convention.

Recommendation:

- Add a lightweight task-runner expansion layer before normal rewrite classification.
- Start with `mise`, `just`, and `task`.

### 2. Expose Mycelium as a library crate

RTK issue: [#758](https://github.com/rtk-ai/rtk/issues/758)

Why it matters:

- Mycelium already has useful reusable pieces: command classification, filters, tracking helpers, JSON summaries, and parser logic.
- A library target would make it easier to reuse Mycelium logic in `stipe`, `cap`, and future agent tooling without shelling out.

Recommendation:

- Add a curated `lib.rs` surface instead of exposing the whole crate.
- Start with classification, diff/status filters, test parsers, tracking path helpers, and JSON output helpers.

### 3. Configurable reduction profiles

RTK issue: [#488](https://github.com/rtk-ai/rtk/issues/488)

Why it matters:

- Debugging and review need different compression levels.
- Mycelium already has configurable adaptive thresholds in [docs/ARCHITECTURE.md](./docs/ARCHITECTURE.md), but not a simple user-facing profile model.

Relevant current state:

- Adaptive thresholds are documented in [docs/ARCHITECTURE.md](./docs/ARCHITECTURE.md) at lines 291-295.
- Test passthrough truncation is already configurable in [docs/INTERNALS.md](./docs/INTERNALS.md) at lines 328-348.

Recommendation:

- Add named profiles such as `debug`, `balanced`, and `aggressive`.
- Let the profile tune diff budgets, passthrough truncation, and whether success output is summarized or preserved.

### 4. Expand generic command coverage where it matters most

RTK issues:

- [#784](https://github.com/rtk-ai/rtk/issues/784) `python3` support
- [#783](https://github.com/rtk-ai/rtk/issues/783) `ssh` support

My read:

- Mycelium already supports Python tooling patterns like `python3 -m pytest`, `python3 -m mypy`, and `python3 -m pip` in [src/discover/rules.rs](./src/discover/rules.rs) at lines 41-45 and 354-385.
- I do not see generic `python3 script.py` handling or an `ssh` command module in the current rewrite rules.

Recommendation:

- Add generic `python3` script filtering only if you can preserve tracebacks and printed structure without becoming a fake “Python interpreter wrapper.”
- `ssh` is a bigger win than generic `python3` if your workflow involves remote logs, `docker logs`, `journalctl`, or `kubectl` over SSH.

## RTK pitfalls that Mycelium already addresses better

### 1. Unsupported commands falling off a cliff

RTK issue family:

- [#600](https://github.com/rtk-ai/rtk/issues/600)
- [#286](https://github.com/rtk-ai/rtk/issues/286)

Mycelium already has the right design:

- Unsupported commands pass through unchanged and are still recorded, as documented in [docs/FEATURES.md](./docs/FEATURES.md) lines 41-43.

This is the right default. Keep it.

### 2. Parser fallback that is too destructive

RTK issue family:

- [#620](https://github.com/rtk-ai/rtk/issues/620)
- [#690](https://github.com/rtk-ai/rtk/issues/690)

Mycelium has already moved in the right direction:

- Test-runner passthrough was increased from 500 to 4000 chars in [docs/INTERNALS.md](./docs/INTERNALS.md) lines 328-348.
- The Playwright parser explicitly falls back to `truncate_output(input, 4000)` in [src/js/playwright.rs](./src/js/playwright.rs) lines 107-116.
- Playwright runs also inject `--reporter=json` when appropriate in [src/js/playwright.rs](./src/js/playwright.rs) lines 282-295.

This is materially better than RTK's old 500-char fallback behavior.

### 3. JSON and structured-output passthrough for GitHub CLI

RTK issue family:

- [#313](https://github.com/rtk-ai/rtk/issues/313)
- [#311](https://github.com/rtk-ai/rtk/issues/311)

Mycelium already has an explicit guard:

- `gh` commands with `--json`, `--jq`, or `--template` skip rewrite, documented in [docs/INTERNALS.md](./docs/INTERNALS.md) lines 169-173.

That is a good protection against filtering data that is meant for machine consumption.

## Pitfalls Mycelium may still share

These are inferences from the current Mycelium code, not confirmed bugs.

### 1. Hardcoded diff and status caps can still hide important context

RTK issue family:

- [#621](https://github.com/rtk-ai/rtk/issues/621)
- [#618](https://github.com/rtk-ai/rtk/issues/618)

Why I think Mycelium is still exposed:

- `git diff` still hardcodes `max_hunk_lines = 100` in [src/vcs/git_filters/diff.rs](./src/vcs/git_filters/diff.rs) lines 7-15 and truncates once a hunk hits the cap at lines 54-56.
- `git status` still has a total 50-file budget in [src/vcs/git_filters/status.rs](./src/vcs/git_filters/status.rs) lines 61-91.

This is much less aggressive than RTK's old limits, but it is still a fixed budget. Large refactors, vendored deletions, or big renames can still produce an incomplete picture.

### 2. Hook PATH fragility still exists

RTK issue: [#685](https://github.com/rtk-ai/rtk/issues/685)

Why I think Mycelium is still exposed:

- The installed hook exits early if `command -v mycelium` or `jq` fails in [hooks/mycelium-rewrite.sh](./hooks/mycelium-rewrite.sh) lines 26-29.
- Mycelium does have better diagnostics and integrity checks in [src/init/mod.rs](./src/init/mod.rs) lines 132-168 and 236-248.

So this is better than RTK, but the underlying dependency on hook-time PATH resolution is still there.

Best next fix:

- Install the hook with an absolute binary path or a tiny launcher that resolves to the install location recorded during `init`.

### 3. GitHub issue/PR special modes deserve explicit regression tests

RTK issue family:

- [#730](https://github.com/rtk-ai/rtk/issues/730)
- [#720](https://github.com/rtk-ai/rtk/issues/720)

Why I think Mycelium is worth auditing here:

- `gh pr view` already has a passthrough guard for `--json`, `--jq`, and `--web` in [src/vcs/gh_pr/view.rs](./src/vcs/gh_pr/view.rs) lines 11-29.
- `gh issue view` currently hardcodes a JSON field set of `number,title,state,author,body,url` and then appends extra args in [src/vcs/gh_cmd/issue.rs](./src/vcs/gh_cmd/issue.rs) lines 95-105.

That means `gh issue view --comments` and other presentation-changing flags are worth explicit testing. This is exactly the kind of mode-sensitive bug RTK hit repeatedly.

### 4. Database fragmentation across environments is still possible

RTK issue: [#699](https://github.com/rtk-ai/rtk/issues/699)

Why I think Mycelium is only partially protected:

- Mycelium supports override paths via `MYCELIUM_DB_PATH` and config in [src/tracking/utils.rs](./src/tracking/utils.rs) lines 22-45.
- The default still comes from `dirs::data_local_dir()`, which can vary across host environments, containers, snaps, or editor-integrated terminals.

This is not necessarily a bug, but it is the same class of issue.

Best next fix:

- Add a `mycelium doctor` or `mycelium tracking status` check that warns when multiple DB locations are likely.

### 5. Git commit signing should be tested explicitly

RTK issue: [#733](https://github.com/rtk-ai/rtk/issues/733)

Why I think Mycelium should audit this:

- `mycelium git commit` shells through to `git commit` in [src/vcs/git/mutations.rs](./src/vcs/git/mutations.rs) lines 6-26.
- I do see tests for `-am` and `--amend` in [src/vcs/git/mutations.rs](./src/vcs/git/mutations.rs) lines 305-329.
- I do not see signing-specific tests in the current commit proxy path.

This may already work, but it is worth adding a regression test for `git commit -S` and SSH signing.

## What I would do next

### Highest leverage

1. Add task-runner support for `mise`, `just`, and `task`.
2. Add user-facing compaction profiles: `debug`, `balanced`, `aggressive`.
3. Add regression tests for:
   - `gh issue view --comments`
   - `gh pr diff --name-only`
   - very large `git status`
   - very large `git diff`
   - signed `git commit -S`
   - hook execution when `mycelium` is not on restricted PATH

### Good second wave

1. Expose a small library API.
2. Add generic `ssh` support.
3. Add better DB-path introspection and warnings.

## Bottom line

The main RTK lesson is not “support more commands.” It is “do not hide the exact signal the agent needs when a command enters an important edge mode.”

Mycelium is already better than RTK in a few important places:

- unsupported-command passthrough
- larger test-runner fallback windows
- explicit `gh --json/--jq/--template` passthrough
- hook integrity and settings checks

The biggest remaining risk areas are:

- fixed truncation budgets for Git-heavy workflows
- hook PATH fragility
- mode-sensitive GitHub CLI behavior
- untested signing and multi-environment tracking edge cases
