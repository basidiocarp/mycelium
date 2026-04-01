//! Language-aware code filtering with configurable levels (none, minimal, aggressive).
use regex::Regex;
use std::str::FromStr;
use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterLevel {
    None,
    Minimal,
    Aggressive,
}

impl FromStr for FilterLevel {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "none" => Ok(FilterLevel::None),
            "minimal" => Ok(FilterLevel::Minimal),
            "aggressive" => Ok(FilterLevel::Aggressive),
            _ => Err(format!("Unknown filter level: {}", s)),
        }
    }
}

impl std::fmt::Display for FilterLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FilterLevel::None => write!(f, "none"),
            FilterLevel::Minimal => write!(f, "minimal"),
            FilterLevel::Aggressive => write!(f, "aggressive"),
        }
    }
}

/// Quality signal reported by a filter after processing.
#[derive(Debug, Clone, Copy, PartialEq)]
#[allow(dead_code)] // Used in Step 5 when filters migrate to filter_with_quality
pub enum FilterQuality {
    /// Filter understood the output format and produced structured compression.
    Full,
    /// Filter partially matched — some structure extracted, some raw passthrough.
    Degraded,
    /// Filter didn't understand the output — raw passthrough returned.
    Passthrough,
}

/// Result of a filter operation, including quality metadata.
#[allow(dead_code)] // Used in Step 5 when filters migrate to filter_with_quality
pub struct FilterResult {
    pub output: String,
    pub quality: FilterQuality,
    pub input_tokens: usize,
    pub output_tokens: usize,
}

pub trait FilterStrategy {
    /// Filter content and return quality metadata.
    ///
    /// The default implementation wraps [`FilterStrategy::filter`] and reports
    /// [`FilterQuality::Full`]. Individual filters can override this to return
    /// accurate quality signals.
    #[allow(dead_code)] // Will be called by route_or_filter in Step 5
    fn filter_with_quality(&self, content: &str, lang: &Language) -> FilterResult {
        let input_tokens = crate::tracking::utils::estimate_tokens(content);
        let output = self.filter(content, lang);
        let output_tokens = crate::tracking::utils::estimate_tokens(&output);
        FilterResult {
            output,
            quality: FilterQuality::Full,
            input_tokens,
            output_tokens,
        }
    }

    fn filter(&self, content: &str, lang: &Language) -> String;

    #[allow(dead_code)]
    fn name(&self) -> &'static str;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    Rust,
    Python,
    JavaScript,
    TypeScript,
    Go,
    C,
    Cpp,
    Java,
    Ruby,
    Shell,
    Unknown,
}

impl Language {
    pub fn from_extension(ext: &str) -> Self {
        match ext.to_lowercase().as_str() {
            "rs" => Language::Rust,
            "py" | "pyw" => Language::Python,
            "js" | "mjs" | "cjs" => Language::JavaScript,
            "ts" | "tsx" => Language::TypeScript,
            "go" => Language::Go,
            "c" | "h" => Language::C,
            "cpp" | "cc" | "cxx" | "hpp" | "hh" => Language::Cpp,
            "java" => Language::Java,
            "rb" => Language::Ruby,
            "sh" | "bash" | "zsh" => Language::Shell,
            _ => Language::Unknown,
        }
    }

    pub fn comment_patterns(&self) -> CommentPatterns {
        match self {
            Language::Rust => CommentPatterns {
                line: Some("//"),
                block_start: Some("/*"),
                block_end: Some("*/"),
                doc_line: Some("///"),
                doc_block_start: Some("/**"),
            },
            Language::Python => CommentPatterns {
                line: Some("#"),
                block_start: Some("\"\"\""),
                block_end: Some("\"\"\""),
                doc_line: None,
                doc_block_start: Some("\"\"\""),
            },
            Language::JavaScript
            | Language::TypeScript
            | Language::Go
            | Language::C
            | Language::Cpp
            | Language::Java => CommentPatterns {
                line: Some("//"),
                block_start: Some("/*"),
                block_end: Some("*/"),
                doc_line: None,
                doc_block_start: Some("/**"),
            },
            Language::Ruby => CommentPatterns {
                line: Some("#"),
                block_start: Some("=begin"),
                block_end: Some("=end"),
                doc_line: None,
                doc_block_start: None,
            },
            Language::Shell => CommentPatterns {
                line: Some("#"),
                block_start: None,
                block_end: None,
                doc_line: None,
                doc_block_start: None,
            },
            Language::Unknown => CommentPatterns {
                line: Some("//"),
                block_start: Some("/*"),
                block_end: Some("*/"),
                doc_line: None,
                doc_block_start: None,
            },
        }
    }
}

#[derive(Debug, Clone)]
pub struct CommentPatterns {
    pub line: Option<&'static str>,
    pub block_start: Option<&'static str>,
    pub block_end: Option<&'static str>,
    pub doc_line: Option<&'static str>,
    pub doc_block_start: Option<&'static str>,
}

pub struct NoFilter;

impl FilterStrategy for NoFilter {
    fn filter(&self, content: &str, _lang: &Language) -> String {
        content.to_string()
    }

    fn name(&self) -> &'static str {
        "none"
    }
}

pub struct MinimalFilter;

#[allow(dead_code)]
pub(crate) fn is_actionable_comment(line: &str) -> bool {
    let upper = line.to_uppercase();
    upper.contains("TODO")
        || upper.contains("FIXME")
        || upper.contains("HACK")
        || upper.contains("SAFETY")
        || upper.contains("NOTE")
        || upper.contains("XXX")
        || upper.contains("BUG")
        || upper.contains("WARNING")
}

fn is_noise_comment(line: &str) -> bool {
    let trimmed = line.trim();
    // Separator lines: only comment chars + separator chars
    let is_separator = trimmed
        .chars()
        .all(|c| matches!(c, '/' | '#' | '=' | '-' | '*' | '~' | ' ' | '\t'));
    if is_separator && trimmed.len() > 3 {
        return true;
    }
    // Auto-generated markers
    if trimmed.contains("Code generated")
        || trimmed.contains("DO NOT EDIT")
        || trimmed.contains("@generated")
        || trimmed.contains("AUTO-GENERATED")
    {
        return true;
    }
    // Pragma/lint directives
    if trimmed.contains("eslint-disable")
        || trimmed.contains("type: ignore")
        || trimmed.contains("nolint")
        || trimmed.contains("#pragma")
    {
        return true;
    }
    false
}

fn multiple_blank_lines() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\n{3,}").expect("valid regex"))
}

#[allow(dead_code)]
fn trailing_whitespace() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"[ \t]+$").expect("valid regex"))
}

impl FilterStrategy for MinimalFilter {
    fn filter(&self, content: &str, lang: &Language) -> String {
        let patterns = lang.comment_patterns();
        let mut result = String::with_capacity(content.len());
        let mut in_block_comment = false;
        let mut block_buf: Vec<String> = Vec::new();
        let mut in_docstring = false;
        // Track preamble to detect and strip license headers (>3 consecutive comment
        // lines before any code).
        let mut code_seen = false;
        let mut preamble_buf: Vec<String> = Vec::new();

        for line in content.lines() {
            let trimmed = line.trim();

            // Handle block comments
            if let (Some(start), Some(end)) = (patterns.block_start, patterns.block_end) {
                if !in_docstring
                    && !in_block_comment
                    && trimmed.contains(start)
                    && !trimmed.starts_with(patterns.doc_block_start.unwrap_or("###"))
                {
                    in_block_comment = true;
                    block_buf.clear();
                }
                if in_block_comment {
                    block_buf.push(line.to_string());
                    if trimmed.contains(end) {
                        in_block_comment = false;
                        // Keep block unless every line (stripped of markers) is noise.
                        let all_noise = block_buf.iter().all(|l| {
                            let stripped = l
                                .trim()
                                .trim_start_matches("/**")
                                .trim_start_matches("/*")
                                .trim_end_matches("*/")
                                .trim_start_matches('*')
                                .trim();
                            stripped.is_empty()
                                || is_noise_comment(stripped)
                                || is_noise_comment(l.trim())
                        });
                        if !all_noise {
                            if code_seen {
                                for l in block_buf.drain(..) {
                                    result.push_str(&l);
                                    result.push('\n');
                                }
                            } else {
                                preamble_buf.append(&mut block_buf);
                            }
                        } else {
                            block_buf.clear();
                        }
                    }
                    continue;
                }
            }

            // Handle Python docstrings (keep them in minimal mode)
            if *lang == Language::Python && trimmed.starts_with("\"\"\"") {
                in_docstring = !in_docstring;
                result.push_str(line);
                result.push('\n');
                continue;
            }

            if in_docstring {
                result.push_str(line);
                result.push('\n');
                continue;
            }

            // Handle single-line comments
            if let Some(line_comment) = patterns.line
                && trimmed.starts_with(line_comment)
            {
                // Always keep doc comments (/// and //!)
                let is_doc = patterns.doc_line.is_some_and(|d| trimmed.starts_with(d))
                    || (*lang == Language::Rust && trimmed.starts_with("//!"));
                if is_doc {
                    result.push_str(line);
                    result.push('\n');
                    continue;
                }

                // In preamble: buffer for license-header detection
                if !code_seen {
                    preamble_buf.push(line.to_string());
                    continue;
                }

                // After code has started: only drop noise
                if is_noise_comment(trimmed) {
                    continue;
                }

                // Keep actionable comments and all other regular comments
                result.push_str(line);
                result.push('\n');
                continue;
            }

            // Empty lines
            if trimmed.is_empty() {
                // An empty line ends the current preamble run — flush or discard
                if !code_seen && !preamble_buf.is_empty() {
                    if preamble_buf.len() > 3 {
                        preamble_buf.clear();
                    } else {
                        for l in preamble_buf.drain(..) {
                            result.push_str(&l);
                            result.push('\n');
                        }
                    }
                }
                result.push('\n');
                continue;
            }

            // Non-comment, non-empty code line: finalise preamble
            if !code_seen {
                if preamble_buf.len() > 3 {
                    preamble_buf.clear();
                } else {
                    for l in preamble_buf.drain(..) {
                        result.push_str(&l);
                        result.push('\n');
                    }
                }
                code_seen = true;
            }

            result.push_str(line);
            result.push('\n');
        }

        // Flush any remaining preamble (file that ends without real code)
        if !code_seen && preamble_buf.len() <= 3 {
            for l in preamble_buf.drain(..) {
                result.push_str(&l);
                result.push('\n');
            }
        }

        // Normalize multiple blank lines to max 2
        let result = multiple_blank_lines().replace_all(&result, "\n\n");
        result.trim().to_string()
    }

    fn name(&self) -> &'static str {
        "minimal"
    }
}

pub struct AggressiveFilter;

fn import_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^(use |import |from |require\(|#include)").expect("valid regex"))
}

fn func_signature() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
        r"^(pub\s+)?(async\s+)?(fn|def|function|func|class|struct|enum|trait|interface|type)\s+\w+"
    ).expect("regex: definition detection")
    })
}

impl FilterStrategy for AggressiveFilter {
    fn filter(&self, content: &str, lang: &Language) -> String {
        let minimal = MinimalFilter.filter(content, lang);
        let mut result = String::with_capacity(minimal.len() / 2);
        let mut brace_depth: i32 = 0;
        let mut in_impl_body = false;
        let mut impl_body_buf: Vec<String> = Vec::new();

        for line in minimal.lines() {
            let trimmed = line.trim();

            // Always keep imports
            if import_pattern().is_match(trimmed) {
                result.push_str(line);
                result.push('\n');
                continue;
            }

            // Always keep function/struct/class signatures
            if func_signature().is_match(trimmed) {
                result.push_str(line);
                result.push('\n');
                in_impl_body = true;
                brace_depth = 0;
                impl_body_buf.clear();
                continue;
            }

            // Track brace depth for implementation bodies
            let open_braces = trimmed.matches('{').count();
            let close_braces = trimmed.matches('}').count();

            if in_impl_body {
                brace_depth += open_braces as i32;
                brace_depth -= close_braces as i32;

                impl_body_buf.push(line.to_string());

                if brace_depth <= 0 {
                    in_impl_body = false;
                    if impl_body_buf.len() <= 30 {
                        for l in impl_body_buf.drain(..) {
                            result.push_str(&l);
                            result.push('\n');
                        }
                    } else {
                        let n = impl_body_buf.len();
                        impl_body_buf.clear();
                        result.push_str(&format!("    // ... ({n} lines)\n"));
                    }
                }
                continue;
            }

            // Keep type definitions, constants, etc.
            if trimmed.starts_with("const ")
                || trimmed.starts_with("static ")
                || trimmed.starts_with("let ")
                || trimmed.starts_with("pub const ")
                || trimmed.starts_with("pub static ")
            {
                result.push_str(line);
                result.push('\n');
            }
        }

        result.trim().to_string()
    }

    fn name(&self) -> &'static str {
        "aggressive"
    }
}

pub fn get_filter(level: FilterLevel) -> Box<dyn FilterStrategy> {
    match level {
        FilterLevel::None => Box::new(NoFilter),
        FilterLevel::Minimal => Box::new(MinimalFilter),
        FilterLevel::Aggressive => Box::new(AggressiveFilter),
    }
}

pub fn smart_truncate(content: &str, max_lines: usize, _lang: &Language) -> String {
    let lines: Vec<&str> = content.lines().collect();
    if lines.len() <= max_lines {
        return content.to_string();
    }

    let mut result = Vec::with_capacity(max_lines);
    let mut kept_lines = 0;
    let mut skipped_count = 0;

    for line in &lines {
        let trimmed = line.trim();

        // Always keep signatures and important structural elements
        let is_important = func_signature().is_match(trimmed)
            || import_pattern().is_match(trimmed)
            || trimmed.starts_with("pub ")
            || trimmed.starts_with("export ")
            || trimmed == "}"
            || trimmed == "{";

        if is_important || kept_lines < max_lines / 2 {
            // Emit omission marker when transitioning from skip to keep
            if skipped_count > 0 {
                result.push(format!("    // ... {} lines omitted", skipped_count));
                skipped_count = 0;
            }
            result.push((*line).to_string());
            kept_lines += 1;
        } else {
            // Track each skipped line
            skipped_count += 1;
        }

        if kept_lines >= max_lines - 1 {
            break;
        }
    }

    // Emit final summary line if we skipped anything or reached the limit
    if skipped_count > 0 || kept_lines < lines.len() {
        let remaining = lines.len() - result.len();
        result.push(format!(
            "// ... {} more lines (total: {})",
            remaining,
            lines.len()
        ));
    }

    result.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filter_level_parsing() {
        assert_eq!(FilterLevel::from_str("none").unwrap(), FilterLevel::None);
        assert_eq!(
            FilterLevel::from_str("minimal").unwrap(),
            FilterLevel::Minimal
        );
        assert_eq!(
            FilterLevel::from_str("aggressive").unwrap(),
            FilterLevel::Aggressive
        );
    }

    #[test]
    fn test_language_detection() {
        assert_eq!(Language::from_extension("rs"), Language::Rust);
        assert_eq!(Language::from_extension("py"), Language::Python);
        assert_eq!(Language::from_extension("js"), Language::JavaScript);
    }

    #[test]
    fn test_minimal_filter_preserves_regular_comments() {
        // A single-line comment before code is short preamble (<=3) — kept.
        let code = "// This is a comment\nfn main() {\n    println!(\"Hello\");\n}\n";
        let filter = MinimalFilter;
        let result = filter.filter(code, &Language::Rust);
        assert!(
            result.contains("// This is a comment"),
            "regular comments should be kept; got:\n{result}"
        );
        assert!(result.contains("fn main()"));
    }

    #[test]
    fn test_minimal_filter_strips_noise_comments() {
        let code = "fn foo() {\n    // ========================\n    let x = 1;\n    x\n}\n";
        let filter = MinimalFilter;
        let result = filter.filter(code, &Language::Rust);
        assert!(
            !result.contains("// ========================"),
            "separator noise should be stripped; got:\n{result}"
        );
        assert!(result.contains("fn foo()"));
    }

    #[test]
    fn test_minimal_filter_strips_license_header() {
        let code = "// Copyright (c) 2024\n// All rights reserved\n// Licensed under MIT\n// See LICENSE file\nfn main() {}\n";
        let filter = MinimalFilter;
        let result = filter.filter(code, &Language::Rust);
        assert!(
            !result.contains("Copyright"),
            "4-line license header should be stripped; got:\n{result}"
        );
        assert!(result.contains("fn main()"));
    }

    #[test]
    fn test_minimal_filter_preserves_todo_comments() {
        let code = "fn foo() {\n    // TODO: fix this\n    let x = 1;\n    x\n}\n";
        let filter = MinimalFilter;
        let result = filter.filter(code, &Language::Rust);
        assert!(
            result.contains("// TODO: fix this"),
            "TODO comments should be kept; got:\n{result}"
        );
    }

    #[test]
    fn test_minimal_filter_snapshot_mixed_comments() {
        let code = concat!(
            "// Copyright (c) 2024 Example Corp\n",
            "// All rights reserved.\n",
            "// Licensed under the MIT License.\n",
            "// See LICENSE for details.\n",
            "\n",
            "use std::collections::HashMap;\n",
            "\n",
            "/// A simple counter struct.\n",
            "pub struct Counter {\n",
            "    count: usize,\n",
            "}\n",
            "\n",
            "impl Counter {\n",
            "    // TODO: add overflow protection\n",
            "    pub fn increment(&mut self) {\n",
            "        self.count += 1;\n",
            "    }\n",
            "\n",
            "    // ========================\n",
            "    // Regular comment about logic\n",
            "    pub fn get(&self) -> usize {\n",
            "        self.count\n",
            "    }\n",
            "}\n",
        );
        let filter = MinimalFilter;
        let result = filter.filter(code, &Language::Rust);
        insta::assert_snapshot!(result);
    }
}
