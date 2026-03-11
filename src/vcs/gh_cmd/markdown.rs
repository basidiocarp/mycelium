//! Markdown filtering utilities for GitHub CLI output.
//!
//! Removes noise (HTML comments, badges, image lines, horizontal rules)
//! while preserving meaningful content and code blocks.

use regex::Regex;
use std::sync::OnceLock;

fn html_comment_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?s)<!--.*?-->").expect("valid regex"))
}

fn badge_line_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?m)^\s*\[!\[[^]]*]\([^)]*\)]\([^)]*\)\s*$").expect("valid regex")
    })
}

fn image_only_line_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?m)^\s*!\[[^]]*]\([^)]*\)\s*$").expect("valid regex"))
}

fn horizontal_rule_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?m)^\s*(?:---+|\*\*\*+|___+)\s*$").expect("valid regex"))
}

fn multi_blank_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\n{3,}").expect("valid regex"))
}

/// Filter markdown body to remove noise while preserving meaningful content.
/// Removes HTML comments, badge lines, image-only lines, horizontal rules,
/// and collapses excessive blank lines. Preserves code blocks untouched.
pub(crate) fn filter_markdown_body(body: &str) -> String {
    if body.is_empty() {
        return String::new();
    }

    // Split into code blocks and non-code segments
    let mut result = String::new();
    let mut remaining = body;

    loop {
        // Find next code block opening (``` or ~~~)
        let fence_pos = remaining
            .find("```")
            .or_else(|| remaining.find("~~~"))
            .map(|pos| {
                let fence = if remaining[pos..].starts_with("```") {
                    "```"
                } else {
                    "~~~"
                };
                (pos, fence)
            });

        match fence_pos {
            Some((start, fence)) => {
                // Filter the text before the code block
                let before = &remaining[..start];
                result.push_str(&filter_markdown_segment(before));

                // Find the closing fence
                let after_open = start + fence.len();
                // Skip past the opening fence line
                let code_start = remaining[after_open..]
                    .find('\n')
                    .map(|p| after_open + p + 1)
                    .unwrap_or(remaining.len());

                let close_pos = remaining[code_start..]
                    .find(fence)
                    .map(|p| code_start + p + fence.len());

                match close_pos {
                    Some(end) => {
                        // Preserve the entire code block as-is
                        result.push_str(&remaining[start..end]);
                        // Include the rest of the closing fence line
                        let after_close = remaining[end..]
                            .find('\n')
                            .map(|p| end + p + 1)
                            .unwrap_or(remaining.len());
                        result.push_str(&remaining[end..after_close]);
                        remaining = &remaining[after_close..];
                    }
                    None => {
                        // Unclosed code block — preserve everything
                        result.push_str(&remaining[start..]);
                        remaining = "";
                    }
                }
            }
            None => {
                // No more code blocks, filter the rest
                result.push_str(&filter_markdown_segment(remaining));
                break;
            }
        }
    }

    // Final cleanup: trim trailing whitespace
    result.trim().to_string()
}

/// Filter a markdown segment that is NOT inside a code block.
pub(crate) fn filter_markdown_segment(text: &str) -> String {
    let mut s = html_comment_re().replace_all(text, "").to_string();
    s = badge_line_re().replace_all(&s, "").to_string();
    s = image_only_line_re().replace_all(&s, "").to_string();
    s = horizontal_rule_re().replace_all(&s, "").to_string();
    s = multi_blank_re().replace_all(&s, "\n\n").to_string();
    s
}
