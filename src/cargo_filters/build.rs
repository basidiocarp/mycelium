use std::collections::HashMap;

/// Filter cargo build/check output - strip "Compiling"/"Checking" lines, keep errors + summary
pub(crate) fn filter_cargo_build(output: &str) -> String {
    let mut errors: Vec<String> = Vec::new();
    let mut warnings = 0;
    let mut error_count = 0;
    let mut compiled = 0;
    let mut in_error = false;
    let mut current_error = Vec::new();

    for line in output.lines() {
        if line.trim_start().starts_with("Compiling") || line.trim_start().starts_with("Checking") {
            compiled += 1;
            continue;
        }
        if line.trim_start().starts_with("Downloading")
            || line.trim_start().starts_with("Downloaded")
        {
            continue;
        }
        if line.trim_start().starts_with("Finished") {
            continue;
        }

        // Detect error/warning blocks
        if line.starts_with("error[") || line.starts_with("error:") {
            // Skip "error: aborting due to" summary lines
            if line.contains("aborting due to") || line.contains("could not compile") {
                continue;
            }
            if in_error && !current_error.is_empty() {
                errors.push(current_error.join("\n"));
                current_error.clear();
            }
            error_count += 1;
            in_error = true;
            current_error.push(line.to_string());
        } else if line.starts_with("warning:")
            && line.contains("generated")
            && line.contains("warning")
        {
            // "warning: `crate` generated N warnings" summary line
            continue;
        } else if line.starts_with("warning:") || line.starts_with("warning[") {
            if in_error && !current_error.is_empty() {
                errors.push(current_error.join("\n"));
                current_error.clear();
            }
            warnings += 1;
            in_error = true;
            current_error.push(line.to_string());
        } else if in_error {
            if line.trim().is_empty() && current_error.len() > 3 {
                errors.push(current_error.join("\n"));
                current_error.clear();
                in_error = false;
            } else {
                current_error.push(line.to_string());
            }
        }
    }

    if !current_error.is_empty() {
        errors.push(current_error.join("\n"));
    }

    if error_count == 0 && warnings == 0 {
        return format!("✓ cargo build ({} crates compiled)", compiled);
    }

    let mut result = String::new();
    result.push_str(&format!(
        "cargo build: {} errors, {} warnings ({} crates)\n",
        error_count, warnings, compiled
    ));
    result.push_str("═══════════════════════════════════════\n");

    for (i, err) in errors.iter().enumerate().take(15) {
        result.push_str(err);
        result.push('\n');
        if i < errors.len() - 1 {
            result.push('\n');
        }
    }

    if errors.len() > 15 {
        result.push_str(&format!("\n... +{} more issues\n", errors.len() - 15));
    }

    result.trim().to_string()
}

/// Filter cargo clippy output - group warnings by lint rule
pub(crate) fn filter_cargo_clippy(output: &str) -> String {
    let mut by_rule: HashMap<String, Vec<String>> = HashMap::new();
    let mut error_count = 0;
    let mut warning_count = 0;

    // Parse clippy output lines
    // Format: "warning: description\n  --> file:line:col\n  |\n  | code\n"
    let mut current_rule = String::new();

    for line in output.lines() {
        // Skip compilation lines
        if line.trim_start().starts_with("Compiling")
            || line.trim_start().starts_with("Checking")
            || line.trim_start().starts_with("Downloading")
            || line.trim_start().starts_with("Downloaded")
            || line.trim_start().starts_with("Finished")
        {
            continue;
        }

        // "warning: unused variable [unused_variables]" or "warning: description [clippy::rule_name]"
        if (line.starts_with("warning:") || line.starts_with("warning["))
            || (line.starts_with("error:") || line.starts_with("error["))
        {
            // Skip summary lines: "warning: `mycelium` (bin) generated 5 warnings"
            if line.contains("generated") && line.contains("warning") {
                continue;
            }
            // Skip "error: aborting" / "error: could not compile"
            if line.contains("aborting due to") || line.contains("could not compile") {
                continue;
            }

            let is_error = line.starts_with("error");
            if is_error {
                error_count += 1;
            } else {
                warning_count += 1;
            }

            // Extract rule name from brackets
            current_rule = if let Some(bracket_start) = line.rfind('[') {
                if let Some(bracket_end) = line.rfind(']') {
                    line[bracket_start + 1..bracket_end].to_string()
                } else {
                    line.to_string()
                }
            } else {
                // No bracket: use the message itself as the rule
                let prefix = if is_error { "error: " } else { "warning: " };
                line.strip_prefix(prefix).unwrap_or(line).to_string()
            };
        } else if line.trim_start().starts_with("--> ") {
            let location = line.trim_start().trim_start_matches("--> ").to_string();
            if !current_rule.is_empty() {
                by_rule
                    .entry(current_rule.clone())
                    .or_default()
                    .push(location);
            }
        }
    }

    if error_count == 0 && warning_count == 0 {
        return "✓ cargo clippy: No issues found".to_string();
    }

    let mut result = String::new();
    result.push_str(&format!(
        "cargo clippy: {} errors, {} warnings\n",
        error_count, warning_count
    ));
    result.push_str("═══════════════════════════════════════\n");

    // Sort rules by frequency
    let mut rule_counts: Vec<_> = by_rule.iter().collect();
    rule_counts.sort_by_key(|a| std::cmp::Reverse(a.1.len()));

    for (rule, locations) in rule_counts.iter().take(15) {
        result.push_str(&format!("  {} ({}x)\n", rule, locations.len()));
        for loc in locations.iter().take(3) {
            result.push_str(&format!("    {}\n", loc));
        }
        if locations.len() > 3 {
            result.push_str(&format!("    ... +{} more\n", locations.len() - 3));
        }
    }

    if by_rule.len() > 15 {
        result.push_str(&format!("\n... +{} more rules\n", by_rule.len() - 15));
    }

    result.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filter_cargo_build_success() {
        let output = r#"   Compiling libc v0.2.153
   Compiling cfg-if v1.0.0
   Compiling mycelium v0.5.0
    Finished dev [unoptimized + debuginfo] target(s) in 15.23s
"#;
        let result = filter_cargo_build(output);
        assert!(result.contains("✓ cargo build"));
        assert!(result.contains("3 crates compiled"));
    }

    #[test]
    fn test_filter_cargo_build_errors() {
        let output = r#"   Compiling mycelium v0.5.0
error[E0308]: mismatched types
 --> src/main.rs:10:5
  |
10|     "hello"
  |     ^^^^^^^ expected `i32`, found `&str`

error: aborting due to 1 previous error
"#;
        let result = filter_cargo_build(output);
        assert!(result.contains("1 errors"));
        assert!(result.contains("E0308"));
        assert!(result.contains("mismatched types"));
    }

    #[test]
    fn test_filter_cargo_clippy_clean() {
        let output = r#"    Checking mycelium v0.5.0
    Finished dev [unoptimized + debuginfo] target(s) in 1.53s
"#;
        let result = filter_cargo_clippy(output);
        assert!(result.contains("✓ cargo clippy: No issues found"));
    }

    #[test]
    fn test_filter_cargo_clippy_warnings() {
        let output = r#"    Checking mycelium v0.5.0
warning: unused variable: `x` [unused_variables]
 --> src/main.rs:10:9
  |
10|     let x = 5;
  |         ^ help: if this is intentional, prefix it with an underscore: `_x`

warning: this function has too many arguments [clippy::too_many_arguments]
 --> src/git.rs:16:1
  |
16| pub fn run(a: i32, b: i32, c: i32, d: i32, e: i32, f: i32, g: i32, h: i32) {}
  |

warning: `mycelium` (bin) generated 2 warnings
    Finished dev [unoptimized + debuginfo] target(s) in 1.53s
"#;
        let result = filter_cargo_clippy(output);
        assert!(result.contains("0 errors, 2 warnings"));
        assert!(result.contains("unused_variables"));
        assert!(result.contains("clippy::too_many_arguments"));
    }

    fn count_tokens(text: &str) -> usize {
        crate::tracking::estimate_tokens(text)
    }

    #[test]
    fn test_cargo_build_token_savings() {
        let input = include_str!("../../tests/fixtures/cargo_build_raw.txt");
        let output = filter_cargo_build(input);
        let input_tokens = count_tokens(input);
        let output_tokens = count_tokens(&output);
        let savings = if input_tokens > 0 {
            (input_tokens.saturating_sub(output_tokens)) * 100 / input_tokens
        } else {
            0
        };
        assert!(
            savings >= 60,
            "Expected >= 60% token savings, got {}% ({} -> {} tokens)",
            savings,
            input_tokens,
            output_tokens
        );
    }

    #[test]
    fn test_cargo_clippy_token_savings() {
        let input = include_str!("../../tests/fixtures/cargo_clippy_raw.txt");
        let output = filter_cargo_clippy(input);
        let input_tokens = count_tokens(input);
        let output_tokens = count_tokens(&output);
        let savings = if input_tokens > 0 {
            (input_tokens.saturating_sub(output_tokens)) * 100 / input_tokens
        } else {
            0
        };
        assert!(
            savings >= 60,
            "Expected >= 60% token savings, got {}% ({} -> {} tokens)",
            savings,
            input_tokens,
            output_tokens
        );
    }
}
