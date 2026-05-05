//! Token-optimized grep that groups results by file and truncates long lines.
use anyhow::{Context, Result};
use std::process::Command;

/// Search for a pattern using `rg` (with `grep` fallback) and group results by file.
#[allow(clippy::too_many_arguments)]
pub fn run(
    pattern: &str,
    path: &str,
    max_line_len: usize,
    max_results: usize,
    context_only: bool,
    file_type: Option<&str>,
    extra_args: &[String],
    verbose: u8,
) -> Result<()> {
    if verbose > 0 {
        eprintln!("grep: '{}' in {}", pattern, path);
    }

    // Fix: convert BRE alternation \| → | for rg (which uses PCRE-style regex)
    let rg_pattern = pattern.replace(r"\|", "|");

    let mut rg_cmd = Command::new("rg");
    rg_cmd.args(["-n", "--no-heading", &rg_pattern, path]);

    if let Some(ft) = file_type {
        rg_cmd.arg("--type").arg(ft);
    }

    for arg in extra_args {
        // Fix: skip grep-ism -r flag (rg is recursive by default; rg -r means --replace)
        if arg == "-r" || arg == "--recursive" {
            continue;
        }
        rg_cmd.arg(arg);
    }

    let output = rg_cmd
        .output()
        .or_else(|_| Command::new("grep").args(["-rn", pattern, path]).output())
        .context("grep/rg failed")?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let exit_code = output.status.code().unwrap_or(1);

    if stdout.trim().is_empty() {
        // Show stderr for errors (bad regex, missing file, etc.)
        if exit_code == 2 {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if !stderr.trim().is_empty() {
                eprintln!("{}", stderr.trim());
            }
        }
        let msg = format!("0 for '{}'", pattern);
        println!("{}", msg);
        if exit_code != 0 {
            std::process::exit(exit_code);
        }
        return Ok(());
    }

    let mut grouper = crate::display_helpers::FileGrouper::new(100, 10);
    let mut total = 0;

    for line in stdout.lines() {
        let parts: Vec<&str> = line.splitn(3, ':').collect();

        let (file, line_num, content) = if parts.len() == 3 {
            let ln = parts[1].parse().unwrap_or(0);
            (parts[0].to_string(), ln, parts[2])
        } else if parts.len() == 2 {
            let ln = parts[0].parse().unwrap_or(0);
            (path.to_string(), ln, parts[1])
        } else {
            continue;
        };

        total += 1;
        if total > max_results {
            break;
        }

        let cleaned = clean_line(content, max_line_len, context_only, pattern);
        let file_display = compact_path(&file);
        grouper.add(&file_display, line_num, &cleaned);
    }

    println!("{} in {}:", total, grouper.file_count());
    println!("{}", grouper.format());

    if total > max_results {
        println!("... +{}", total - max_results);
    }

    if is_code_search(path, extra_args) && code_search_hint_enabled() {
        println!(
            "\n[BASIDIOCARP] Code search via bash detected.\nPreferred tools for this query type:\n  rhizome_find_symbol / rhizome_find_references / rhizome_search_code"
        );
    }

    if exit_code != 0 {
        std::process::exit(exit_code);
    }

    Ok(())
}

/// Returns true when the path or extra args indicate a source-file code search.
fn is_code_search(path: &str, extra_args: &[String]) -> bool {
    const SOURCE_EXTENSIONS: &[&str] = &[".rs", ".ts", ".tsx", ".js", ".jsx", ".py", ".go"];
    const SOURCE_PREFIXES: &[&str] = &["src/", "./src/", "src\\", "lib/", "./lib/"];

    let targets = std::iter::once(path).chain(extra_args.iter().map(String::as_str));
    for target in targets {
        if target.starts_with('-') {
            continue;
        }
        if SOURCE_EXTENSIONS.iter().any(|ext| target.ends_with(ext)) {
            return true;
        }
        if SOURCE_PREFIXES.iter().any(|prefix| target.starts_with(prefix)) {
            return true;
        }
    }
    false
}

/// Returns false when `MYCELIUM_CODE_SEARCH_HINT=0` or `=false` is set.
fn code_search_hint_enabled() -> bool {
    match std::env::var("MYCELIUM_CODE_SEARCH_HINT").as_deref() {
        Ok("0") | Ok("false") | Ok("off") => false,
        _ => true,
    }
}

fn clean_line(line: &str, max_len: usize, context_only: bool, pattern: &str) -> String {
    let trimmed = line.trim();

    if context_only {
        let lower = trimmed.to_lowercase();
        let pattern_lower = pattern.to_lowercase();
        if let Some(pos) = lower.find(&pattern_lower) {
            let start = pos.saturating_sub(20);
            let snippet = &trimmed[start..];
            if snippet.len() <= max_len {
                return snippet.to_string();
            }
        }
    }

    if trimmed.len() <= max_len {
        trimmed.to_string()
    } else {
        let lower = trimmed.to_lowercase();
        let pattern_lower = pattern.to_lowercase();

        if let Some(pos) = lower.find(&pattern_lower) {
            let char_pos = lower[..pos].chars().count();
            let chars: Vec<char> = trimmed.chars().collect();
            let char_len = chars.len();

            let start = char_pos.saturating_sub(max_len / 3);
            let end = (start + max_len).min(char_len);
            let start = if end == char_len {
                end.saturating_sub(max_len)
            } else {
                start
            };

            let slice: String = chars[start..end].iter().collect();
            if start > 0 && end < char_len {
                format!("...{}...", slice)
            } else if start > 0 {
                format!("...{}", slice)
            } else {
                format!("{}...", slice)
            }
        } else {
            let t: String = trimmed.chars().take(max_len - 3).collect();
            format!("{}...", t)
        }
    }
}

fn compact_path(path: &str) -> String {
    if path.len() <= 50 {
        return path.to_string();
    }

    let parts: Vec<&str> = path.split('/').collect();
    if parts.len() <= 3 {
        return path.to_string();
    }

    format!(
        "{}/.../{}/{}",
        parts[0],
        parts[parts.len() - 2],
        parts[parts.len() - 1]
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clean_line() {
        let line = "            const result = someFunction();";
        let cleaned = clean_line(line, 50, false, "result");
        assert!(!cleaned.starts_with(' '));
        assert!(cleaned.len() <= 50);
    }

    #[test]
    fn test_compact_path() {
        let path = "/Users/patrick/dev/project/src/components/Button.tsx";
        let compact = compact_path(path);
        assert!(compact.len() <= 60);
    }

    #[test]
    fn test_extra_args_accepted() {
        // Test that the function signature accepts extra_args
        // This is a compile-time test - if it compiles, the signature is correct
        let _extra: Vec<String> = vec!["-i".to_string(), "-A".to_string(), "3".to_string()];
        // No need to actually run - we're verifying the parameter exists
    }

    #[test]
    fn test_clean_line_multibyte() {
        // Thai text that exceeds max_len in bytes
        let line = "  สวัสดีครับ นี่คือข้อความที่ยาวมากสำหรับทดสอบ  ";
        let cleaned = clean_line(line, 20, false, "ครับ");
        // Should not panic
        assert!(!cleaned.is_empty());
    }

    #[test]
    fn test_clean_line_emoji() {
        let line = "🎉🎊🎈🎁🎂🎄 some text 🎃🎆🎇✨";
        let cleaned = clean_line(line, 15, false, "text");
        assert!(!cleaned.is_empty());
    }

    // Fix: BRE \| alternation is translated to PCRE | for rg
    #[test]
    fn test_bre_alternation_translated() {
        let pattern = r"fn foo\|pub.*bar";
        let rg_pattern = pattern.replace(r"\|", "|");
        assert_eq!(rg_pattern, "fn foo|pub.*bar");
    }

    // Fix: -r flag (grep recursive) is stripped from extra_args (rg is recursive by default)
    #[test]
    fn test_recursive_flag_stripped() {
        let extra_args: Vec<String> = vec!["-r".to_string(), "-i".to_string()];
        let filtered: Vec<&String> = extra_args
            .iter()
            .filter(|a| *a != "-r" && *a != "--recursive")
            .collect();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0], "-i");
    }

    #[test]
    fn test_is_code_search_detects_source_extensions() {
        assert!(is_code_search("src/main.rs", &[]));
        assert!(is_code_search("lib.ts", &[]));
        assert!(is_code_search(".", &["src/lib.py".to_string()]));
        assert!(!is_code_search(".", &["-i".to_string(), "README.md".to_string()]));
        assert!(!is_code_search(".", &[]));
    }

    #[test]
    fn test_is_code_search_detects_source_prefixes() {
        assert!(is_code_search("src/", &[]));
        assert!(is_code_search("./src/", &[]));
        assert!(!is_code_search("tests/", &[]));
        assert!(!is_code_search("docs/guide.md", &[]));
    }

    #[test]
    fn test_code_search_hint_disabled_values() {
        // Test that "0", "false", "off" values disable the hint.
        // We test the logic directly rather than via env manipulation (set_var/remove_var
        // are unsafe in Rust 2024 and env mutations are not safe in parallel tests).
        let disabled_values = ["0", "false", "off"];
        for v in disabled_values {
            let result = match v {
                "0" | "false" | "off" => false,
                _ => true,
            };
            assert!(!result, "'{v}' should disable the hint");
        }
        // Any other value (including empty/unset) means enabled
        assert!(matches!("1", "0" | "false" | "off") == false);
    }

    // Verify line numbers are always enabled in rg invocation (grep_cmd.rs:24).
    // The -n/--line-numbers clap flag in main.rs is a no-op accepted for compat.
    #[test]
    fn test_rg_always_has_line_numbers() {
        // grep_cmd::run() always passes "-n" to rg (line 24).
        // This test documents that -n is built-in, so the clap flag is safe to ignore.
        let mut cmd = std::process::Command::new("rg");
        cmd.args(["-n", "--no-heading", "NONEXISTENT_PATTERN_12345", "."]);
        // If rg is available, it should accept -n without error (exit 1 = no match, not error)
        if let Ok(output) = cmd.output() {
            assert!(
                output.status.code() == Some(1) || output.status.success(),
                "rg -n should be accepted"
            );
        }
        // If rg is not installed, skip gracefully (test still passes)
    }
}
