use super::shared::format_crate_info;

/// Filter cargo install output - strip dep compilation, keep installed/replaced/errors
pub(crate) fn filter_cargo_install(output: &str) -> String {
    let mut errors: Vec<String> = Vec::new();
    let mut error_count = 0;
    let mut compiled = 0;
    let mut in_error = false;
    let mut current_error = Vec::new();
    let mut installed_crate = String::new();
    let mut installed_version = String::new();
    let mut replaced_lines: Vec<String> = Vec::new();
    let mut already_installed = false;
    let mut ignored_line = String::new();

    for line in output.lines() {
        let trimmed = line.trim_start();

        // Strip noise: dep compilation, downloading, locking, etc.
        if trimmed.starts_with("Compiling") {
            compiled += 1;
            continue;
        }
        if trimmed.starts_with("Downloading")
            || trimmed.starts_with("Downloaded")
            || trimmed.starts_with("Locking")
            || trimmed.starts_with("Updating")
            || trimmed.starts_with("Adding")
            || trimmed.starts_with("Finished")
            || trimmed.starts_with("Blocking waiting for file lock")
        {
            continue;
        }

        // Keep: Installing line (extract crate name + version)
        if trimmed.starts_with("Installing") {
            let rest = trimmed.strip_prefix("Installing").unwrap_or("").trim();
            if !rest.is_empty() && !rest.starts_with('/') {
                if let Some((name, version)) = rest.split_once(' ') {
                    installed_crate = name.to_string();
                    installed_version = version.to_string();
                } else {
                    installed_crate = rest.to_string();
                }
            }
            continue;
        }

        // Keep: Installed line (extract crate + version if not already set)
        if trimmed.starts_with("Installed") {
            let rest = trimmed.strip_prefix("Installed").unwrap_or("").trim();
            if !rest.is_empty() && installed_crate.is_empty() {
                let mut parts = rest.split_whitespace();
                if let (Some(name), Some(version)) = (parts.next(), parts.next()) {
                    installed_crate = name.to_string();
                    installed_version = version.to_string();
                }
            }
            continue;
        }

        // Keep: Replacing/Replaced lines
        if trimmed.starts_with("Replacing") || trimmed.starts_with("Replaced") {
            replaced_lines.push(trimmed.to_string());
            continue;
        }

        // Keep: "Ignored package" (already up to date)
        if trimmed.starts_with("Ignored package") {
            already_installed = true;
            ignored_line = trimmed.to_string();
            continue;
        }

        // Keep: actionable warnings (e.g., "be sure to add `/path` to your PATH")
        // Skip summary lines like "warning: `crate` generated N warnings"
        if line.starts_with("warning:") {
            if !(line.contains("generated") && line.contains("warning")) {
                replaced_lines.push(line.to_string());
            }
            continue;
        }

        // Detect error blocks
        if line.starts_with("error[") || line.starts_with("error:") {
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

    // Already installed / up to date
    if already_installed {
        let info = ignored_line.split('`').nth(1).unwrap_or(&ignored_line);
        return format!("✓ cargo install: {} already installed", info);
    }

    // Errors
    if error_count > 0 {
        let crate_info = format_crate_info(&installed_crate, &installed_version, "");
        let deps_info = if compiled > 0 {
            format!(", {} deps compiled", compiled)
        } else {
            String::new()
        };

        let mut result = String::new();
        if crate_info.is_empty() {
            result.push_str(&format!(
                "cargo install: {} error{}{}\n",
                error_count,
                if error_count > 1 { "s" } else { "" },
                deps_info
            ));
        } else {
            result.push_str(&format!(
                "cargo install: {} error{} ({}{})\n",
                error_count,
                if error_count > 1 { "s" } else { "" },
                crate_info,
                deps_info
            ));
        }
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

        return result.trim().to_string();
    }

    // Success
    let crate_info = format_crate_info(&installed_crate, &installed_version, "package");

    let mut result = format!(
        "✓ cargo install ({}, {} deps compiled)",
        crate_info, compiled
    );

    for line in &replaced_lines {
        result.push_str(&format!("\n  {}", line));
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filter_cargo_install_success() {
        let output = r#"  Installing mycelium v0.11.0
  Downloading crates ...
  Downloaded anyhow v1.0.80
  Downloaded clap v4.5.0
   Compiling libc v0.2.153
   Compiling cfg-if v1.0.0
   Compiling anyhow v1.0.80
   Compiling clap v4.5.0
   Compiling mycelium v0.11.0
    Finished `release` profile [optimized] target(s) in 45.23s
  Replacing /Users/user/.cargo/bin/mycelium
   Replaced package `mycelium v0.9.4` with `mycelium v0.11.0` (/Users/user/.cargo/bin/mycelium)
"#;
        let result = filter_cargo_install(output);
        assert!(result.contains("✓ cargo install"), "got: {}", result);
        assert!(result.contains("mycelium v0.11.0"), "got: {}", result);
        assert!(result.contains("5 deps compiled"), "got: {}", result);
        assert!(result.contains("Replaced"), "got: {}", result);
        assert!(!result.contains("Compiling"), "got: {}", result);
        assert!(!result.contains("Downloading"), "got: {}", result);
    }

    #[test]
    fn test_filter_cargo_install_replace() {
        let output = r#"  Installing mycelium v0.11.0
   Compiling mycelium v0.11.0
    Finished `release` profile [optimized] target(s) in 10.0s
  Replacing /Users/user/.cargo/bin/mycelium
   Replaced package `mycelium v0.9.4` with `mycelium v0.11.0` (/Users/user/.cargo/bin/mycelium)
"#;
        let result = filter_cargo_install(output);
        assert!(result.contains("✓ cargo install"), "got: {}", result);
        assert!(result.contains("Replacing"), "got: {}", result);
        assert!(result.contains("Replaced"), "got: {}", result);
    }

    #[test]
    fn test_filter_cargo_install_error() {
        let output = r#"  Installing mycelium v0.11.0
   Compiling mycelium v0.11.0
error[E0308]: mismatched types
 --> src/main.rs:10:5
  |
10|     "hello"
  |     ^^^^^^^ expected `i32`, found `&str`

error: aborting due to 1 previous error
"#;
        let result = filter_cargo_install(output);
        assert!(result.contains("cargo install: 1 error"), "got: {}", result);
        assert!(result.contains("E0308"), "got: {}", result);
        assert!(result.contains("mismatched types"), "got: {}", result);
        assert!(!result.contains("aborting"), "got: {}", result);
    }

    #[test]
    fn test_filter_cargo_install_already_installed() {
        let output = r#"  Ignored package `mycelium v0.11.0`, is already installed
"#;
        let result = filter_cargo_install(output);
        assert!(result.contains("already installed"), "got: {}", result);
        assert!(result.contains("mycelium v0.11.0"), "got: {}", result);
    }

    #[test]
    fn test_filter_cargo_install_up_to_date() {
        let output = r#"  Ignored package `cargo-deb v2.1.0 (/Users/user/cargo-deb)`, is already installed
"#;
        let result = filter_cargo_install(output);
        assert!(result.contains("already installed"), "got: {}", result);
        assert!(result.contains("cargo-deb v2.1.0"), "got: {}", result);
    }

    #[test]
    fn test_filter_cargo_install_empty_output() {
        let result = filter_cargo_install("");
        assert!(result.contains("✓ cargo install"), "got: {}", result);
        assert!(result.contains("0 deps compiled"), "got: {}", result);
    }

    #[test]
    fn test_filter_cargo_install_path_warning() {
        let output = r#"  Installing mycelium v0.11.0
   Compiling mycelium v0.11.0
    Finished `release` profile [optimized] target(s) in 10.0s
  Replacing /Users/user/.cargo/bin/mycelium
   Replaced package `mycelium v0.9.4` with `mycelium v0.11.0` (/Users/user/.cargo/bin/mycelium)
warning: be sure to add `/Users/user/.cargo/bin` to your PATH
"#;
        let result = filter_cargo_install(output);
        assert!(result.contains("✓ cargo install"), "got: {}", result);
        assert!(
            result.contains("be sure to add"),
            "PATH warning should be kept: {}",
            result
        );
        assert!(result.contains("Replaced"), "got: {}", result);
    }

    #[test]
    fn test_filter_cargo_install_multiple_errors() {
        let output = r#"  Installing mycelium v0.11.0
   Compiling mycelium v0.11.0
error[E0308]: mismatched types
 --> src/main.rs:10:5
  |
10|     "hello"
  |     ^^^^^^^ expected `i32`, found `&str`

error[E0425]: cannot find value `foo`
 --> src/lib.rs:20:9
  |
20|     foo
  |     ^^^ not found in this scope

error: aborting due to 2 previous errors
"#;
        let result = filter_cargo_install(output);
        assert!(
            result.contains("2 errors"),
            "should show 2 errors: {}",
            result
        );
        assert!(result.contains("E0308"), "got: {}", result);
        assert!(result.contains("E0425"), "got: {}", result);
        assert!(!result.contains("aborting"), "got: {}", result);
    }

    #[test]
    fn test_filter_cargo_install_locking_and_blocking() {
        let output = r#"  Locking 45 packages to latest compatible versions
  Blocking waiting for file lock on package cache
  Downloading crates ...
  Downloaded serde v1.0.200
   Compiling serde v1.0.200
   Compiling mycelium v0.11.0
    Finished `release` profile [optimized] target(s) in 30.0s
  Installing mycelium v0.11.0
"#;
        let result = filter_cargo_install(output);
        assert!(result.contains("✓ cargo install"), "got: {}", result);
        assert!(!result.contains("Locking"), "got: {}", result);
        assert!(!result.contains("Blocking"), "got: {}", result);
        assert!(!result.contains("Downloading"), "got: {}", result);
    }

    #[test]
    fn test_filter_cargo_install_from_path() {
        let output = r#"  Installing /Users/user/projects/mycelium
   Compiling mycelium v0.11.0
    Finished `release` profile [optimized] target(s) in 10.0s
"#;
        let result = filter_cargo_install(output);
        // Path-based install: crate info not extracted from path
        assert!(result.contains("✓ cargo install"), "got: {}", result);
        assert!(result.contains("1 deps compiled"), "got: {}", result);
    }

    fn count_tokens(text: &str) -> usize {
        crate::tracking::estimate_tokens(text)
    }

    #[test]
    fn test_cargo_install_token_savings() {
        let input = include_str!("../../tests/fixtures/cargo_install_raw.txt");
        let output = filter_cargo_install(input);
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
