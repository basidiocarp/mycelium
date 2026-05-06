# Compressed Format Experiments — Decision Note

## Outcome: No new format implemented

Experiments ran against 7 representative fixture types using three formats:
raw text (baseline), compact JSON (whitespace-stripped), and a TOON-like
key-value notation. The 30% savings threshold for implementation was not
consistently met.

## Results

| Fixture | Baseline tokens | Compact JSON savings | TOON-like savings |
|---------|-----------------|----------------------|-------------------|
| cargo_build_raw.txt | 530 | 12.5% | 8.1% |
| cargo_nextest_raw.txt | 658 | 9.0% | 7.4% |
| docker_ps_raw.txt | 295 | **43.1%** | 0.0% |
| git_stash_list_raw.txt | 319 | 0.0% | 0.0% |
| pip_list_raw.txt | 220 | 14.1% | 25.5% |
| eslint_json_raw.txt | 631 | 16.5% | n/a (non-JSON path) |
| curl_large_json.json | 1724 | 20.4% | **–2.4%** (expanded) |

## Analysis

**Compact JSON** shows modest savings for most types (9–21%) and one outlier
(docker_ps_raw.txt at 43%) due to that file's heavy whitespace. Mycelium
already strips excess whitespace via its existing filters, so the remaining
gains do not justify a new pipeline stage.

**TOON-like** underperforms compact JSON on all JSON inputs and actually
expands nested JSON (curl_large_json: +42 tokens) because the flat key-path
notation adds characters for deeply nested structures. For plain text it
offers minor savings but nothing transformative.

## Decision

Neither format clears the 30% threshold consistently across the tested output
types. The evaluation framework lives in `tests/format_eval/main.rs` and can
be re-run if new fixtures or formats warrant re-evaluation. No change to
Mycelium's output pipeline.
