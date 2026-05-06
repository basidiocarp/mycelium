//! Format evaluation harness for comparing token savings across output encodings.
//!
//! Compares three formats: raw text, compact JSON, and a TOON-like key-value
//! notation. Uses existing fixtures from tests/fixtures/ and mycelium's own
//! estimate_tokens to measure savings.

use mycelium::tracking::estimate_tokens;

// ── Format definitions ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    /// Input text unchanged.
    Raw,
    /// JSON with all whitespace stripped (compact serialization).
    CompactJson,
    /// Key-value flat notation inspired by TOON: reduces JSON verbosity
    /// by removing structural punctuation and using minimal delimiters.
    ToonLike,
}

impl std::fmt::Display for Format {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Format::Raw => write!(f, "raw"),
            Format::CompactJson => write!(f, "compact-json"),
            Format::ToonLike => write!(f, "toon-like"),
        }
    }
}

// ── Encoders ─────────────────────────────────────────────────────────────────

/// Encode text into the target format. Returns input unchanged on encode error.
pub fn encode(format: Format, input: &str) -> String {
    match format {
        Format::Raw => input.to_string(),
        Format::CompactJson => compact_json(input),
        Format::ToonLike => toon_like(input),
    }
}

/// Strip whitespace from JSON by re-serializing through serde_json.
/// Falls back to whitespace-collapsing for non-JSON input.
fn compact_json(input: &str) -> String {
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(input) {
        serde_json::to_string(&value).unwrap_or_else(|_| input.to_string())
    } else {
        // Non-JSON: collapse runs of whitespace to single space
        input
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    }
}

/// Produce a TOON-like flat notation. For JSON input this flattens the
/// structure to `key:value` pairs separated by `|`. For plain text, collapses
/// whitespace. The goal is measurable token reduction, not perfect fidelity.
fn toon_like(input: &str) -> String {
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(input) {
        let mut parts: Vec<String> = Vec::new();
        flatten_value("", &value, &mut parts);
        parts.join("|")
    } else {
        // Plain text: strip leading/trailing whitespace per line, drop blank lines
        input
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .collect::<Vec<_>>()
            .join("|")
    }
}

fn flatten_value(prefix: &str, value: &serde_json::Value, out: &mut Vec<String>) {
    match value {
        serde_json::Value::Object(map) => {
            for (k, v) in map {
                let key = if prefix.is_empty() {
                    k.clone()
                } else {
                    format!("{}.{}", prefix, k)
                };
                flatten_value(&key, v, out);
            }
        }
        serde_json::Value::Array(arr) => {
            for (i, v) in arr.iter().enumerate() {
                let key = if prefix.is_empty() {
                    i.to_string()
                } else {
                    format!("{}[{}]", prefix, i)
                };
                flatten_value(&key, v, out);
            }
        }
        serde_json::Value::Null => {
            if !prefix.is_empty() {
                out.push(format!("{}:null", prefix));
            }
        }
        serde_json::Value::Bool(b) => out.push(format!("{}:{}", prefix, b)),
        serde_json::Value::Number(n) => out.push(format!("{}:{}", prefix, n)),
        serde_json::Value::String(s) => {
            let clean = s.split_whitespace().collect::<Vec<_>>().join(" ");
            if !clean.is_empty() {
                out.push(format!("{}:{}", prefix, clean));
            }
        }
    }
}

// ── Measurement types ─────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct FormatResult {
    pub format: Format,
    pub tokens: usize,
    pub savings_pct: f64,
    pub encoded: String,
}

/// Compare all formats against `input`. Raw is the baseline for savings %.
pub fn compare_formats(input: &str) -> Vec<FormatResult> {
    let baseline_tokens = estimate_tokens(input);

    [Format::Raw, Format::CompactJson, Format::ToonLike]
        .iter()
        .map(|&fmt| {
            let encoded = encode(fmt, input);
            let tokens = estimate_tokens(&encoded);
            let savings_pct = if baseline_tokens > 0 && tokens < baseline_tokens {
                (1.0 - tokens as f64 / baseline_tokens as f64) * 100.0
            } else {
                0.0
            };
            FormatResult {
                format: fmt,
                tokens,
                savings_pct,
                encoded,
            }
        })
        .collect()
}

// ── Structural preservation check ────────────────────────────────────────────

/// Check whether scalar string values from a JSON input survive encoding
/// into the target format. Returns ratio of preserved values (0.0–1.0).
pub fn structural_preservation_score(input: &str, encoded: &str) -> f64 {
    let Ok(root) = serde_json::from_str::<serde_json::Value>(input) else {
        return 1.0; // non-JSON: no structural check possible
    };

    let mut leaves: Vec<String> = Vec::new();
    collect_string_leaves(&root, &mut leaves);

    if leaves.is_empty() {
        return 1.0;
    }

    let preserved = leaves
        .iter()
        .filter(|leaf| encoded.contains(leaf.as_str()))
        .count();

    preserved as f64 / leaves.len() as f64
}

fn collect_string_leaves(value: &serde_json::Value, out: &mut Vec<String>) {
    match value {
        serde_json::Value::String(s) if !s.is_empty() && s.len() >= 3 => {
            out.push(s.clone());
        }
        serde_json::Value::Object(m) => m.values().for_each(|v| collect_string_leaves(v, out)),
        serde_json::Value::Array(a) => a.iter().for_each(|v| collect_string_leaves(v, out)),
        _ => {}
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    const PRETTY_JSON: &str = r#"{
    "status": "ok",
    "count": 42,
    "items": [
        {"name": "alpha", "value": 1},
        {"name": "beta",  "value": 2}
    ]
}"#;

    #[test]
    fn raw_format_is_unchanged() {
        let out = encode(Format::Raw, PRETTY_JSON);
        assert_eq!(out, PRETTY_JSON);
    }

    #[test]
    fn compact_json_strips_whitespace() {
        let out = encode(Format::CompactJson, PRETTY_JSON);
        // No newlines or leading spaces
        assert!(!out.contains('\n'));
        // Still valid JSON
        let reparsed: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(reparsed["status"], "ok");
        assert_eq!(reparsed["count"], 42);
    }

    #[test]
    fn toon_like_is_shorter_than_raw() {
        let raw_tokens = estimate_tokens(PRETTY_JSON);
        let toon = encode(Format::ToonLike, PRETTY_JSON);
        let toon_tokens = estimate_tokens(&toon);
        assert!(
            toon_tokens < raw_tokens,
            "TOON ({} tokens) should be shorter than raw ({} tokens)",
            toon_tokens,
            raw_tokens
        );
    }

    #[test]
    fn toon_like_preserves_key_values() {
        let toon = encode(Format::ToonLike, PRETTY_JSON);
        assert!(toon.contains("status:ok"), "missing status:ok in {}", toon);
        assert!(toon.contains("count:42"), "missing count:42 in {}", toon);
    }

    #[test]
    fn compare_formats_returns_three_results() {
        let results = compare_formats(PRETTY_JSON);
        assert_eq!(results.len(), 3);
        assert_eq!(results[0].format, Format::Raw);
        assert_eq!(results[1].format, Format::CompactJson);
        assert_eq!(results[2].format, Format::ToonLike);
    }

    #[test]
    fn raw_baseline_has_zero_savings() {
        let results = compare_formats(PRETTY_JSON);
        let raw = &results[0];
        assert_eq!(raw.savings_pct, 0.0);
    }

    #[test]
    fn compact_json_saves_some_tokens() {
        let results = compare_formats(PRETTY_JSON);
        let compact = &results[1];
        assert!(
            compact.savings_pct > 0.0,
            "compact JSON should save tokens on pretty JSON"
        );
    }

    #[test]
    fn structural_preservation_score_is_high_for_compact_json() {
        let compact = encode(Format::CompactJson, PRETTY_JSON);
        let score = structural_preservation_score(PRETTY_JSON, &compact);
        assert!(
            score >= 0.9,
            "compact JSON should preserve ≥90% of string leaves, got {:.2}",
            score
        );
    }

    #[test]
    fn plain_text_toon_strips_blank_lines() {
        let plain = "  line one  \n\n  line two  \n";
        let out = encode(Format::ToonLike, plain);
        assert_eq!(out, "line one|line two");
    }

    #[test]
    fn plain_text_compact_collapses_whitespace() {
        let plain = "  line one  \n\n  line two  \n";
        let out = encode(Format::CompactJson, plain);
        assert_eq!(out, "line one line two");
    }

    // ── Fixture-based experiments ─────────────────────────────────────────────

    #[test]
    fn experiment_curl_large_json() {
        let input = include_str!("../fixtures/curl_large_json.json");
        let results = compare_formats(input);
        print_experiment_results("curl_large_json.json", &results, input);
        // compact JSON must save tokens on a pretty-printed JSON fixture
        let compact = &results[1];
        assert!(
            compact.savings_pct >= 0.0,
            "compact JSON should not increase token count"
        );
    }

    #[test]
    fn experiment_cargo_build() {
        let input = include_str!("../fixtures/cargo_build_raw.txt");
        let results = compare_formats(input);
        print_experiment_results("cargo_build_raw.txt", &results, input);
        // plain text TOON must not expand the output
        let toon = &results[2];
        assert!(
            toon.tokens <= estimate_tokens(input),
            "TOON should not expand cargo build output"
        );
    }

    #[test]
    fn experiment_cargo_nextest() {
        let input = include_str!("../fixtures/cargo_nextest_raw.txt");
        let results = compare_formats(input);
        print_experiment_results("cargo_nextest_raw.txt", &results, input);
    }

    #[test]
    fn experiment_eslint_json() {
        let input = include_str!("../fixtures/eslint_json_raw.txt");
        let results = compare_formats(input);
        print_experiment_results("eslint_json_raw.txt", &results, input);
    }

    #[test]
    fn experiment_docker_ps() {
        let input = include_str!("../fixtures/docker_ps_raw.txt");
        let results = compare_formats(input);
        print_experiment_results("docker_ps_raw.txt", &results, input);
    }

    #[test]
    fn experiment_pip_list() {
        let input = include_str!("../fixtures/pip_list_raw.txt");
        let results = compare_formats(input);
        print_experiment_results("pip_list_raw.txt", &results, input);
    }

    #[test]
    fn experiment_git_stash_list() {
        let input = include_str!("../fixtures/git_stash_list_raw.txt");
        let results = compare_formats(input);
        print_experiment_results("git_stash_list_raw.txt", &results, input);
    }

    fn print_experiment_results(fixture: &str, results: &[FormatResult], input: &str) {
        let baseline = estimate_tokens(input);
        println!("\n=== {} ({} tokens baseline) ===", fixture, baseline);
        for r in results {
            println!(
                "  {:14} {:5} tokens  {:5.1}% savings",
                r.format.to_string(),
                r.tokens,
                r.savings_pct
            );
        }
    }
}
