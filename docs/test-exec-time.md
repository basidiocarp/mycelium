# Testing Execution Time Tracking

## Quick Test

```bash
# 1. Install latest version
cargo install --path .

# 2. Run a few commands to populate data
mycelium git status
mycelium ls .
mycelium grep "tracking" src/

# 3. Check gain stats (should show execution times)
mycelium gain

# Expected output:
# Total exec time:   XX.Xs (avg XXms)
# By Command table should show Time column
```

## Detailed Test Scenarios

### 1. Basic Time Tracking
```bash
# Run commands with different execution times
mycelium git log -10          # Fast (~10ms)
mycelium cargo test           # Slow (~300ms)
mycelium vitest run           # Very slow (seconds)

# Verify times are recorded
mycelium gain
# Should show different avg times per command
```

### 2. Daily Breakdown
```bash
mycelium gain --daily

# Expected:
# Date column + Time column showing avg time per day
# Today should have non-zero times
# Historical data shows 0ms (no time recorded)
```

### 3. Export Formats

**JSON Export:**
```bash
mycelium gain --daily --format json | jq '.summary'

# Should include:
# "total_time_ms": 12345,
# "avg_time_ms": 67
```

**CSV Export:**
```bash
mycelium gain --daily --format csv

# Headers should include:
# date,commands,input_tokens,...,total_time_ms,avg_time_ms
```

### 4. Multiple Commands
```bash
# Run 10 commands and measure total time
for i in {1..10}; do mycelium git status; done

mycelium gain
# Total exec time should be ~10-50ms (10 × 1-5ms)
```

## Verification Checklist

- [ ] `mycelium gain` shows "Total exec time: X (avg Yms)"
- [ ] By Command table has "Time" column
- [ ] `mycelium gain --daily` shows time per day
- [ ] JSON export includes `total_time_ms` and `avg_time_ms`
- [ ] CSV export has time columns
- [ ] New commands show realistic times (not 0ms)
- [ ] Historical data preserved (old entries show 0ms)

## Database Schema Verification

```bash
# Check SQLite schema includes exec_time_ms
sqlite3 ~/.local/share/mycelium/history.db "PRAGMA table_info(commands);"

# Should show:
# ...
# 7|exec_time_ms|INTEGER|0|0|0
```

## Performance Impact

The timer adds negligible overhead:
- `Instant::now()` → ~10-50ns
- `elapsed()` → ~10-50ns
- SQLite insert with extra column → ~1-5µs

Total overhead: **< 0.1ms per command**

## Benchmarking and Profiling

Use **Criterion** when you want a repeatable benchmark for a pure Rust hot
path with stable inputs. In Mycelium, that is the right fit for functions like
command classification, rewrite decisions, and compact text filters where you
want regression detection across commits.

Use end-to-end timing and targeted tracing when you do not yet know which Rust
function is responsible for the slowdown. That is more useful for command
spawning, SQLite access, shell parsing, or other I/O-heavy work that Criterion
would miss or over-isolate.

Practical rule:

- start with command timing and tracing if the slowdown is still a mystery
- switch to Criterion once you know the hot function and want a stable guardrail
