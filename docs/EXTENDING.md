# Extending Mycelium

> Guide to adding new commands, development patterns, and architecture decisions. For system architecture, see [ARCHITECTURE.md](ARCHITECTURE.md).

---

## Table of Contents

1. [Module Development Pattern](#module-development-pattern)
2. [Adding a New Command](#adding-a-new-command)
3. [Common Patterns](#common-patterns)
4. [Design Checklist](#design-checklist)
5. [Architecture Decision Records](#architecture-decision-records)

---

## Module Development Pattern

### Standard Module Template

```rust
// src/example_cmd.rs

use anyhow::{Context, Result};
use std::process::Command;
use crate::{tracking, utils};

/// Public entry point called by main.rs router
pub fn run(args: &[String], verbose: u8) -> Result<()> {
    // 1. Execute underlying command
    let raw_output = execute_command(args)?;

    // 2. Apply filtering strategy
    let filtered = filter_output(&raw_output, verbose);

    // 3. Print result
    println!("{}", filtered);

    // 4. Track token savings
    tracking::track(
        "original_command",
        "mycelium command",
        &raw_output,
        &filtered
    );

    Ok(())
}

/// Execute the underlying tool
fn execute_command(args: &[String]) -> Result<String> {
    let output = Command::new("tool")
        .args(args)
        .output()
        .context("Failed to execute tool")?;

    // Preserve exit codes (critical for CI/CD)
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        eprintln!("{}", stderr);
        std::process::exit(output.status.code().unwrap_or(1));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Apply filtering strategy
fn filter_output(raw: &str, verbose: u8) -> String {
    // Choose strategy: stats, grouping, deduplication, etc.
    // See "Filtering Strategies" in ARCHITECTURE.md for options

    if verbose >= 3 {
        eprintln!("Raw output:\n{}", raw);
    }

    // Apply compression logic
    let compressed = compress(raw);

    compressed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filter_output() {
        let raw = "verbose output here";
        let filtered = filter_output(raw, 0);
        assert!(filtered.len() < raw.len());
    }
}
```

---

## Adding a New Command

### Step 1: Create Module File

```bash
touch src/mycmd.rs
```

### Step 2: Implement Module (src/mycmd.rs)

```rust
use anyhow::{Context, Result};
use std::process::Command;
use crate::tracking;

pub fn run(args: &[String], verbose: u8) -> Result<()> {
    // Execute underlying command
    let output = Command::new("mycmd")
        .args(args)
        .output()
        .context("Failed to execute mycmd")?;

    let raw = String::from_utf8_lossy(&output.stdout);

    // Apply filtering strategy
    let filtered = filter(&raw, verbose);

    // Print result
    println!("{}", filtered);

    // Track savings
    tracking::track("mycmd", "mycelium mycmd", &raw, &filtered);

    Ok(())
}

fn filter(raw: &str, verbose: u8) -> String {
    // Implement your filtering logic
    raw.lines().take(10).collect::<Vec<_>>().join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filter() {
        let raw = "line1\nline2\n";
        let result = filter(raw, 0);
        assert!(result.contains("line1"));
    }
}
```

### Step 3: Declare Module (main.rs)

```rust
// Add to module declarations (alphabetically)
mod mycmd;
```

### Step 4: Add Command Enum Variant (commands.rs)

```rust
#[derive(Subcommand)]
enum Commands {
    // ... existing commands ...

    /// Description of your command
    Mycmd {
        /// Arguments your command accepts
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
}
```

### Step 5: Add Router Match Arm (dispatch.rs)

```rust
match cli.command {
    // ... existing matches ...

    Commands::Mycmd { args } => {
        mycmd::run(&args, verbose)?;
    }
}
```

### Step 6: Test Your Command

```bash
# Build and test
cargo build
./target/debug/mycelium mycmd arg1 arg2

# Run tests
cargo test mycmd::tests

# Check with clippy
cargo clippy --all-targets
```

### Step 7: Document Your Command

Update FEATURES.md or COMMANDS.md:

```markdown
### New Commands

**mycelium mycmd** - Description of what it does
- Strategy: [stats/grouping/filtering/etc.]
- Savings: X-Y%
- Used by: [workflow description]
```

---

## Common Patterns

### 1. Package Manager Detection (JS/TS modules)

```rust
// Detect lockfiles
let is_pnpm = Path::new("pnpm-lock.yaml").exists();
let is_yarn = Path::new("yarn.lock").exists();

// Build command
let mut cmd = if is_pnpm {
    Command::new("pnpm").arg("exec").arg("--").arg("eslint")
} else if is_yarn {
    Command::new("yarn").arg("exec").arg("--").arg("eslint")
} else {
    Command::new("npx").arg("--no-install").arg("--").arg("eslint")
};
```

### 2. Lazy Static Regex (filter.rs, runner.rs)

```rust
lazy_static::lazy_static! {
    static ref PATTERN: Regex = Regex::new(r"ERROR:.*").unwrap();
}

// Usage: compiled once, reused across invocations
let matches: Vec<_> = PATTERN.find_iter(text).collect();
```

### 3. Verbosity Guards

```rust
if verbose > 0 {
    eprintln!("Debug: Processing {} files", count);
}

if verbose >= 2 {
    eprintln!("Executing: {:?}", cmd);
}

if verbose >= 3 {
    eprintln!("Raw output:\n{}", raw);
}
```

---

## Design Checklist

When implementing a new command, consider:

- [ ] **Filtering Strategy**: Which of the 12 strategies fits best? (See [ARCHITECTURE.md](ARCHITECTURE.md#filtering-strategies))
- [ ] **Exit Code Preservation**: Does your command need to preserve exit codes for CI/CD?
- [ ] **Verbosity Support**: Add debug output for `-v`, `-vv`, `-vvv`
- [ ] **Error Handling**: Use `.context()` for meaningful error messages
- [ ] **Package Manager Detection**: For JS/TS tools, use the standard detection pattern
- [ ] **Tests**: Add unit tests for filtering logic
- [ ] **Token Tracking**: Integrate with `tracking::track()`
- [ ] **Documentation**: Update COMMANDS.md with token savings and use cases

---

## Architecture Decision Records

### Why Rust?

- **Performance**: ~5-15ms overhead per command (negligible for user experience)
- **Safety**: No runtime errors from null pointers, data races, etc.
- **Single Binary**: No runtime dependencies (distribute one executable)
- **Cross-Platform**: Works on macOS, Linux, Windows without modification

### Why SQLite for Tracking?

- **Zero Config**: No server setup, works out-of-the-box
- **Lightweight**: ~100KB database for 90 days of history
- **Reliable**: ACID compliance for data integrity
- **Queryable**: Rich analytics via SQL (gain report)

### Why anyhow for Error Handling?

- **Context**: `.context()` adds meaningful error messages throughout call chain
- **Ergonomic**: `?` operator for concise error propagation
- **User-Friendly**: Error display shows full context chain

### Why Clap for CLI Parsing?

- **Derive Macros**: Less boilerplate (declarative CLI definition)
- **Auto-Generated Help**: `--help` generated automatically
- **Type Safety**: Parse arguments directly into typed structs
- **Global Flags**: `-v` and `-u` work across all commands
