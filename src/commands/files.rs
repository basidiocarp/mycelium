use clap::Args;
use std::path::PathBuf;

use crate::filter;

/// List directory contents with token-optimized output (proxy to native ls)
#[derive(Args)]
pub struct Ls {
    /// Arguments passed to ls (supports all native ls flags like -l, -a, -h, -R)
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub args: Vec<String>,
}

/// Directory tree with token-optimized output (proxy to native tree)
#[derive(Args)]
pub struct Tree {
    /// Arguments passed to tree (supports all native tree flags like -L, -d, -a)
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub args: Vec<String>,
}

/// Read file with intelligent filtering
#[derive(Args)]
pub struct Read {
    /// File to read
    pub file: PathBuf,
    /// Filter: none, minimal, aggressive
    #[arg(short, long, default_value = "minimal")]
    pub level: filter::FilterLevel,
    /// Max lines
    #[arg(short, long)]
    pub max_lines: Option<usize>,
    /// Show line numbers
    #[arg(short = 'n', long)]
    pub line_numbers: bool,
}

/// Generate 2-line technical summary of a file (heuristic-based)
#[derive(Args)]
pub struct Peek {
    /// File to analyze
    pub file: PathBuf,
    /// Model: heuristic
    #[arg(short, long, default_value = "heuristic")]
    pub model: String,
    /// Force model download
    #[arg(long)]
    pub force_download: bool,
}

/// Find files with compact tree output (accepts native find flags like -name, -type)
#[derive(Args)]
pub struct Find {
    /// All find arguments (supports both Mycelium and native find syntax)
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub args: Vec<String>,
}

/// Compact grep - strips whitespace, truncates, groups by file
#[derive(Args)]
pub struct Grep {
    /// Pattern to search
    pub pattern: String,
    /// Path to search in
    #[arg(default_value = ".")]
    pub path: String,
    /// Max line length
    #[arg(short = 'l', long, default_value = "80")]
    pub max_len: usize,
    /// Max results to show
    #[arg(short, long, default_value = "50")]
    pub max: usize,
    /// Show only match context (not full line)
    #[arg(short, long)]
    pub context_only: bool,
    /// Filter by file type (e.g., ts, py, rust)
    #[arg(short = 't', long)]
    pub file_type: Option<String>,
    /// Show line numbers (always on, accepted for grep/rg compatibility)
    #[arg(short = 'n', long)]
    pub line_numbers: bool,
    /// Extra ripgrep arguments (e.g., -i, -A 3, -w, --glob)
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub extra_args: Vec<String>,
}

/// Ultra-condensed diff (only changed lines)
#[derive(Args)]
pub struct Diff {
    /// First file or - for stdin (unified diff)
    pub file1: PathBuf,
    /// Second file (optional if stdin)
    pub file2: Option<PathBuf>,
}

/// Word/line/byte count with compact output (strips paths and padding)
#[derive(Args)]
pub struct Wc {
    /// Arguments passed to wc (files, flags like -l, -w, -c)
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub args: Vec<String>,
}
