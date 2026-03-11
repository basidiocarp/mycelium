//! npm run script proxy with token tracking.
use crate::filtered_cmd::FilteredCommand;
use anyhow::Result;

/// Run an npm script and filter output to strip boilerplate and warnings.
pub fn run(args: &[String], verbose: u8, skip_env: bool) -> Result<()> {
    let all_args: Vec<String> = std::iter::once("run".to_string())
        .chain(args.iter().cloned())
        .collect();

    let mut cmd = FilteredCommand::new("npm")
        .args(all_args)
        .verbose(verbose)
        .filter(filter_npm_output);

    if skip_env {
        cmd = cmd.env("SKIP_ENV_VALIDATION", "1");
    }

    cmd.run()
}

/// Filter npm run output - strip boilerplate, progress bars, npm WARN
fn filter_npm_output(output: &str) -> String {
    let mut result = Vec::new();

    for line in output.lines() {
        // Skip npm boilerplate
        if line.starts_with('>') && line.contains('@') {
            continue;
        }
        // Skip npm lifecycle scripts
        if line.trim_start().starts_with("npm WARN") {
            continue;
        }
        if line.trim_start().starts_with("npm notice") {
            continue;
        }
        // Skip progress indicators
        if line.contains("⸩") || line.contains("⸨") || line.contains("...") && line.len() < 10 {
            continue;
        }
        // Skip empty lines
        if line.trim().is_empty() {
            continue;
        }

        result.push(line.to_string());
    }

    if result.is_empty() {
        "ok ✓".to_string()
    } else {
        result.join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filter_npm_output() {
        let output = r#"
> project@1.0.0 build
> next build

npm WARN deprecated inflight@1.0.6: This module is not supported
npm notice

   Creating an optimized production build...
   ✓ Build completed
"#;
        let result = filter_npm_output(output);
        assert!(!result.contains("npm WARN"));
        assert!(!result.contains("npm notice"));
        assert!(!result.contains("> project@"));
        assert!(result.contains("Build completed"));
    }

    #[test]
    fn test_filter_npm_output_empty() {
        let output = "\n\n\n";
        let result = filter_npm_output(output);
        assert_eq!(result, "ok ✓");
    }

    #[test]
    fn test_npm_token_savings() {
        fn count_tokens(text: &str) -> usize {
            text.split_whitespace().count()
        }

        let input = include_str!("../../tests/fixtures/npm_run_output_raw.txt");
        let output = filter_npm_output(input);
        let input_tokens = count_tokens(input);
        let output_tokens = count_tokens(&output);
        let savings = (input_tokens.saturating_sub(output_tokens)) * 100 / input_tokens.max(1);

        assert!(
            savings >= 60,
            "npm filter: expected >= 60% token savings, got {}%",
            savings
        );
    }
}
