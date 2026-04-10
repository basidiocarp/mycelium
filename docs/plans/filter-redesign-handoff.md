# Filter Redesign Handoff

Complete the filter redesign handoff at `.handoffs/mycelium/filter-redesign.md`. Steps 1-4 are already implemented. Only Step 5 remains, plus running the verification script.

## Current State

Steps 1-4 were implemented in mycelium v0.8.5-0.8.6:

- **Step 1 (DONE):** `classify_by_tokens()` in `src/adaptive.rs` — token-based routing with 500/2000 thresholds
- **Step 2 (DONE):** `FilterQuality` enum and `FilterResult` struct in `src/filter.rs` — but `FilterQuality` and `FilterResult` are `#[allow(dead_code)]` because no filters use them yet
- **Step 3 (DONE):** `validate_filter_output()` in `src/hyphae.rs` — 3 validation rules (no empty, >20% savings, <95% aggressive)
- **Step 4 (DONE):** `add_filter_header()` in `src/hyphae.rs` + `show_filter_header` config option

## What Remains

### Step 5: Migrate key filters to return `FilterQuality`

The `FilterStrategy` trait has `filter_with_quality()` with a default implementation that wraps the old `filter()` and returns `FilterQuality::Full`. Individual filters need to override this to return meaningful quality signals.

**Priority filters to migrate:**
1. `src/filters/gh.rs` (or wherever gh filter lives) — caused the most friction
2. Git filters (`src/vcs/git/` or similar) — git log, status, diff, show
3. Cargo filters (`src/cargo_cmd.rs` or similar) — cargo test, build, clippy

For each filter:
- Override `filter_with_quality()` instead of just `filter()`
- Return `FilterQuality::Full` when the filter fully understood the output format
- Return `FilterQuality::Degraded` when partial match (some lines passed through raw)
- Return `FilterQuality::Passthrough` when format not recognized
- Include `input_tokens` and `output_tokens` in `FilterResult`

Then update the routing in `src/hyphae.rs` to use `filter_with_quality()` and the quality signal:
- `Degraded` quality with <40% savings → fall back to raw (Rule 3 in the handoff)
- Pass `FilterQuality` through to the header for richer transparency

### After Step 5: Run verification

```bash
bash .handoffs/mycelium/verify-filter-redesign.sh
```

The verification script checks all 5 steps. Paste the output into the handoff document between the `PASTE START`/`PASTE END` markers for each step.

Then run:
```bash
cargo test
cargo clippy
```

## Key Files to Read First

Read these to understand the current architecture:

1. `src/filter.rs` — `FilterStrategy` trait, `FilterQuality`, `FilterResult` (the types to use)
2. `src/hyphae.rs` — `route_or_filter()` (the routing layer), `validate_filter_output()`, `add_filter_header()`
3. `src/adaptive.rs` — `classify_by_tokens()`, `classify_with_tuning()`, `AdaptiveLevel`
4. `src/config.rs` — `CompactionTuning`, `FilterConfig` (token thresholds and config)
5. `src/main.rs` or `src/lib.rs` — find where filters are registered and dispatched to understand the filter module layout

Then find the actual filter implementations:
- Search for `impl FilterStrategy` to find all filters
- Focus on gh, git, and cargo filters first

## What NOT to Change

- Do NOT change token thresholds (500/2000) — these are tuned
- Do NOT change `validate_filter_output` rules — these are tested
- Do NOT change the `add_filter_header` format — agents already expect it
- Do NOT break backward compatibility — `filter()` must still work for unmigrated filters
- The default `filter_with_quality()` returns `FilterQuality::Full` — only override for filters where you add actual quality detection

## Definition of Done

1. `FilterQuality` and `FilterResult` are no longer `#[allow(dead_code)]`
2. At least gh, git log, and cargo test filters return meaningful `FilterQuality`
3. The routing layer uses `filter_with_quality()` and respects quality signals
4. `cargo test` passes
5. `cargo clippy` clean
6. `bash .handoffs/mycelium/verify-filter-redesign.sh` passes
7. Verification output pasted into the handoff document
8. Update `.handoffs/HANDOFFS.md` — move Filter Redesign to Completed
