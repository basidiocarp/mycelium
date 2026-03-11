use std::collections::HashMap;

/// Groups diagnostic items by file path for compact display.
///
/// # Example
/// ```
/// let mut g = FileGrouper::new(10, 3);
/// g.add("src/main.rs", 42, "unused variable `x`");
/// g.add("src/main.rs", 55, "missing semicolon");
/// g.add("src/lib.rs", 10, "dead code");
/// println!("{}", g.format());
/// ```
pub struct FileGrouper {
    by_file: HashMap<String, Vec<(usize, String)>>,
    max_files: usize,
    max_items_per_file: usize,
}

impl FileGrouper {
    /// Create a new grouper with display limits.
    pub fn new(max_files: usize, max_items_per_file: usize) -> Self {
        Self {
            by_file: HashMap::new(),
            max_files,
            max_items_per_file,
        }
    }

    /// Add an item to the grouper.
    pub fn add(&mut self, file: &str, line: usize, content: &str) {
        self.by_file
            .entry(file.to_string())
            .or_default()
            .push((line, content.to_string()));
    }

    /// Format all grouped items into a compact display string.
    /// Files are sorted alphabetically. Items per file are truncated with "+N more".
    pub fn format(&self) -> String {
        let mut files: Vec<&String> = self.by_file.keys().collect();
        files.sort();

        let mut out = Vec::new();
        for file in files.iter().take(self.max_files) {
            let items = &self.by_file[*file];
            out.push(format!("{} ({}):", file, items.len()));
            for (line, content) in items.iter().take(self.max_items_per_file) {
                out.push(format!("  {}: {}", line, content));
            }
            if items.len() > self.max_items_per_file {
                out.push(format!("  +{} more", items.len() - self.max_items_per_file));
            }
        }
        let hidden_files = self.by_file.len().saturating_sub(self.max_files);
        if hidden_files > 0 {
            out.push(format!("... and {} more files", hidden_files));
        }
        out.join("\n")
    }

    /// Total number of items across all files.
    #[allow(dead_code)]
    pub fn total(&self) -> usize {
        self.by_file.values().map(|v| v.len()).sum()
    }

    /// Number of distinct files.
    pub fn file_count(&self) -> usize {
        self.by_file.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_grouper() {
        let g = FileGrouper::new(10, 5);
        assert_eq!(g.total(), 0);
        assert_eq!(g.file_count(), 0);
        assert_eq!(g.format(), "");
    }

    #[test]
    fn test_single_file() {
        let mut g = FileGrouper::new(10, 5);
        g.add("src/main.rs", 10, "unused variable");
        assert_eq!(g.total(), 1);
        assert_eq!(g.file_count(), 1);
        let out = g.format();
        assert!(out.contains("src/main.rs (1)"));
        assert!(out.contains("10: unused variable"));
    }

    #[test]
    fn test_multiple_files_sorted() {
        let mut g = FileGrouper::new(10, 5);
        g.add("src/z.rs", 1, "z error");
        g.add("src/a.rs", 2, "a error");
        let out = g.format();
        let a_pos = out.find("src/a.rs").unwrap();
        let z_pos = out.find("src/z.rs").unwrap();
        assert!(a_pos < z_pos, "Files should be sorted alphabetically");
    }

    #[test]
    fn test_items_per_file_truncation() {
        let mut g = FileGrouper::new(10, 2);
        g.add("src/main.rs", 1, "err1");
        g.add("src/main.rs", 2, "err2");
        g.add("src/main.rs", 3, "err3");
        let out = g.format();
        assert!(out.contains("err1"));
        assert!(out.contains("err2"));
        assert!(!out.contains("err3"));
        assert!(out.contains("+1 more"));
    }

    #[test]
    fn test_max_files_truncation() {
        let mut g = FileGrouper::new(2, 5);
        g.add("a.rs", 1, "err");
        g.add("b.rs", 1, "err");
        g.add("c.rs", 1, "err");
        let out = g.format();
        assert!(out.contains("and 1 more files"));
    }
}
