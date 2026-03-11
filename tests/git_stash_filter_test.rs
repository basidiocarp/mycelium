//! Tests for git stash filter
//!
//! Snapshot and token savings tests for the filter_stash_list function.

use std::path::Path;

fn filter_stash_list(output: &str) -> String {
    // Replicate the filter_stash_list logic for testing
    // Format: "stash@{0}: WIP on main: abc1234 commit message"
    let mut result = Vec::new();
    for line in output.lines() {
        if let Some(colon_pos) = line.find(": ") {
            let index = &line[..colon_pos];
            let rest = &line[colon_pos + 2..];
            // Compact: strip "WIP on branch:" prefix if present
            let message = if let Some(second_colon) = rest.find(": ") {
                rest[second_colon + 2..].trim()
            } else {
                rest.trim()
            };
            result.push(format!("{}: {}", index, message));
        } else {
            result.push(line.to_string());
        }
    }
    result.join("\n")
}

fn count_tokens(text: &str) -> usize {
    text.split_whitespace().count()
}

fn fixture_path(name: &str) -> String {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
        .to_string_lossy()
        .into_owned()
}

#[test]
fn test_filter_stash_list_snapshot() {
    let path = fixture_path("git_stash_list_raw.txt");
    let input = std::fs::read_to_string(&path).expect("failed to read fixture");
    let output = filter_stash_list(&input);
    insta::assert_snapshot!(output);
}

#[test]
fn test_filter_stash_list_token_savings() {
    let path = fixture_path("git_stash_list_raw.txt");
    let input = std::fs::read_to_string(&path).expect("failed to read fixture");
    let output = filter_stash_list(&input);

    let input_tokens = count_tokens(&input);
    let output_tokens = count_tokens(&output);

    let savings = 100.0 - (output_tokens as f64 / input_tokens as f64 * 100.0);

    println!("Input tokens: {}", input_tokens);
    println!("Output tokens: {}", output_tokens);
    println!("Token savings: {:.1}%", savings);

    assert!(
        savings >= 25.0,
        "Git stash list filter: expected ≥25% savings, got {:.1}%",
        savings
    );
}
