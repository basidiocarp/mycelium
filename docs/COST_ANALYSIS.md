# Cost Analysis

How mycelium measures token savings and estimates dollar-value impact.

## Table of Contents

- [Token Estimation](#token-estimation)
- [Per-Command Savings](#per-command-savings)
- [Aggregate Metrics](#aggregate-metrics)
- [Weighted Economics](#weighted-economics)
- [Legacy Metrics](#legacy-metrics)
- [Quota Analysis](#quota-analysis)
- [Accuracy and Limitations](#accuracy-and-limitations)
- [Reference Tables](#reference-tables)

## Token Estimation

Every command execution measures two values: the raw output (what the shell would have returned) and the filtered output (what mycelium returns to the LLM).

Both are converted to token counts using a character-based heuristic:

```
tokens = ceil(characters / 4)
```

This approximation runs in constant time with zero dependencies. It uses byte length (`str::len()`), not Unicode scalar count.

**Source:** `src/tracking/utils.rs` — `estimate_tokens()`

### Why 4?

The ratio of ~4 characters per token holds for English prose with most LLM tokenizers (BPE-based). It's a deliberate trade-off: fast and good enough for relative comparisons, not precise enough for billing reconciliation.

## Per-Command Savings

When a command executes, mycelium records savings before writing to the database:

```
saved_tokens  = max(0, input_tokens - output_tokens)
savings_pct   = (saved_tokens / input_tokens) * 100
```

`saturating_sub` prevents underflow if filtered output somehow exceeds raw output (edge case — passthrough commands, encoding differences).

**Source:** `src/tracking/mod.rs` — `Tracker::record()`

### What Gets Recorded

Each execution writes one row to SQLite with:

| Column | Type | Description |
|--------|------|-------------|
| `input_tokens` | INTEGER | Estimated tokens of raw command output |
| `output_tokens` | INTEGER | Estimated tokens of filtered output |
| `saved_tokens` | INTEGER | `input - output` (pre-computed) |
| `savings_pct` | REAL | `(saved / input) * 100` (pre-computed) |
| `exec_time_ms` | INTEGER | Wall-clock execution time |
| `parse_tier` | INTEGER | 1=Full, 2=Degraded, 3=Passthrough |

Passthrough commands (where mycelium streams output without filtering) record `input_tokens=0, output_tokens=0` to avoid diluting savings statistics.

## Aggregate Metrics

`mycelium gain` aggregates across all recorded commands using SQL:

```sql
SELECT
    COUNT(*)           as commands,
    SUM(input_tokens)  as total_input,
    SUM(output_tokens) as total_output,
    SUM(saved_tokens)  as total_saved,
    SUM(exec_time_ms)  as total_time
FROM commands
```

The efficiency percentage displayed in the summary is:

```
efficiency = (total_saved / total_input) * 100
```

This is a weighted average — commands that process more tokens contribute proportionally more to the aggregate number. It is not the arithmetic mean of individual savings percentages.

**Source:** `src/tracking/queries.rs` — `get_summary_filtered()`

### Grouping

The same aggregation runs at multiple granularities:

| Granularity | SQL GROUP BY | Flag |
|-------------|-------------|------|
| Daily | `DATE(timestamp)` | `--daily` |
| Weekly | `DATE(timestamp, 'weekday 0', '-6 days')` | `--weekly` |
| Monthly | `strftime('%Y-%m', timestamp)` | `--monthly` |
| By command | `mycelium_cmd` | default view |

Weekly grouping uses SQLite's weekday function where week 0 = Sunday, offset by -6 days to produce Monday-start weeks.

## Weighted Economics

When Claude Code usage data is available (via `ccusage`), mycelium computes dollar-value savings by deriving a cost-per-token that accounts for the actual pricing structure.

### Anthropic API Pricing Weights

Different token types have different costs relative to input tokens:

| Token Type | Weight | Meaning |
|------------|--------|---------|
| Input | 1.0x | Baseline |
| Output | 5.0x | 5x more expensive than input |
| Cache write | 1.25x | 1.25x input price |
| Cache read | 0.1x | 10x cheaper than input |

**Source:** `src/cc_economics/models.rs` — `WEIGHT_OUTPUT`, `WEIGHT_CACHE_CREATE`, `WEIGHT_CACHE_READ`

### Weighted Input CPT (Primary Metric)

To derive a meaningful cost-per-token, mycelium converts all token types into "input-equivalent units":

```
weighted_units = input_tokens
               + 5.0  * output_tokens
               + 1.25 * cache_create_tokens
               + 0.1  * cache_read_tokens
```

Then:

```
weighted_input_cpt = total_api_cost / weighted_units
```

This gives the effective cost of one input-equivalent token, accounting for the actual mix of token types in your Claude Code usage.

### Dollar Savings

Mycelium's saved tokens are input tokens (raw command output that the LLM would have consumed). The dollar value of those savings:

```
savings_dollars = saved_tokens * weighted_input_cpt
```

**Source:** `src/cc_economics/models.rs` — `compute_weighted_metrics()`

### Worked Example

Given a Claude Code session:
- API cost: $2.40
- Input tokens: 100,000
- Output tokens: 20,000
- Cache write tokens: 50,000
- Cache read tokens: 300,000

Step 1 — Weighted units:
```
100,000 + 5(20,000) + 1.25(50,000) + 0.1(300,000)
= 100,000 + 100,000 + 62,500 + 30,000
= 292,500
```

Step 2 — Cost per input-equivalent token:
```
$2.40 / 292,500 = $0.0000082 per token
```

Step 3 — If mycelium saved 80,000 tokens:
```
80,000 * $0.0000082 = $0.656
```

Mycelium saved ~$0.66 worth of input tokens in that session.

## Legacy Metrics

Two additional cost-per-token calculations exist for comparison. They appear only in verbose/export mode and bracket the true value.

### Blended CPT (Lower Bound)

```
blended_cpt = total_cost / total_tokens
```

Where `total_tokens` includes all token types (input + output + cache write + cache read). Because cache-read tokens are very cheap but numerous, they dilute the denominator, producing a CPT that **underestimates** the value of saved tokens.

### Active CPT (Upper Bound)

```
active_cpt = total_cost / (input_tokens + output_tokens)
```

Ignores cache tokens entirely. Because the full cost is divided among fewer tokens, this **overestimates** the cost per token.

### Relationship

```
blended_cpt  <  weighted_input_cpt  <  active_cpt
(lower bound)   (best estimate)       (upper bound)
```

The weighted metric sits between the two because it properly accounts for the cost contribution of each token type.

**Source:** `src/cc_economics/models.rs` — `compute_dual_metrics()`

## Quota Analysis

For Claude Code subscription users (non-API), mycelium estimates savings as a percentage of your monthly token quota.

### Estimated Quotas

| Tier | Monthly Tokens | Derivation |
|------|---------------|------------|
| Pro ($20/mo) | 6,000,000 | ~44K tokens per 5-hour rolling window |
| Max 5x ($100/mo) | 30,000,000 | 5x Pro baseline |
| Max 20x ($200/mo) | 120,000,000 | 20x Pro baseline |

```
quota_utilization = (total_saved / quota_tokens) * 100
```

These are rough estimates. Claude Code uses rolling 5-hour rate windows, not fixed monthly caps. The 6M/month figure is derived from sustained-use extrapolation and will vary by actual usage patterns.

**Source:** `src/gain/display.rs` — `ESTIMATED_PRO_MONTHLY`

## Accuracy and Limitations

### Token Estimation Error

The `ceil(chars / 4)` heuristic has different accuracy depending on content type:

| Content Type | Real chars/token | Heuristic | Relative Error |
|-------------|-----------------|-----------|----------------|
| English prose | ~4.0 | 4.0 | ~0% |
| Source code | ~3.0–3.5 | 4.0 | 15–30% overcount |
| JSON / structured | ~2.5–3.0 | 4.0 | 25–40% overcount |
| Whitespace-heavy | ~2.0–2.5 | 4.0 | 40–50% overcount |
| Hex strings, hashes | ~5.0+ | 4.0 | slight undercount |

### Why Percentages Are More Reliable Than Absolutes

Both the raw output and filtered output are measured with the same heuristic. Because the systematic bias affects numerator and denominator equally, it largely cancels out in the ratio:

```
savings_pct = (estimated_raw - estimated_filtered) / estimated_raw
```

If both estimates are off by the same factor k:

```
(k * actual_raw - k * actual_filtered) / (k * actual_raw)
= (actual_raw - actual_filtered) / actual_raw
```

The percentage is invariant to the scaling factor. Absolute token counts (e.g., "223K tokens saved") may be off by 20–40%, but the efficiency percentage (e.g., "93%") is close to the true value.

### Dollar Savings Accuracy

The weighted economics calculation compounds two sources of error:

1. **Token estimation** — the chars/4 heuristic (see above)
2. **Price weights** — the 5x/1.25x/0.1x ratios are based on published API pricing and may not exactly match Claude Code subscription economics

For API users with pay-per-token billing, the dollar figures are reasonably accurate. For subscription users, treat them as directional estimates.

## Reference Tables

### Per-Command Savings Estimates

These static estimates are used by `mycelium discover` to project potential savings before actual execution.

| Command | Base Savings | Subcommand Overrides |
|---------|-------------|---------------------|
| git | 70% | diff/show: 80%, add/commit: 59% |
| gh | 82% | pr: 87%, issue: 80%, run: 82% |
| cargo | 80% | test/nextest: 90%, fmt: passthrough |
| cat/head/tail | 60% | — |
| grep/rg | 75% | — |
| ls | 65% | — |
| find | 70% | — |
| tsc | 83% | — |
| eslint/biome | 84% | — |
| prettier | 70% | — |
| next build | 87% | — |
| vitest/jest | 99% | — |
| playwright | 94% | — |
| prisma | 88% | — |
| docker/podman | 85% | — |
| kubectl | 85% | — |
| curl | 70% | — |
| wget | 65% | — |
| ruff | 80% | check: 80%, format: 75% |
| pytest | 90% | — |
| pip | 75% | list: 75%, outdated: 80% |
| go | 85% | test: 90%, build: 80%, vet: 75% |
| golangci-lint | 85% | — |
| terraform/tofu | 75% | plan: 80%, apply: 85% |
| aws | 80% | — |
| psql | 75% | — |

**Source:** `src/discover/rules.rs`

### Category Average Tokens

Expected output size per command category (used for discovery projections):

| Category | Subcommand | Estimated Tokens |
|----------|-----------|-----------------|
| Git | log, diff, show | 200 |
| Git | other | 40 |
| Cargo | test | 500 |
| Cargo | other | 150 |
| Tests | — | 800 |
| Files | — | 100 |
| Build | — | 300 |
| Infra | — | 120 |
| Network | — | 150 |
| GitHub | — | 200 |
| PackageManager | — | 150 |

**Source:** `src/discover/registry.rs` — `category_avg_tokens()`

## See Also

- [TRACKING.md](TRACKING.md) — Tracking API, database schema, integration examples
- [AUDIT_GUIDE.md](AUDIT_GUIDE.md) — Using `mycelium gain` and `mycelium discover`
- [ARCHITECTURE.md](ARCHITECTURE.md) — System architecture overview
