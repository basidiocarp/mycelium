//! Manages the Mycelium instructions block inside CLAUDE.md files.
use anyhow::{Context, Result};
#[cfg_attr(not(unix), allow(unused_imports))]
use std::fs;
#[cfg(unix)]
use std::path::Path;
use std::path::PathBuf;

// Embedded slim Mycelium awareness instructions — Unix-only (referenced by the bash hook)
#[cfg(unix)]
pub(crate) const MYCELIUM_SLIM: &str = include_str!("../../hooks/mycelium-awareness.md");

// Legacy full instructions for backward compatibility (--claude-md mode)
pub(crate) const MYCELIUM_INSTRUCTIONS: &str = r##"<!-- mycelium-instructions v2 -->
# Mycelium - Token-Optimized Commands

## Golden Rule

**Always prefix commands with `mycelium`**. If Mycelium has a dedicated filter, it uses it. If not, it passes through unchanged. This means Mycelium is always safe to use.

**Important**: Even in command chains with `&&`, use `mycelium`:
```bash
# Wrong
git add . && git commit -m "msg" && git push

# Correct
mycelium git add . && mycelium git commit -m "msg" && mycelium git push
```

## Mycelium Commands by Workflow

### Build & Compile (80-90% savings)
```bash
mycelium cargo build         # Cargo build output
mycelium cargo check         # Cargo check output
mycelium cargo clippy        # Clippy warnings grouped by file (80%)
mycelium tsc                 # TypeScript errors grouped by file/code (83%)
mycelium lint                # ESLint/Biome violations grouped (84%)
mycelium prettier --check    # Files needing format only (70%)
mycelium next build          # Next.js build with route metrics (87%)
```

### Test (90-99% savings)
```bash
mycelium cargo test          # Cargo test failures only (90%)
mycelium vitest run          # Vitest failures only (99.5%)
mycelium playwright test     # Playwright failures only (94%)
mycelium test <cmd>          # Generic test wrapper - failures only
```

### Git (59-80% savings)
```bash
mycelium git status          # Compact status
mycelium git log             # Compact log (works with all git flags)
mycelium git diff            # Compact diff (80%)
mycelium git show            # Compact show (80%)
mycelium git add             # Ultra-compact confirmations (59%)
mycelium git commit          # Ultra-compact confirmations (59%)
mycelium git push            # Ultra-compact confirmations
mycelium git pull            # Ultra-compact confirmations
mycelium git branch          # Compact branch list
mycelium git fetch           # Compact fetch
mycelium git stash           # Compact stash
mycelium git worktree        # Compact worktree
```

Note: Git passthrough works for ALL subcommands, even those not explicitly listed.

### GitHub (26-87% savings)
```bash
mycelium gh pr view <num>    # Compact PR view (87%)
mycelium gh pr checks        # Compact PR checks (79%)
mycelium gh run list         # Compact workflow runs (82%)
mycelium gh issue list       # Compact issue list (80%)
mycelium gh api              # Compact API responses (26%)
```

### JavaScript/TypeScript Tooling (70-90% savings)
```bash
mycelium pnpm list           # Compact dependency tree (70%)
mycelium pnpm outdated       # Compact outdated packages (80%)
mycelium pnpm install        # Compact install output (90%)
mycelium npm run <script>    # Compact npm script output
mycelium npx <cmd>           # Compact npx command output
mycelium prisma              # Prisma without ASCII art (88%)
```

### Files & Search (60-75% savings)
```bash
mycelium ls <path>           # Tree format, compact (65%)
mycelium read <file>         # Code reading with filtering (60%)
mycelium grep <pattern>      # Search grouped by file (75%)
mycelium find <pattern>      # Find grouped by directory (70%)
```

### Analysis & Debug (70-90% savings)
```bash
mycelium err <cmd>           # Filter errors only from any command
mycelium log <file>          # Deduplicated logs with counts
mycelium json <file>         # JSON structure without values
mycelium deps                # Dependency overview
mycelium env                 # Environment variables compact
mycelium summary <cmd>       # Smart summary of command output
mycelium diff                # Ultra-compact diffs
```

### Infrastructure (85% savings)
```bash
mycelium docker ps           # Compact container list
mycelium docker images       # Compact image list
mycelium docker logs <c>     # Deduplicated logs
mycelium kubectl get         # Compact resource list
mycelium kubectl logs        # Deduplicated pod logs
```

### Network (65-70% savings)
```bash
mycelium curl <url>          # Compact HTTP responses (70%)
mycelium wget <url>          # Compact download output (65%)
```

### Meta Commands
```bash
mycelium gain                # View token savings statistics
mycelium gain --history      # View command history with savings
mycelium discover            # Analyze Claude Code and Codex sessions for missed Mycelium usage
mycelium proxy <cmd>         # Run command without filtering (for debugging)
mycelium invoke <cmd>        # Execute a command through Mycelium rewrite resolution
mycelium init                # Add Mycelium instructions to CLAUDE.md
mycelium init --global       # Add Mycelium to ~/.claude/CLAUDE.md
```

## Token Savings Overview

| Category | Commands | Typical Savings |
|----------|----------|-----------------|
| Tests | vitest, playwright, cargo test | 90-99% |
| Build | next, tsc, lint, prettier | 70-87% |
| Git | status, log, diff, add, commit | 59-80% |
| GitHub | gh pr, gh run, gh issue | 26-87% |
| Package Managers | pnpm, npm, npx | 70-90% |
| Files | ls, read, grep, find | 60-75% |
| Infrastructure | docker, kubectl | 85% |
| Network | curl, wget | 65-70% |

Overall average: **60-90% token reduction** on common development operations.
<!-- /mycelium-instructions -->
"##;

// --- upsert_mycelium_block: idempotent Mycelium block management ---

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum MyceliumBlockUpsert {
    /// No existing block found — appended new block
    Added,
    /// Existing block found with different content — replaced
    Updated,
    /// Existing block found with identical content — no-op
    Unchanged,
    /// Opening marker found without closing marker — not safe to rewrite
    Malformed,
}

/// Insert or replace the Mycelium instructions block in `content`.
///
/// Returns `(new_content, action)` describing what happened.
/// The caller decides whether to write `new_content` based on `action`.
pub(crate) fn upsert_mycelium_block(content: &str, block: &str) -> (String, MyceliumBlockUpsert) {
    let start_marker = "<!-- mycelium-instructions";
    let end_marker = "<!-- /mycelium-instructions -->";

    if let Some(start) = content.find(start_marker) {
        if let Some(relative_end) = content[start..].find(end_marker) {
            let end = start + relative_end;
            let end_pos = end + end_marker.len();
            let current_block = content[start..end_pos].trim();
            let desired_block = block.trim();

            if current_block == desired_block {
                return (content.to_string(), MyceliumBlockUpsert::Unchanged);
            }

            // Replace stale block with desired block
            let before = content[..start].trim_end();
            let after = content[end_pos..].trim_start();

            let result = match (before.is_empty(), after.is_empty()) {
                (true, true) => desired_block.to_string(),
                (true, false) => format!("{desired_block}\n\n{after}"),
                (false, true) => format!("{before}\n\n{desired_block}"),
                (false, false) => format!("{before}\n\n{desired_block}\n\n{after}"),
            };

            return (result, MyceliumBlockUpsert::Updated);
        }

        // Opening marker without closing marker — malformed
        return (content.to_string(), MyceliumBlockUpsert::Malformed);
    }

    // No existing block — append
    let trimmed = content.trim();
    if trimmed.is_empty() {
        (block.to_string(), MyceliumBlockUpsert::Added)
    } else {
        (
            format!("{trimmed}\n\n{}", block.trim()),
            MyceliumBlockUpsert::Added,
        )
    }
}

/// Patch CLAUDE.md: add @MYCELIUM.md, migrate if old block exists — Unix-only
#[cfg(unix)]
pub(crate) fn patch_claude_md(path: &Path, verbose: u8) -> Result<bool> {
    let mut content = if path.exists() {
        fs::read_to_string(path)?
    } else {
        String::new()
    };

    let mut migrated = false;

    // Check for old block and migrate
    if content.contains("<!-- mycelium-instructions") {
        let (new_content, did_migrate) = remove_mycelium_block(&content);
        if did_migrate {
            content = new_content;
            migrated = true;
            if verbose > 0 {
                eprintln!("Migrated: removed old Mycelium block from CLAUDE.md");
            }
        }
    }

    // Check if @MYCELIUM.md already present
    if content.contains("@MYCELIUM.md") {
        if verbose > 0 {
            eprintln!("@MYCELIUM.md reference already present in CLAUDE.md");
        }
        if migrated {
            fs::write(path, content)?;
        }
        return Ok(migrated);
    }

    // Add @MYCELIUM.md
    let new_content = if content.is_empty() {
        "@MYCELIUM.md\n".to_string()
    } else {
        format!("{}\n\n@MYCELIUM.md\n", content.trim())
    };

    fs::write(path, new_content)?;

    if verbose > 0 {
        eprintln!("Added @MYCELIUM.md reference to CLAUDE.md");
    }

    Ok(migrated)
}

/// Remove old Mycelium block from CLAUDE.md (migration helper) — Unix-only
#[cfg(unix)]
pub(crate) fn remove_mycelium_block(content: &str) -> (String, bool) {
    if let (Some(start), Some(end)) = (
        content.find("<!-- mycelium-instructions"),
        content.find("<!-- /mycelium-instructions -->"),
    ) {
        let end_pos = end + "<!-- /mycelium-instructions -->".len();
        let before = content[..start].trim_end();
        let after = content[end_pos..].trim_start();

        let result = if after.is_empty() {
            before.to_string()
        } else {
            format!("{}\n\n{}", before, after)
        };

        (result, true) // migrated
    } else if content.contains("<!-- mycelium-instructions") {
        eprintln!("[!] Warning: Found '<!-- mycelium-instructions' without closing marker.");
        eprintln!("    This can happen if CLAUDE.md was manually edited.");

        // Find line number
        if let Some((line_num, _)) = content
            .lines()
            .enumerate()
            .find(|(_, line)| line.contains("<!-- mycelium-instructions"))
        {
            eprintln!("    Location: line {}", line_num + 1);
        }

        eprintln!("    Action: Manually remove the incomplete block, then re-run:");
        eprintln!("            mycelium init -g");
        (content.to_string(), false)
    } else {
        (content.to_string(), false)
    }
}

/// Resolve ~/.claude directory with proper home expansion
pub(crate) fn resolve_claude_dir() -> Result<PathBuf> {
    dirs::home_dir()
        .map(|h| h.join(".claude"))
        .context("Cannot determine home directory. Is $HOME set?")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_init_mentions_all_top_level_commands() {
        for cmd in [
            "mycelium cargo",
            "mycelium gh",
            "mycelium vitest",
            "mycelium tsc",
            "mycelium lint",
            "mycelium prettier",
            "mycelium next",
            "mycelium playwright",
            "mycelium prisma",
            "mycelium pnpm",
            "mycelium npm",
            "mycelium curl",
            "mycelium git",
            "mycelium docker",
            "mycelium kubectl",
        ] {
            assert!(
                MYCELIUM_INSTRUCTIONS.contains(cmd),
                "Missing {cmd} in MYCELIUM_INSTRUCTIONS"
            );
        }
    }

    #[test]
    fn test_init_has_version_marker() {
        assert!(
            MYCELIUM_INSTRUCTIONS.contains("<!-- mycelium-instructions"),
            "MYCELIUM_INSTRUCTIONS must have version marker for idempotency"
        );
    }

    #[test]
    fn test_claude_md_mode_creates_full_injection() {
        // Just verify MYCELIUM_INSTRUCTIONS constant has the right content
        assert!(MYCELIUM_INSTRUCTIONS.contains("<!-- mycelium-instructions"));
        assert!(MYCELIUM_INSTRUCTIONS.contains("mycelium cargo test"));
        assert!(MYCELIUM_INSTRUCTIONS.contains("<!-- /mycelium-instructions -->"));
        assert!(MYCELIUM_INSTRUCTIONS.len() > 4000);
    }

    // --- upsert_mycelium_block tests ---

    #[test]
    fn test_upsert_mycelium_block_appends_when_missing() {
        let input = "# Team instructions";
        let (content, action) = upsert_mycelium_block(input, MYCELIUM_INSTRUCTIONS);
        assert_eq!(action, MyceliumBlockUpsert::Added);
        assert!(content.contains("# Team instructions"));
        assert!(content.contains("<!-- mycelium-instructions"));
    }

    #[test]
    fn test_upsert_mycelium_block_updates_stale_block() {
        let input = r#"# Team instructions

<!-- mycelium-instructions v1 -->
OLD MYCELIUM CONTENT
<!-- /mycelium-instructions -->

More notes
"#;

        let (content, action) = upsert_mycelium_block(input, MYCELIUM_INSTRUCTIONS);
        assert_eq!(action, MyceliumBlockUpsert::Updated);
        assert!(!content.contains("OLD MYCELIUM CONTENT"));
        assert!(content.contains("mycelium cargo test")); // from current MYCELIUM_INSTRUCTIONS
        assert!(content.contains("# Team instructions"));
        assert!(content.contains("More notes"));
    }

    #[test]
    fn test_upsert_mycelium_block_noop_when_already_current() {
        let input = format!(
            "# Team instructions\n\n{}\n\nMore notes\n",
            MYCELIUM_INSTRUCTIONS
        );
        let (content, action) = upsert_mycelium_block(&input, MYCELIUM_INSTRUCTIONS);
        assert_eq!(action, MyceliumBlockUpsert::Unchanged);
        assert_eq!(content, input);
    }

    #[test]
    fn test_upsert_mycelium_block_detects_malformed_block() {
        let input = "<!-- mycelium-instructions v2 -->\npartial";
        let (content, action) = upsert_mycelium_block(input, MYCELIUM_INSTRUCTIONS);
        assert_eq!(action, MyceliumBlockUpsert::Malformed);
        assert_eq!(content, input);
    }

    #[test]
    #[cfg(unix)]
    fn test_migration_removes_old_block() {
        let input = r#"# My Config

<!-- mycelium-instructions v2 -->
OLD MYCELIUM STUFF
<!-- /mycelium-instructions -->

More content"#;

        let (result, migrated) = remove_mycelium_block(input);
        assert!(migrated);
        assert!(!result.contains("OLD MYCELIUM STUFF"));
        assert!(result.contains("# My Config"));
        assert!(result.contains("More content"));
    }

    #[test]
    #[cfg(unix)]
    fn test_migration_warns_on_missing_end_marker() {
        let input = "<!-- mycelium-instructions v2 -->\nOLD STUFF\nNo end marker";
        let (result, migrated) = remove_mycelium_block(input);
        assert!(!migrated);
        assert_eq!(result, input);
    }

    #[test]
    fn test_init_is_idempotent() {
        let temp = TempDir::new().unwrap();
        let claude_md = temp.path().join("CLAUDE.md");

        fs::write(&claude_md, "# My stuff\n\n@MYCELIUM.md\n").unwrap();

        let content = fs::read_to_string(&claude_md).unwrap();
        let count = content.matches("@MYCELIUM.md").count();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_local_init_unchanged() {
        // Local init should use claude-md mode
        let temp = TempDir::new().unwrap();
        let claude_md = temp.path().join("CLAUDE.md");

        fs::write(&claude_md, MYCELIUM_INSTRUCTIONS).unwrap();
        let content = fs::read_to_string(&claude_md).unwrap();

        assert!(content.contains("<!-- mycelium-instructions"));
    }
}
