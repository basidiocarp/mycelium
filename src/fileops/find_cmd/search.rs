//! Gitignore-aware file search engine and output formatting.
use super::parser::glob_match;
use crate::tracking;
use anyhow::Result;
use ignore::WalkBuilder;
use std::collections::HashMap;
use std::path::Path;

/// Find files matching a glob pattern using gitignore-aware traversal, grouped by directory.
pub fn run(
    pattern: &str,
    path: &str,
    max_results: usize,
    max_depth: Option<usize>,
    file_type: &str,
    case_insensitive: bool,
    verbose: u8,
) -> Result<()> {
    let timer = tracking::TimedExecution::start();

    let effective_pattern = if pattern == "." { "*" } else { pattern };

    if verbose > 0 {
        eprintln!("find: {} in {}", effective_pattern, path);
    }

    let want_dirs = file_type == "d";

    let mut builder = WalkBuilder::new(path);
    builder
        .hidden(true)
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true);
    if let Some(depth) = max_depth {
        builder.max_depth(Some(depth));
    }
    let walker = builder.build();

    let mut files: Vec<String> = Vec::new();

    for entry in walker {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };

        let ft = entry.file_type();
        let is_dir = ft.as_ref().is_some_and(|t| t.is_dir());

        if want_dirs && !is_dir {
            continue;
        }
        if !want_dirs && is_dir {
            continue;
        }

        let entry_path = entry.path();

        let name = match entry_path.file_name() {
            Some(n) => n.to_string_lossy(),
            None => continue,
        };

        let matches = if case_insensitive {
            glob_match(&effective_pattern.to_lowercase(), &name.to_lowercase())
        } else {
            glob_match(effective_pattern, &name)
        };
        if !matches {
            continue;
        }

        let display_path = entry_path
            .strip_prefix(path)
            .unwrap_or(entry_path)
            .to_string_lossy()
            .to_string();

        if !display_path.is_empty() {
            files.push(display_path);
        }
    }

    files.sort();

    let raw_output = files.join("\n");

    if files.is_empty() {
        let msg = format!("0 for '{}'", effective_pattern);
        println!("{}", msg);
        timer.track(
            &format!("find {} -name '{}'", path, effective_pattern),
            "mycelium find",
            &raw_output,
            &msg,
        );
        return Ok(());
    }

    let mut by_dir: HashMap<String, Vec<String>> = HashMap::new();

    for file in &files {
        let p = Path::new(file);
        let dir = p
            .parent()
            .map(|d| d.to_string_lossy().to_string())
            .unwrap_or_else(|| ".".to_string());
        let dir = if dir.is_empty() { ".".to_string() } else { dir };
        let filename = p
            .file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_default();
        by_dir.entry(dir).or_default().push(filename);
    }

    let mut dirs: Vec<_> = by_dir.keys().cloned().collect();
    dirs.sort();
    let dirs_count = dirs.len();
    let total_files = files.len();

    println!("📁 {}F {}D:", total_files, dirs_count);
    println!();

    let mut shown = 0;
    for dir in &dirs {
        if shown >= max_results {
            break;
        }

        let files_in_dir = &by_dir[dir];
        let dir_display = if dir.len() > 50 {
            format!("...{}", &dir[dir.len() - 47..])
        } else {
            dir.clone()
        };

        let remaining_budget = max_results - shown;
        if files_in_dir.len() <= remaining_budget {
            println!("{}/ {}", dir_display, files_in_dir.join(" "));
            shown += files_in_dir.len();
        } else {
            let partial: Vec<_> = files_in_dir
                .iter()
                .take(remaining_budget)
                .cloned()
                .collect();
            println!("{}/ {}", dir_display, partial.join(" "));
            shown += partial.len();
            break;
        }
    }

    if shown < total_files {
        println!("+{} more", total_files - shown);
    }

    let mut by_ext: HashMap<String, usize> = HashMap::new();
    for file in &files {
        let ext = Path::new(file)
            .extension()
            .map(|e| e.to_string_lossy().to_string())
            .unwrap_or_else(|| "none".to_string());
        *by_ext.entry(ext).or_default() += 1;
    }

    let mut ext_line = String::new();
    if by_ext.len() > 1 {
        println!();
        let mut exts: Vec<_> = by_ext.iter().collect();
        exts.sort_by(|a, b| b.1.cmp(a.1));
        let ext_str: Vec<String> = exts
            .iter()
            .take(5)
            .map(|(e, c)| format!(".{}({})", e, c))
            .collect();
        ext_line = format!("ext: {}", ext_str.join(" "));
        println!("{}", ext_line);
    }

    let mycelium_output = format!("{}F {}D + {}", total_files, dirs_count, ext_line);
    timer.track(
        &format!("find {} -name '{}'", path, effective_pattern),
        "mycelium find",
        &raw_output,
        &mycelium_output,
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fileops::find_cmd::parser::parse_find_args;

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn run_from_args_native_find_syntax() {
        let parsed = parse_find_args(&args(&[".", "-name", "*.rs", "-type", "f"])).unwrap();
        let result = run(
            &parsed.pattern,
            &parsed.path,
            parsed.max_results,
            parsed.max_depth,
            &parsed.file_type,
            parsed.case_insensitive,
            0,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn run_from_args_mycelium_syntax() {
        let parsed = parse_find_args(&args(&["*.rs", "src"])).unwrap();
        let result = run(
            &parsed.pattern,
            &parsed.path,
            parsed.max_results,
            parsed.max_depth,
            &parsed.file_type,
            parsed.case_insensitive,
            0,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn run_from_args_iname_case_insensitive() {
        let parsed = parse_find_args(&args(&[".", "-iname", "cargo.toml"])).unwrap();
        let result = run(
            &parsed.pattern,
            &parsed.path,
            parsed.max_results,
            parsed.max_depth,
            &parsed.file_type,
            parsed.case_insensitive,
            0,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn find_rs_files_in_src() {
        let result = run("*.rs", "src", 100, None, "f", false, 0);
        assert!(result.is_ok());
    }

    #[test]
    fn find_dot_pattern_works() {
        let result = run(".", "src", 10, None, "f", false, 0);
        assert!(result.is_ok());
    }

    #[test]
    fn find_no_matches() {
        let result = run("*.xyz_nonexistent", "src", 50, None, "f", false, 0);
        assert!(result.is_ok());
    }

    #[test]
    fn find_respects_max() {
        let result = run("*.rs", "src", 2, None, "f", false, 0);
        assert!(result.is_ok());
    }

    #[test]
    fn find_gitignored_excluded() {
        let result = run("*", ".", 1000, None, "f", false, 0);
        assert!(result.is_ok());
    }
}
