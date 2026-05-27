//! Command rewriting engine — transforms shell commands to their Mycelium equivalents.
#[cfg(not(test))]
use crate::platform::command_on_path;
use crate::platform::render_shell_command;
use regex::Regex;
use std::sync::OnceLock;
#[cfg(test)]
use std::{cell::RefCell, rc::Rc};

use super::rules::RULES;

// Import the shared shell parsing modules
use super::registry::{parser, shell};

fn find_fd_rewrite_active() -> bool {
    #[cfg(test)]
    {
        find_fd_rewrite_active_for_tests().unwrap_or(false)
    }

    #[cfg(not(test))]
    {
        read_bool_env("MYCELIUM_FD_REWRITE_ENABLED", true) && command_on_path("fd")
    }
}

#[cfg(not(test))]
fn read_bool_env(name: &str, default: bool) -> bool {
    std::env::var(name).map_or(default, |value| {
        matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        )
    })
}

#[cfg(test)]
fn find_fd_rewrite_active_for_tests() -> Option<bool> {
    FIND_FD_REWRITE_ACTIVE_OVERRIDE.with(|value| *value.borrow())
}

#[cfg(test)]
pub(crate) struct FindFdRewriteGuard {
    previous: Option<bool>,
    _token: Rc<()>,
}

#[cfg(test)]
pub(crate) fn set_find_fd_rewrite_active_for_tests(active: bool) -> FindFdRewriteGuard {
    FIND_FD_REWRITE_ACTIVE_OVERRIDE.with(|value| {
        let previous = value.replace(Some(active));
        FindFdRewriteGuard {
            previous,
            _token: Rc::new(()),
        }
    })
}

#[cfg(test)]
impl Drop for FindFdRewriteGuard {
    fn drop(&mut self) {
        FIND_FD_REWRITE_ACTIVE_OVERRIDE.with(|value| {
            value.replace(self.previous);
        });
    }
}

#[cfg(test)]
thread_local! {
    static FIND_FD_REWRITE_ACTIVE_OVERRIDE: RefCell<Option<bool>> = const { RefCell::new(None) };
}

pub(crate) fn rewrite_primary_command(cmd: &str) -> Option<String> {
    let trimmed = cmd.trim();
    if trimmed.is_empty() {
        return None;
    }

    let (_, cmd_clean) = shell::strip_env_prefix_segments(trimmed);
    let effective = shell::unwrap_all_task_runner_commands(&cmd_clean);

    effective.split_whitespace().next().map(ToString::to_string)
}

fn rewrite_segment_passthrough_reason(cmd: &str, excluded: &[String]) -> Option<String> {
    let trimmed = cmd.trim();
    if trimmed.is_empty() {
        return Some("empty command".to_string());
    }

    let (env_prefix, _) = shell::strip_env_prefix_segments(trimmed);
    if env_prefix
        .split_whitespace()
        .any(|token| token == "MYCELIUM_DISABLED=1")
    {
        return Some("MYCELIUM_DISABLED=1 disables rewrite for this command".to_string());
    }

    if let Some(base) = rewrite_primary_command(trimmed)
        && excluded.iter().any(|entry| entry == &base)
    {
        return Some(format!("command base `{base}` is excluded by config"));
    }

    if shell::command_has_structured_gh_output(trimmed) {
        return Some(
            "gh --json/--jq/--template output is structured, so it is not rewritten".to_string(),
        );
    }

    // If a piped command's first segment is a diagnostic passthrough,
    // block rewriting so the whole pipe passes through raw shell.
    // Non-piped diagnostics are handled by rewrite_segment which routes
    // them to `mycelium invoke <cmd>`.
    if trimmed.contains('|') {
        let first_segment = trimmed.split('|').next().unwrap_or(trimmed).trim();
        if diagnostic_passthrough_base(first_segment) {
            return Some(format!(
                "first pipe segment `{first_segment}` is a diagnostic passthrough command"
            ));
        }
    }

    None
}

fn rewrite_shape_block_reason(cmd: &str) -> Option<String> {
    let trimmed = cmd.trim();
    if trimmed.is_empty() {
        return Some("empty command".to_string());
    }

    if parser::rewrite_shape_requires_parser(trimmed) {
        if !parser::parser_allows_rewrite_shape(trimmed) {
            return Some(
                "command contains shell constructs that require raw shell semantics".to_string(),
            );
        }

        return None;
    }

    if shell::contains_unquoted_sequence(trimmed, b"<<") {
        return Some("command contains a heredoc, so it is not rewritten".to_string());
    }

    if shell::contains_unquoted_sequence(trimmed, b"$((") {
        return Some("command contains arithmetic expansion, so it is not rewritten".to_string());
    }

    if shell::has_unsafe_shell_syntax(trimmed) {
        return Some(
            "command contains pipelines, redirections, or shell grouping that require raw shell semantics"
                .to_string(),
        );
    }

    None
}

pub(crate) fn rewrite_block_reason(cmd: &str, excluded: &[String]) -> Option<String> {
    if let Some(reason) = rewrite_shape_block_reason(cmd) {
        return Some(reason);
    }

    let trimmed = cmd.trim();
    let segments = super::registry::split_command_chain(trimmed);
    if segments.len() == 1 {
        return rewrite_segment_passthrough_reason(trimmed, excluded);
    }

    None
}

#[allow(
    dead_code,
    reason = "Binary rewrite resolution consumes this helper even though the library target does not"
)]
pub(crate) fn learned_correction_block_reason(cmd: &str, excluded: &[String]) -> Option<String> {
    if let Some(reason) = rewrite_shape_block_reason(cmd) {
        return Some(reason);
    }

    for segment in super::registry::split_command_chain(cmd) {
        if let Some(reason) = rewrite_segment_passthrough_reason(segment, excluded) {
            return Some(reason);
        }
    }

    None
}

/// Rewrite a raw command to its Mycelium equivalent.
///
/// Returns `Some(rewritten)` if the command has a Mycelium equivalent or is already Mycelium.
/// Returns `None` if the command is unsupported or ignored (hook should pass through).
///
/// Handles compound commands (`&&`, `||`, `;`) by rewriting each segment independently.
/// Piped commands are left unchanged because downstream stages expect raw stdout,
/// not Mycelium's summarized output.
#[must_use]
pub fn rewrite_command(cmd: &str, excluded: &[String]) -> Option<String> {
    let trimmed = cmd.trim();
    if trimmed.is_empty() {
        return None;
    }

    // Simple (non-compound) already-Mycelium command — return as-is.
    // For compound commands that start with "mycelium" (e.g. "mycelium git add . && cargo test"),
    // fall through to rewrite_compound so the remaining segments get rewritten.
    let has_compound = trimmed.contains("&&")
        || trimmed.contains("||")
        || trimmed.contains(';')
        || trimmed.contains('|')
        || trimmed.contains(" & ");
    if !has_compound && (trimmed.starts_with("mycelium ") || trimmed == "mycelium") {
        return Some(trimmed.to_string());
    }

    // Fast-path: if a piped command's first segment is a diagnostic passthrough,
    // skip rewriting so the whole pipe passes through raw shell.
    // Only applies to piped commands — non-piped diagnostics still get rewritten
    // to `mycelium invoke <cmd>` for proper routing.
    if trimmed.contains('|') {
        let first_pipe_segment = trimmed.split('|').next().unwrap_or(trimmed).trim();
        if diagnostic_passthrough_base(first_pipe_segment) {
            return None;
        }
    }

    if rewrite_block_reason(trimmed, excluded).is_some() {
        return None;
    }

    rewrite_compound(trimmed, excluded)
}

/// Rewrite a compound command (with `&&`, `||`, `;`, `|`) by rewriting each segment.
#[allow(clippy::too_many_lines)]
fn rewrite_compound(cmd: &str, excluded: &[String]) -> Option<String> {
    let bytes = cmd.as_bytes();
    let len = bytes.len();
    let mut result = String::with_capacity(len + 32);
    let mut any_changed = false;
    let mut seg_start = 0;
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
            b'|' if !in_single && !in_double => {
                if i + 1 < len && bytes[i + 1] == b'|' {
                    // `||` operator — rewrite left, continue
                    let seg = cmd[seg_start..i].trim();
                    let rewritten =
                        rewrite_segment(seg, excluded).unwrap_or_else(|| seg.to_string());
                    if rewritten != seg {
                        any_changed = true;
                    }
                    result.push_str(&rewritten);
                    result.push_str(" || ");
                    i += 2;
                    while i < len && bytes[i] == b' ' {
                        i += 1;
                    }
                    seg_start = i;
                } else {
                    // Piped commands should have been rejected before compound rewriting.
                    return None;
                }
            }
            b'&' if !in_single && !in_double && i + 1 < len && bytes[i + 1] == b'&' => {
                // `&&` operator — rewrite left, continue
                let seg = cmd[seg_start..i].trim();
                let rewritten = rewrite_segment(seg, excluded).unwrap_or_else(|| seg.to_string());
                if rewritten != seg {
                    any_changed = true;
                }
                result.push_str(&rewritten);
                result.push_str(" && ");
                i += 2;
                while i < len && bytes[i] == b' ' {
                    i += 1;
                }
                seg_start = i;
            }
            b'&' if !in_single && !in_double => {
                // #346: redirect detection — 2>&1 / >&2 (> before &) or &>file / &>>file (> after &)
                let is_redirect =
                    (i > 0 && bytes[i - 1] == b'>') || (i + 1 < len && bytes[i + 1] == b'>');
                if is_redirect {
                    i += 1;
                } else {
                    // single `&` background execution operator
                    let seg = cmd[seg_start..i].trim();
                    let rewritten =
                        rewrite_segment(seg, excluded).unwrap_or_else(|| seg.to_string());
                    if rewritten != seg {
                        any_changed = true;
                    }
                    result.push_str(&rewritten);
                    result.push_str(" & ");
                    i += 1;
                    while i < len && bytes[i] == b' ' {
                        i += 1;
                    }
                    seg_start = i;
                }
            }
            b';' if !in_single && !in_double => {
                // `;` separator
                let seg = cmd[seg_start..i].trim();
                let rewritten = rewrite_segment(seg, excluded).unwrap_or_else(|| seg.to_string());
                if rewritten != seg {
                    any_changed = true;
                }
                result.push_str(&rewritten);
                result.push(';');
                i += 1;
                while i < len && bytes[i] == b' ' {
                    i += 1;
                }
                if i < len {
                    result.push(' ');
                }
                seg_start = i;
            }
            _ => {
                i += 1;
            }
        }
    }

    // Last (or only) segment
    let seg = cmd[seg_start..len].trim();
    let rewritten = rewrite_segment(seg, excluded).unwrap_or_else(|| seg.to_string());
    if rewritten != seg {
        any_changed = true;
    }
    result.push_str(&rewritten);

    if any_changed { Some(result) } else { None }
}

/// Rewrite `head -N file` → `mycelium read file --max-lines N`.
/// Returns `None` if the command doesn't match this pattern (fall through to generic logic).
fn rewrite_head_numeric(cmd: &str) -> Option<String> {
    // Match: head -<digits> <file>  (with optional env prefix)
    fn head_n() -> &'static Regex {
        static RE: OnceLock<Regex> = OnceLock::new();
        RE.get_or_init(|| Regex::new(r"^head\s+-(\d+)\s+(.+)$").expect("valid regex"))
    }
    fn head_lines() -> &'static Regex {
        static RE: OnceLock<Regex> = OnceLock::new();
        RE.get_or_init(|| Regex::new(r"^head\s+--lines=(\d+)\s+(.+)$").expect("valid regex"))
    }
    if let Some(caps) = head_n().captures(cmd) {
        let n = caps.get(1)?.as_str();
        let file = caps.get(2)?.as_str();
        return Some(format!("mycelium read {file} --max-lines {n}"));
    }
    if let Some(caps) = head_lines().captures(cmd) {
        let n = caps.get(1)?.as_str();
        let file = caps.get(2)?.as_str();
        return Some(format!("mycelium read {file} --max-lines {n}"));
    }
    // head with any other flag (e.g. -c, -q): skip rewriting to avoid clap errors
    if cmd.starts_with("head -") {
        return None;
    }
    None
}

/// Rewrite a single (non-compound) command segment.
/// Returns `Some(rewritten)` if matched (including already-Mycelium passthrough).
/// Returns `None` if no match (caller uses original segment).
fn rewrite_segment(seg: &str, excluded: &[String]) -> Option<String> {
    let trimmed = seg.trim();
    if trimmed.is_empty() {
        return None;
    }

    // Already Mycelium — pass through unchanged
    if trimmed.starts_with("mycelium ") || trimmed == "mycelium" {
        return Some(trimmed.to_string());
    }

    if rewrite_segment_passthrough_reason(trimmed, excluded).is_some() {
        return None;
    }

    let (env_prefix, cmd_clean) = shell::strip_env_prefix_segments(trimmed);

    if let Some(wrapper) = shell::split_task_runner_command(&cmd_clean) {
        let rewritten = rewrite_segment(wrapper.inner, excluded)?;
        return Some(format!("{env_prefix}{}{rewritten}", wrapper.prefix));
    }

    if super::registry::is_diagnostic_passthrough_command(&cmd_clean) {
        return Some(format!("{env_prefix}mycelium invoke {cmd_clean}"));
    }

    if cmd_clean.starts_with("find ") && find_fd_rewrite_active() {
        return rewrite_find_to_fd(&cmd_clean).map(|rewritten| format!("{env_prefix}{rewritten}"));
    }

    // Special case: `head -N file` / `head --lines=N file` → `mycelium read file --max-lines N`
    // Must intercept before generic prefix replacement, which would produce `mycelium read -20 file`.
    // Only intercept when head has a flag (-N, --lines=N, -c, etc.); plain `head file` falls
    // through to the generic rewrite below and produces `mycelium read file` as expected.
    if cmd_clean.starts_with("head -") {
        return rewrite_head_numeric(&cmd_clean);
    }

    // Use classify_command for correct ignore/prefix handling
    let mycelium_equivalent = match super::registry::classify_command(&cmd_clean) {
        super::registry::Classification::Supported {
            mycelium_equivalent,
            ..
        } => {
            // Check if the base command is excluded from rewriting (#243)
            let base = rewrite_primary_command(trimmed)?;
            if excluded.iter().any(|e| e == &base) {
                return None;
            }
            mycelium_equivalent
        }
        _ => return None,
    };

    // Find the matching rule — pick the rule whose rewrite_prefixes actually match the input,
    // since multiple rules can share the same mycelium_cmd (e.g. diff + diffsitter).
    let rule = RULES.iter().find(|r| {
        r.mycelium_cmd == mycelium_equivalent
            && r.rewrite_prefixes
                .iter()
                .any(|&p| strip_word_prefix(&cmd_clean, p).is_some())
    })?;

    // Try each rewrite prefix (longest first) with word-boundary check
    for &prefix in rule.rewrite_prefixes {
        if let Some(rest) = strip_word_prefix(&cmd_clean, prefix) {
            let rewritten = if rest.is_empty() {
                format!("{}{}", env_prefix, rule.mycelium_cmd)
            } else {
                format!("{}{} {}", env_prefix, rule.mycelium_cmd, rest)
            };
            return Some(rewritten);
        }
    }

    None
}

fn rewrite_find_to_fd(cmd: &str) -> Option<String> {
    if !find_fd_rewrite_active() {
        return None;
    }

    let words = shell::split_shell_words(cmd);
    if !matches!(words.first().map(String::as_str), Some("find")) {
        return None;
    }

    let mut index = 1;
    let mut path = ".".to_string();
    if let Some(candidate) = words.get(index)
        && !candidate.starts_with('-')
    {
        path.clone_from(candidate);
        index += 1;
    }

    let mut name_pattern: Option<String> = None;
    let mut file_type: Option<String> = None;
    let mut max_depth: Option<String> = None;
    let mut min_depth: Option<String> = None;
    while index < words.len() {
        match words[index].as_str() {
            "-name" => {
                index += 1;
                name_pattern = words.get(index).cloned();
                name_pattern.as_ref()?;
            }
            "-type" => {
                index += 1;
                file_type = words.get(index).cloned();
                match file_type.as_deref() {
                    Some("f" | "d") => {}
                    _ => return None,
                }
            }
            "-maxdepth" => {
                index += 1;
                max_depth = words.get(index).cloned();
                max_depth.as_ref()?;
            }
            "-mindepth" => {
                index += 1;
                min_depth = words.get(index).cloned();
                min_depth.as_ref()?;
            }
            token
                if matches!(
                    token,
                    "-exec"
                        | "-delete"
                        | "-print0"
                        | "-newer"
                        | "-not"
                        | "!"
                        | "-o"
                        | "-or"
                        | "-a"
                        | "-and"
                        | "("
                        | ")"
                ) || token.starts_with("-exec")
                    || token.starts_with("-delete")
                    || token.starts_with("-print0")
                    || token.starts_with("-newer") =>
            {
                return None;
            }
            _ => return None,
        }
        index += 1;
    }

    let name_pattern = name_pattern?;
    let mut args = vec!["fd".to_string()];
    if let Some(extension) = extension_from_find_pattern(&name_pattern) {
        args.extend(["-e".to_string(), extension]);
    } else {
        args.extend(["--glob".to_string(), name_pattern]);
    }
    if let Some(file_type) = file_type {
        args.extend(["--type".to_string(), file_type]);
    }
    if let Some(max_depth_val) = max_depth {
        args.extend(["--max-depth".to_string(), max_depth_val]);
    }
    if let Some(min_depth_val) = min_depth {
        args.extend(["--min-depth".to_string(), min_depth_val]);
    }
    args.push(path);
    Some(render_shell_command(&args))
}

fn extension_from_find_pattern(pattern: &str) -> Option<String> {
    let extension = pattern.strip_prefix("*.")?;
    if extension.is_empty() || extension.chars().any(|c| "*?[]{}".contains(c)) {
        return None;
    }
    Some(extension.to_string())
}

fn diagnostic_passthrough_base(cmd: &str) -> bool {
    let (_, cmd_clean) = shell::strip_env_prefix_segments(cmd);
    let effective = shell::unwrap_all_task_runner_commands(&cmd_clean);
    let Some(base) = effective.split_whitespace().next() else {
        return false;
    };

    if super::registry::DIAGNOSTIC_PASSTHROUGH.contains(&base) {
        return true;
    }

    if base == "ls"
        && effective
            .split_whitespace()
            .skip(1)
            .any(|arg| arg.starts_with('-'))
    {
        return true;
    }

    false
}

/// Strip a command prefix with word-boundary check.
/// Returns the remainder of the command after the prefix, or `None` if no match.
fn strip_word_prefix<'a>(cmd: &'a str, prefix: &str) -> Option<&'a str> {
    if cmd == prefix {
        Some("")
    } else if cmd.len() > prefix.len()
        && cmd.starts_with(prefix)
        && cmd.as_bytes()[prefix.len()] == b' '
    {
        Some(cmd[prefix.len() + 1..].trim_start())
    } else {
        None
    }
}
