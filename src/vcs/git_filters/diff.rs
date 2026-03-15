//! Compact unified diff filter.

/// Compact a unified diff into a token-efficient summary.
///
/// Shows file names, hunk headers, and changed lines up to `max_hunk_lines`
/// per hunk, capping the total output at `max_lines` result lines.
pub(crate) fn compact_diff(diff: &str, max_lines: usize) -> String {
    let mut result = Vec::new();
    let mut current_file = String::new();
    let mut added = 0;
    let mut removed = 0;
    let mut in_hunk = false;
    let mut hunk_lines = 0;
    let max_hunk_lines = 100;

    for line in diff.lines() {
        if line.starts_with("diff --git") {
            // New file
            if !current_file.is_empty() && (added > 0 || removed > 0) {
                result.push(format!("  +{} -{}", added, removed));
            }
            current_file = line.split(" b/").nth(1).unwrap_or("unknown").to_string();
            result.push(format!("\n📄 {}", current_file));
            added = 0;
            removed = 0;
            in_hunk = false;
        } else if line.starts_with("@@") {
            // New hunk
            in_hunk = true;
            hunk_lines = 0;
            let hunk_info = line.split("@@").nth(1).unwrap_or("").trim();
            result.push(format!("  @@ {} @@", hunk_info));
        } else if in_hunk {
            if line.starts_with('+') && !line.starts_with("+++") {
                added += 1;
                if hunk_lines < max_hunk_lines {
                    result.push(format!("  {}", line));
                    hunk_lines += 1;
                }
            } else if line.starts_with('-') && !line.starts_with("---") {
                removed += 1;
                if hunk_lines < max_hunk_lines {
                    result.push(format!("  {}", line));
                    hunk_lines += 1;
                }
            } else if hunk_lines < max_hunk_lines && !line.starts_with("\\") {
                // Context line
                if hunk_lines > 0 {
                    result.push(format!("  {}", line));
                    hunk_lines += 1;
                }
            }

            if hunk_lines == max_hunk_lines {
                result.push("  ... (truncated)".to_string());
                hunk_lines += 1;
            }
        }

        if result.len() >= max_lines {
            result.push("\n... (more changes truncated)".to_string());
            break;
        }
    }

    if !current_file.is_empty() && (added > 0 || removed > 0) {
        result.push(format!("  +{} -{}", added, removed));
    }

    result.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compact_diff() {
        let diff = r#"diff --git a/foo.rs b/foo.rs
--- a/foo.rs
+++ b/foo.rs
@@ -1,3 +1,4 @@
 fn main() {
+    println!("hello");
 }
"#;
        let result = compact_diff(diff, 100);
        assert!(result.contains("foo.rs"));
        assert!(result.contains("+"));
    }

    #[test]
    fn test_compact_diff_increased_hunk_limit() {
        // Build a hunk with 25 changed lines — should NOT be truncated with limit 30
        let mut diff =
            "diff --git a/big.rs b/big.rs\n--- a/big.rs\n+++ b/big.rs\n@@ -1,25 +1,25 @@\n"
                .to_string();
        for i in 1..=25 {
            diff.push_str(&format!("+line{}\n", i));
        }
        let result = compact_diff(&diff, 500);
        assert!(
            !result.contains("... (truncated)"),
            "25 lines should not be truncated with max_hunk_lines=30"
        );
        assert!(result.contains("+line25"));
    }

    #[test]
    fn test_compact_diff_increased_total_limit() {
        // Build a diff with 150 output result lines across multiple files — should NOT be cut at 100
        let mut diff = String::new();
        for f in 1..=5 {
            diff.push_str(&format!("diff --git a/file{f}.rs b/file{f}.rs\n--- a/file{f}.rs\n+++ b/file{f}.rs\n@@ -1,20 +1,20 @@\n"));
            for i in 1..=20 {
                diff.push_str(&format!("+line{f}_{i}\n"));
            }
        }
        let result = compact_diff(&diff, 500);
        assert!(
            !result.contains("more changes truncated"),
            "5 files × 20 lines should not exceed max_lines=500"
        );
    }

    #[test]
    fn test_compact_diff_token_savings() {
        fn count_tokens(text: &str) -> usize {
            text.split_whitespace().count()
        }

        // Build a large diff across many files with long context lines. The compact_diff filter
        // strips `index`/`---`/`+++`/`\No newline` metadata lines and truncates large hunks,
        // yielding well above 60% savings on a realistic multi-file diff.
        let mut diff = String::new();
        for f in 1..=8 {
            diff.push_str(&format!(
                "diff --git a/src/module{f}/component_{f}.rs b/src/module{f}/component_{f}.rs\n"
            ));
            diff.push_str(&format!("index abc{f:04x}def..123{f:04x}456 100644\n"));
            diff.push_str(&format!("--- a/src/module{f}/component_{f}.rs\n"));
            diff.push_str(&format!("+++ b/src/module{f}/component_{f}.rs\n"));
            diff.push_str(&format!("@@ -{f}0,35 +{f}0,37 @@ impl Component{f} {{\n"));
            // Add 35 context lines (unchanged) + 2 added lines per file
            for i in 1..=35 {
                diff.push_str(&format!(
                    "     let field_{i}: String = some_long_function_call_{i}(arg_one, arg_two, arg_three);\n"
                ));
            }
            diff.push_str("+    let new_field: String = added_function_call(arg_one, arg_two);\n");
            diff.push_str("+    tracing::debug!(\"component {f} initialised with new_field={{}}\", new_field);\n");
        }

        let result = compact_diff(&diff, 500);
        let savings = (count_tokens(&diff).saturating_sub(count_tokens(&result))) * 100
            / count_tokens(&diff).max(1);
        assert!(
            savings >= 60,
            "Git diff filter: expected >= 60% token savings, got {}%",
            savings
        );
    }
}
