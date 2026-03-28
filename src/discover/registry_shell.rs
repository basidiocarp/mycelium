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
pub(super) fn split_task_runner_command(cmd: &str) -> Option<TaskRunnerCommand<'_>> {
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

pub(super) fn unwrap_task_runner_command(cmd: &str) -> Option<&str> {
    split_task_runner_command(cmd).map(|command| command.inner)
}

pub(super) fn unwrap_all_task_runner_commands(mut cmd: &str) -> &str {
    while let Some(wrapper) = split_task_runner_command(cmd) {
        cmd = wrapper.inner;
    }
    cmd
}

pub(super) fn strip_env_prefix_segments(cmd: &str) -> (String, String) {
    let stripped_cow = env_prefix().replace(cmd.trim(), "");
    let env_prefix_len = cmd.trim().len() - stripped_cow.len();
    let trimmed = cmd.trim();
    (
        trimmed[..env_prefix_len].to_string(),
        stripped_cow.trim().to_string(),
    )
}

pub(super) fn contains_unquoted_sequence(cmd: &str, pattern: &[u8]) -> bool {
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

pub(super) fn split_shell_words(cmd: &str) -> Vec<String> {
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

pub(super) fn command_has_structured_gh_output(cmd: &str) -> bool {
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

pub(super) fn has_unsafe_shell_syntax(cmd: &str) -> bool {
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
            b'`' => return true,
            b'$' if i + 1 < len && bytes[i + 1] == b'(' => return true,
            b'|' => {
                if i + 1 < len && bytes[i + 1] == b'|' {
                    i += 2;
                } else {
                    return true;
                }
            }
            b'<' | b'>' | b'(' | b')' | b'{' | b'}' => return true,
            _ => {
                i += 1;
            }
        }
    }

    false
}

pub(super) fn needs_shell_parser_fallback(cmd: &str) -> bool {
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

pub(super) fn has_unsupported_shell_quoting(cmd: &str) -> bool {
    contains_unquoted_sequence(cmd, b"$'") || contains_unquoted_sequence(cmd, b"$\"")
}
