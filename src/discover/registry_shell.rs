use regex::Regex;
use std::sync::OnceLock;

pub(crate) struct TaskRunnerCommand<'a> {
    pub prefix: &'a str,
    pub inner: &'a str,
}

fn env_prefix() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"^(?:sudo\s+|env\s+|[A-Z_][A-Z0-9_]*=[^\s]*\s+)+").expect("valid regex")
    })
}

/// If a task runner is being used as a clear direct-execution wrapper, return the underlying command.
///
/// This intentionally only recognizes explicit raw-command forms rather than opaque recipe names.
pub(crate) fn split_task_runner_command(cmd: &str) -> Option<TaskRunnerCommand<'_>> {
    let trimmed = cmd.trim();
    if trimmed.is_empty() {
        return None;
    }

    if let Some(rest) = trimmed.strip_prefix("mise ") {
        let rest = rest.trim_start();
        if let Some(payload) = rest.strip_prefix("exec -- ") {
            return Some(TaskRunnerCommand {
                prefix: &trimmed[..trimmed.len() - payload.len()],
                inner: payload.trim_start(),
            });
        }
        if let Some(payload) = rest.strip_prefix("x -- ") {
            return Some(TaskRunnerCommand {
                prefix: &trimmed[..trimmed.len() - payload.len()],
                inner: payload.trim_start(),
            });
        }
        if let Some(payload) = rest.strip_prefix("exec ") {
            return Some(TaskRunnerCommand {
                prefix: &trimmed[..trimmed.len() - payload.len()],
                inner: payload.trim_start(),
            });
        }
        if let Some(payload) = rest.strip_prefix("x ") {
            return Some(TaskRunnerCommand {
                prefix: &trimmed[..trimmed.len() - payload.len()],
                inner: payload.trim_start(),
            });
        }
    }

    if let Some(rest) = trimmed.strip_prefix("just ") {
        let rest = rest.trim_start();
        if let Some(payload) = rest.strip_prefix("-- ") {
            return Some(TaskRunnerCommand {
                prefix: &trimmed[..trimmed.len() - payload.len()],
                inner: payload.trim_start(),
            });
        }
    }

    if let Some(rest) = trimmed.strip_prefix("task ") {
        let rest = rest.trim_start();
        if let Some(payload) = rest.strip_prefix("-- ") {
            return Some(TaskRunnerCommand {
                prefix: &trimmed[..trimmed.len() - payload.len()],
                inner: payload.trim_start(),
            });
        }
    }

    None
}

pub(crate) fn unwrap_task_runner_command(cmd: &str) -> Option<&str> {
    split_task_runner_command(cmd).map(|command| command.inner)
}

pub(crate) fn unwrap_all_task_runner_commands(mut cmd: &str) -> &str {
    while let Some(wrapper) = split_task_runner_command(cmd) {
        cmd = wrapper.inner;
    }
    cmd
}

/// Scan user-supplied transparent prefixes for a longest-match that appears at word boundary.
///
/// Returns `TaskRunnerCommand { prefix, inner }` if a prefix matches the start of `cmd` AND
/// the character right after the matched prefix is whitespace or end-of-string.
/// Empty/whitespace-only prefix entries are skipped.
/// The `prefix` slice includes the trailing whitespace; `inner = payload.trim_start()`.
///
/// Returns `None` if no match, if the payload is empty, or if all prefixes are empty.
pub(crate) fn split_transparent_prefix<'a>(
    cmd: &'a str,
    prefixes: &[String],
) -> Option<TaskRunnerCommand<'a>> {
    let trimmed = cmd.trim();
    if trimmed.is_empty() {
        return None;
    }

    // Find the longest matching prefix that respects word boundaries.
    let mut best_match: Option<(&str, usize)> = None; // (matched_prefix_str, matched_len)

    for prefix_str in prefixes {
        let prefix_trimmed = prefix_str.trim();
        if prefix_trimmed.is_empty() {
            continue;
        }

        if trimmed.starts_with(prefix_trimmed) {
            let after_prefix_idx = prefix_trimmed.len();
            // Check word boundary: char after prefix must be whitespace or end-of-string
            if after_prefix_idx >= trimmed.len()
                || trimmed.as_bytes()[after_prefix_idx].is_ascii_whitespace()
            {
                // Keep the longest match
                if best_match.is_none() || prefix_trimmed.len() > best_match.unwrap().1 {
                    best_match = Some((prefix_trimmed, prefix_trimmed.len()));
                }
            }
        }
    }

    let (_matched_prefix, matched_len) = best_match?;
    let rest = &trimmed[matched_len..].trim_start();

    if rest.is_empty() {
        return None;
    }

    // Compute the prefix slice to include the trailing whitespace.
    let payload_after_prefix = &trimmed[matched_len..];
    let inner_trimmed = payload_after_prefix.trim_start();
    let prefix_end_idx = trimmed.len() - inner_trimmed.len();
    let prefix_slice = &trimmed[..prefix_end_idx];

    Some(TaskRunnerCommand {
        prefix: prefix_slice,
        inner: inner_trimmed,
    })
}

pub(crate) fn strip_env_prefix_segments(cmd: &str) -> (String, String) {
    let stripped_cow = env_prefix().replace(cmd.trim(), "");
    let env_prefix_len = cmd.trim().len() - stripped_cow.len();
    let trimmed = cmd.trim();
    (
        trimmed[..env_prefix_len].to_string(),
        stripped_cow.trim().to_string(),
    )
}

pub(crate) fn contains_unquoted_sequence(cmd: &str, pattern: &[u8]) -> bool {
    let trimmed = cmd.trim();
    if trimmed.is_empty() || pattern.is_empty() {
        return false;
    }

    let bytes = trimmed.as_bytes();
    let len = bytes.len();
    let mut i = 0;
    let mut in_single = false;
    let mut in_double = false;
    let mut escaped = false;

    while i < len {
        let b = bytes[i];

        if escaped {
            escaped = false;
            i += 1;
            continue;
        }

        match b {
            b'\\' if !in_single => {
                escaped = true;
                i += 1;
                continue;
            }
            b'\'' if !in_double => {
                in_single = !in_single;
                i += 1;
                continue;
            }
            b'"' if !in_single => {
                in_double = !in_double;
                i += 1;
                continue;
            }
            _ if in_single || in_double => {
                i += 1;
                continue;
            }
            _ => {}
        }

        if i + pattern.len() <= len && &bytes[i..i + pattern.len()] == pattern {
            return true;
        }

        i += 1;
    }

    false
}

pub(crate) fn split_shell_words(cmd: &str) -> Vec<String> {
    let trimmed = cmd.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }

    let mut words = Vec::new();
    let mut current = String::new();
    let bytes = trimmed.as_bytes();
    let len = bytes.len();
    let mut i = 0;
    let mut in_single = false;
    let mut in_double = false;
    let mut escaped = false;

    while i < len {
        let b = bytes[i];

        if escaped {
            current.push(char::from(b));
            escaped = false;
            i += 1;
            continue;
        }

        match b {
            b'\\' if !in_single => {
                escaped = true;
            }
            b'\'' if !in_double => {
                in_single = !in_single;
            }
            b'"' if !in_single => {
                in_double = !in_double;
            }
            b if b.is_ascii_whitespace() && !in_single && !in_double => {
                if !current.is_empty() {
                    words.push(std::mem::take(&mut current));
                }
            }
            _ => current.push(char::from(b)),
        }

        i += 1;
    }

    if !current.is_empty() {
        words.push(current);
    }

    words
}

pub(crate) fn command_has_structured_gh_output(cmd: &str) -> bool {
    let (_, cmd_clean) = strip_env_prefix_segments(cmd);
    let effective = unwrap_all_task_runner_commands(&cmd_clean);
    let words = split_shell_words(effective);
    if !matches!(words.first().map(String::as_str), Some("gh")) {
        return false;
    }

    let flags_with_value = [
        "-R",
        "--repo",
        "-q",
        "--jq",
        "-t",
        "--template",
        "-S",
        "--search",
        "-L",
        "--limit",
        "-s",
        "--state",
        "-a",
        "--assignee",
        "-A",
        "--author",
        "-B",
        "--base",
        "-H",
        "--head",
        "-l",
        "--label",
        "-O",
        "--owner",
    ];
    let mut skip_next = false;

    for word in words.iter().skip(1) {
        if skip_next {
            skip_next = false;
            continue;
        }

        if matches!(word.as_str(), "--json" | "--jq" | "--template")
            || word.starts_with("--json=")
            || word.starts_with("--jq=")
            || word.starts_with("--template=")
        {
            return true;
        }

        if flags_with_value.contains(&word.as_str()) {
            skip_next = true;
        }
    }

    false
}

pub(crate) fn has_unsafe_shell_syntax(cmd: &str) -> bool {
    let trimmed = cmd.trim();
    if trimmed.is_empty() {
        return false;
    }

    let bytes = trimmed.as_bytes();
    let len = bytes.len();
    let mut i = 0;
    let mut in_single = false;
    let mut in_double = false;
    let mut escaped = false;

    while i < len {
        let b = bytes[i];

        if escaped {
            escaped = false;
            i += 1;
            continue;
        }

        match b {
            b'\\' if !in_single => {
                escaped = true;
                i += 1;
            }
            b'\'' if !in_double => {
                in_single = !in_single;
                i += 1;
            }
            b'"' if !in_single => {
                in_double = !in_double;
                i += 1;
            }
            _ if in_single || in_double => {
                i += 1;
            }
            b'$' if i + 1 < len && bytes[i + 1] == b'(' => return true,
            b'|' => {
                if i + 1 < len && bytes[i + 1] == b'|' {
                    i += 2;
                } else {
                    return true;
                }
            }
            b'`' | b'<' | b'>' | b'(' | b')' | b'{' | b'}' => return true,
            _ => {
                i += 1;
            }
        }
    }

    false
}

pub(crate) fn needs_shell_parser_fallback(cmd: &str) -> bool {
    let trimmed = cmd.trim();
    !trimmed.is_empty()
        && trimmed.bytes().any(|b| {
            matches!(
                b,
                b'\''
                    | b'"'
                    | b'\\'
                    | b'`'
                    | b'$'
                    | b'('
                    | b')'
                    | b'{'
                    | b'}'
                    | b'<'
                    | b'>'
                    | b'|'
                    | b'\n'
                    | b'\r'
            )
        })
}

pub(crate) fn has_unsupported_shell_quoting(cmd: &str) -> bool {
    contains_unquoted_sequence(cmd, b"$'") || contains_unquoted_sequence(cmd, b"$\"")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_transparent_prefix_simple_match() {
        let prefixes = vec!["rtk run".to_string()];
        let result = split_transparent_prefix("rtk run cargo test", &prefixes);
        assert!(result.is_some());
        let wrapper = result.unwrap();
        assert_eq!(wrapper.inner, "cargo test");
        assert_eq!(wrapper.prefix.trim(), "rtk run");
    }

    #[test]
    fn test_split_transparent_prefix_longest_match_wins() {
        let prefixes = vec![
            "docker".to_string(),
            "docker compose".to_string(),
            "docker compose exec web".to_string(), // The longest, most specific prefix
        ];
        let result = split_transparent_prefix("docker compose exec web cargo test", &prefixes);
        assert!(result.is_some());
        let wrapper = result.unwrap();
        assert_eq!(wrapper.inner, "cargo test");
        // The prefix includes trailing whitespace
        assert!(wrapper.prefix.contains("docker compose exec web"));
    }

    #[test]
    fn test_split_transparent_prefix_word_boundary_required() {
        let prefixes = vec!["do".to_string()];
        let result = split_transparent_prefix("docker run cargo test", &prefixes);
        assert!(result.is_none()); // "do" is not followed by whitespace in "docker"
    }

    #[test]
    fn test_split_transparent_prefix_exact_match_is_valid() {
        let prefixes = vec!["rtk run".to_string()];
        let result = split_transparent_prefix("rtk run", &prefixes);
        assert!(result.is_none()); // payload is empty after stripping
    }

    #[test]
    fn test_split_transparent_prefix_empty_prefixes() {
        let prefixes: Vec<String> = vec![];
        let result = split_transparent_prefix("rtk run cargo test", &prefixes);
        assert!(result.is_none());
    }

    #[test]
    fn test_split_transparent_prefix_whitespace_only_prefixes_skipped() {
        let prefixes = vec!["  ".to_string(), "rtk run".to_string()];
        let result = split_transparent_prefix("rtk run cargo test", &prefixes);
        assert!(result.is_some());
        let wrapper = result.unwrap();
        assert_eq!(wrapper.inner, "cargo test");
    }

    #[test]
    fn test_split_transparent_prefix_no_match() {
        let prefixes = vec!["rtk run".to_string()];
        let result = split_transparent_prefix("cargo test", &prefixes);
        assert!(result.is_none());
    }

    #[test]
    fn test_split_transparent_prefix_preserves_trailing_whitespace_in_prefix_slice() {
        let prefixes = vec!["rtk run".to_string()];
        let result = split_transparent_prefix("rtk run    cargo test", &prefixes);
        assert!(result.is_some());
        let wrapper = result.unwrap();
        assert_eq!(wrapper.inner, "cargo test");
        // Prefix slice includes the trailing whitespace between wrapper and payload
        assert!(wrapper.prefix.contains("rtk run"));
    }

    #[test]
    fn test_split_task_runner_command_unchanged() {
        // Verify that the existing task runner functions are unaffected
        let result = split_task_runner_command("mise exec -- cargo test");
        assert!(result.is_some());
        let wrapper = result.unwrap();
        assert_eq!(wrapper.inner, "cargo test");
    }
}
