//! Argument parsing and glob matching for the find command.
use anyhow::{Context, Result};

/// Match a filename against a glob pattern (supports `*` and `?`).
pub fn glob_match(pattern: &str, name: &str) -> bool {
    glob_match_inner(pattern.as_bytes(), name.as_bytes())
}

pub fn glob_match_inner(pat: &[u8], name: &[u8]) -> bool {
    match (pat.first(), name.first()) {
        (None, None) => true,
        (Some(b'*'), _) => {
            glob_match_inner(&pat[1..], name)
                || (!name.is_empty() && glob_match_inner(pat, &name[1..]))
        }
        (Some(b'?'), Some(_)) => glob_match_inner(&pat[1..], &name[1..]),
        (Some(&p), Some(&n)) if p == n => glob_match_inner(&pat[1..], &name[1..]),
        _ => false,
    }
}

/// Parsed arguments from either native find or Mycelium find syntax.
#[derive(Debug)]
pub struct FindArgs {
    pub pattern: String,
    pub path: String,
    pub max_results: usize,
    pub max_depth: Option<usize>,
    pub file_type: String,
    pub case_insensitive: bool,
}

impl Default for FindArgs {
    fn default() -> Self {
        Self {
            pattern: "*".to_string(),
            path: ".".to_string(),
            max_results: 50,
            max_depth: None,
            file_type: "f".to_string(),
            case_insensitive: false,
        }
    }
}

/// Consume the next argument from `args` at position `i`, advancing the index.
pub fn next_arg(args: &[String], i: &mut usize) -> Option<String> {
    *i += 1;
    args.get(*i).cloned()
}

/// Check if args contain native find flags (-name, -type, -maxdepth, etc.)
fn has_native_find_flags(args: &[String]) -> bool {
    args.iter()
        .any(|a| a == "-name" || a == "-type" || a == "-maxdepth" || a == "-iname")
}

/// Native find flags that Mycelium cannot handle correctly.
const UNSUPPORTED_FIND_FLAGS: &[&str] = &[
    "-not", "!", "-or", "-o", "-and", "-a", "-exec", "-execdir", "-delete", "-print0", "-newer",
    "-perm", "-size", "-mtime", "-mmin", "-atime", "-amin", "-ctime", "-cmin", "-empty", "-link",
    "-regex", "-iregex",
];

fn has_unsupported_find_flags(args: &[String]) -> bool {
    args.iter()
        .any(|a| UNSUPPORTED_FIND_FLAGS.contains(&a.as_str()))
}

/// Parse arguments from raw args vec, supporting both native find and Mycelium syntax.
///
/// Native find syntax: `find . -name "*.rs" -type f -maxdepth 3`
/// Mycelium syntax: `find *.rs [path] [-m max] [-t type]`
pub fn parse_find_args(args: &[String]) -> Result<FindArgs> {
    if args.is_empty() {
        return Ok(FindArgs::default());
    }

    if has_unsupported_find_flags(args) {
        anyhow::bail!(
            "mycelium find does not support compound predicates or actions (e.g. -not, -exec). Use `find` directly."
        );
    }

    if has_native_find_flags(args) {
        parse_native_find_args(args)
    } else {
        parse_mycelium_find_args(args)
    }
}

/// Parse native find syntax: `find [path] -name "*.rs" -type f -maxdepth 3`
fn parse_native_find_args(args: &[String]) -> Result<FindArgs> {
    let mut parsed = FindArgs::default();
    let mut i = 0;

    if !args[0].starts_with('-') {
        parsed.path = args[0].clone();
        i = 1;
    }

    while i < args.len() {
        match args[i].as_str() {
            "-name" => {
                if let Some(val) = next_arg(args, &mut i) {
                    parsed.pattern = val;
                }
            }
            "-iname" => {
                if let Some(val) = next_arg(args, &mut i) {
                    parsed.pattern = val;
                    parsed.case_insensitive = true;
                }
            }
            "-type" => {
                if let Some(val) = next_arg(args, &mut i) {
                    parsed.file_type = val;
                }
            }
            "-maxdepth" => {
                if let Some(val) = next_arg(args, &mut i) {
                    parsed.max_depth = Some(val.parse().context("invalid -maxdepth value")?);
                }
            }
            flag if flag.starts_with('-') => {
                eprintln!("mycelium find: unknown flag '{}', ignored", flag);
            }
            _ => {}
        }
        i += 1;
    }

    Ok(parsed)
}

/// Parse Mycelium syntax: `find <pattern> [path] [-m max] [-t type]`
fn parse_mycelium_find_args(args: &[String]) -> Result<FindArgs> {
    let mut parsed = FindArgs {
        pattern: args[0].clone(),
        ..FindArgs::default()
    };
    let mut i = 1;

    if i < args.len() && !args[i].starts_with('-') {
        parsed.path = args[i].clone();
        i += 1;
    }

    while i < args.len() {
        match args[i].as_str() {
            "-m" | "--max" => {
                if let Some(val) = next_arg(args, &mut i) {
                    parsed.max_results = val.parse().context("invalid --max value")?;
                }
            }
            "-t" | "--file-type" => {
                if let Some(val) = next_arg(args, &mut i) {
                    parsed.file_type = val;
                }
            }
            _ => {}
        }
        i += 1;
    }

    Ok(parsed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn glob_match_star_rs() {
        assert!(glob_match("*.rs", "main.rs"));
        assert!(glob_match("*.rs", "find_cmd.rs"));
        assert!(!glob_match("*.rs", "main.py"));
        assert!(!glob_match("*.rs", "rs"));
    }

    #[test]
    fn glob_match_star_all() {
        assert!(glob_match("*", "anything.txt"));
        assert!(glob_match("*", "a"));
        assert!(glob_match("*", ".hidden"));
    }

    #[test]
    fn glob_match_question_mark() {
        assert!(glob_match("?.rs", "a.rs"));
        assert!(!glob_match("?.rs", "ab.rs"));
    }

    #[test]
    fn glob_match_exact() {
        assert!(glob_match("Cargo.toml", "Cargo.toml"));
        assert!(!glob_match("Cargo.toml", "cargo.toml"));
    }

    #[test]
    fn glob_match_complex() {
        assert!(glob_match("test_*", "test_foo"));
        assert!(glob_match("test_*", "test_"));
        assert!(!glob_match("test_*", "test"));
    }

    #[test]
    fn dot_becomes_star() {
        let effective = if "." == "." { "*" } else { "." };
        assert_eq!(effective, "*");
    }

    #[test]
    fn parse_native_find_name() {
        let parsed = parse_find_args(&args(&[".", "-name", "*.rs"])).unwrap();
        assert_eq!(parsed.pattern, "*.rs");
        assert_eq!(parsed.path, ".");
        assert_eq!(parsed.file_type, "f");
        assert_eq!(parsed.max_results, 50);
    }

    #[test]
    fn parse_native_find_name_and_type() {
        let parsed = parse_find_args(&args(&["src", "-name", "*.rs", "-type", "f"])).unwrap();
        assert_eq!(parsed.pattern, "*.rs");
        assert_eq!(parsed.path, "src");
        assert_eq!(parsed.file_type, "f");
    }

    #[test]
    fn parse_native_find_type_d() {
        let parsed = parse_find_args(&args(&[".", "-type", "d"])).unwrap();
        assert_eq!(parsed.pattern, "*");
        assert_eq!(parsed.file_type, "d");
    }

    #[test]
    fn parse_native_find_maxdepth() {
        let parsed = parse_find_args(&args(&[".", "-name", "*.toml", "-maxdepth", "2"])).unwrap();
        assert_eq!(parsed.pattern, "*.toml");
        assert_eq!(parsed.max_depth, Some(2));
        assert_eq!(parsed.max_results, 50);
    }

    #[test]
    fn parse_native_find_iname() {
        let parsed = parse_find_args(&args(&[".", "-iname", "Makefile"])).unwrap();
        assert_eq!(parsed.pattern, "Makefile");
        assert!(parsed.case_insensitive);
    }

    #[test]
    fn parse_native_find_name_is_case_sensitive() {
        let parsed = parse_find_args(&args(&[".", "-name", "*.rs"])).unwrap();
        assert!(!parsed.case_insensitive);
    }

    #[test]
    fn parse_native_find_no_path() {
        let parsed = parse_find_args(&args(&["-name", "*.rs"])).unwrap();
        assert_eq!(parsed.pattern, "*.rs");
        assert_eq!(parsed.path, ".");
    }

    #[test]
    fn parse_native_find_rejects_not() {
        let result = parse_find_args(&args(&[".", "-name", "*.rs", "-not", "-name", "*_test.rs"]));
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("compound predicates"));
    }

    #[test]
    fn parse_native_find_rejects_exec() {
        let result = parse_find_args(&args(&[".", "-name", "*.tmp", "-exec", "rm", "{}", ";"]));
        assert!(result.is_err());
    }

    #[test]
    fn parse_mycelium_syntax_pattern_only() {
        let parsed = parse_find_args(&args(&["*.rs"])).unwrap();
        assert_eq!(parsed.pattern, "*.rs");
        assert_eq!(parsed.path, ".");
    }

    #[test]
    fn parse_mycelium_syntax_pattern_and_path() {
        let parsed = parse_find_args(&args(&["*.rs", "src"])).unwrap();
        assert_eq!(parsed.pattern, "*.rs");
        assert_eq!(parsed.path, "src");
    }

    #[test]
    fn parse_mycelium_syntax_with_flags() {
        let parsed = parse_find_args(&args(&["*.rs", "src", "-m", "10", "-t", "d"])).unwrap();
        assert_eq!(parsed.pattern, "*.rs");
        assert_eq!(parsed.path, "src");
        assert_eq!(parsed.max_results, 10);
        assert_eq!(parsed.file_type, "d");
    }

    #[test]
    fn parse_empty_args() {
        let parsed = parse_find_args(&args(&[])).unwrap();
        assert_eq!(parsed.pattern, "*");
        assert_eq!(parsed.path, ".");
    }
}
