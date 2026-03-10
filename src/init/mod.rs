//! Bootstraps Mycelium integration by installing hooks and patching CLAUDE.md.
mod claude_md;
mod hook;
mod json_patch;

use anyhow::Result;
use std::fs;
use std::path::PathBuf;

pub use json_patch::PatchMode;
#[cfg(unix)]
pub use json_patch::PatchResult;

use claude_md::{
    MYCELIUM_INSTRUCTIONS, MyceliumBlockUpsert, resolve_claude_dir, upsert_mycelium_block,
};
#[cfg(unix)]
use claude_md::{MYCELIUM_SLIM, patch_claude_md};
#[cfg(unix)]
use hook::{prepare_hook_paths, write_if_changed};
#[cfg(unix)]
use json_patch::patch_settings_json;
use json_patch::{clean_double_blanks, hook_already_present, remove_hook_from_settings};

#[cfg(unix)]
use hook::ensure_hook_installed;

/// Main entry point for `mycelium init`
pub fn run(
    global: bool,
    claude_md: bool,
    hook_only: bool,
    patch_mode: PatchMode,
    verbose: u8,
) -> Result<()> {
    // Mode selection
    match (claude_md, hook_only) {
        (true, _) => run_claude_md_mode(global, verbose),
        (false, true) => run_hook_only_mode(global, patch_mode, verbose),
        (false, false) => run_default_mode(global, patch_mode, verbose),
    }
}

/// Full uninstall: remove hook, MYCELIUM.md, @MYCELIUM.md reference, settings.json entry
pub fn uninstall(global: bool, verbose: u8) -> Result<()> {
    if !global {
        anyhow::bail!(
            "Uninstall only works with --global flag. For local projects, manually remove Mycelium from CLAUDE.md"
        );
    }

    let claude_dir = resolve_claude_dir()?;
    let mut removed = Vec::new();

    // 1. Remove hook file
    let hook_path = claude_dir.join("hooks").join("mycelium-rewrite.sh");
    if hook_path.exists() {
        fs::remove_file(&hook_path)
            .with_context(|| format!("Failed to remove hook: {}", hook_path.display()))?;
        removed.push(format!("Hook: {}", hook_path.display()));
    }

    // 1b. Remove integrity hash file
    if crate::integrity::remove_hash(&hook_path)? {
        removed.push("Integrity hash: removed".to_string());
    }

    // 2. Remove MYCELIUM.md
    let mycelium_md_path = claude_dir.join("MYCELIUM.md");
    if mycelium_md_path.exists() {
        fs::remove_file(&mycelium_md_path).with_context(|| {
            format!(
                "Failed to remove MYCELIUM.md: {}",
                mycelium_md_path.display()
            )
        })?;
        removed.push(format!("MYCELIUM.md: {}", mycelium_md_path.display()));
    }

    // 3. Remove @MYCELIUM.md reference from CLAUDE.md
    let claude_md_path = claude_dir.join("CLAUDE.md");
    if claude_md_path.exists() {
        let content = fs::read_to_string(&claude_md_path)
            .with_context(|| format!("Failed to read CLAUDE.md: {}", claude_md_path.display()))?;

        if content.contains("@MYCELIUM.md") {
            let new_content = content
                .lines()
                .filter(|line| !line.trim().starts_with("@MYCELIUM.md"))
                .collect::<Vec<_>>()
                .join("\n");

            // Clean up double blanks
            let cleaned = clean_double_blanks(&new_content);

            fs::write(&claude_md_path, cleaned).with_context(|| {
                format!("Failed to write CLAUDE.md: {}", claude_md_path.display())
            })?;
            removed.push("CLAUDE.md: removed @MYCELIUM.md reference".to_string());
        }
    }

    // 4. Remove hook entry from settings.json
    if remove_hook_from_settings(verbose)? {
        removed.push("settings.json: removed Mycelium hook entry".to_string());
    }

    // Report results
    if removed.is_empty() {
        println!("Mycelium was not installed (nothing to remove)");
    } else {
        println!("Mycelium uninstalled:");
        for item in removed {
            println!("  - {}", item);
        }
        println!("\nRestart Claude Code to apply changes.");
    }

    Ok(())
}

/// Show current mycelium configuration
pub fn show_config() -> Result<()> {
    let claude_dir = resolve_claude_dir()?;
    let hook_path = claude_dir.join("hooks").join("mycelium-rewrite.sh");
    let mycelium_md_path = claude_dir.join("MYCELIUM.md");
    let global_claude_md = claude_dir.join("CLAUDE.md");
    let local_claude_md = PathBuf::from("CLAUDE.md");

    println!("📋 mycelium Configuration:\n");

    // Check hook
    if hook_path.exists() {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let metadata = fs::metadata(&hook_path)?;
            let perms = metadata.permissions();
            let is_executable = perms.mode() & 0o111 != 0;

            let hook_content = fs::read_to_string(&hook_path)?;
            let has_guards = hook_content.contains("command -v mycelium")
                && hook_content.contains("command -v jq");
            let is_thin_delegator = hook_content.contains("mycelium rewrite");
            let hook_version = crate::hook_check::parse_hook_version(&hook_content);

            if !is_executable {
                println!(
                    "⚠️  Hook: {} (NOT executable - run: chmod +x)",
                    hook_path.display()
                );
            } else if !is_thin_delegator {
                println!(
                    "⚠️  Hook: {} (outdated — inline logic, not thin delegator)",
                    hook_path.display()
                );
                println!(
                    "   → Run `mycelium init --global` to upgrade to the single source of truth hook"
                );
            } else if is_executable && has_guards {
                println!(
                    "✅ Hook: {} (thin delegator, version {})",
                    hook_path.display(),
                    hook_version
                );
            } else {
                println!("⚠️  Hook: {} (no guards - outdated)", hook_path.display());
            }
        }

        #[cfg(not(unix))]
        {
            println!("✅ Hook: {} (exists)", hook_path.display());
        }
    } else {
        println!("⚪ Hook: not found");
    }

    // Check MYCELIUM.md
    if mycelium_md_path.exists() {
        println!("✅ MYCELIUM.md: {} (slim mode)", mycelium_md_path.display());
    } else {
        println!("⚪ MYCELIUM.md: not found");
    }

    // Check hook integrity
    match crate::integrity::verify_hook_at(&hook_path) {
        Ok(crate::integrity::IntegrityStatus::Verified) => {
            println!("✅ Integrity: hook hash verified");
        }
        Ok(crate::integrity::IntegrityStatus::Tampered { .. }) => {
            println!("❌ Integrity: hook modified outside mycelium init (run: mycelium verify)");
        }
        Ok(crate::integrity::IntegrityStatus::NoBaseline) => {
            println!("⚠️  Integrity: no baseline hash (run: mycelium init -g to establish)");
        }
        Ok(crate::integrity::IntegrityStatus::NotInstalled)
        | Ok(crate::integrity::IntegrityStatus::OrphanedHash) => {
            // Don't show integrity line if hook isn't installed
        }
        Err(_) => {
            println!("⚠️  Integrity: check failed");
        }
    }

    // Check global CLAUDE.md
    if global_claude_md.exists() {
        let content = fs::read_to_string(&global_claude_md)?;
        if content.contains("@MYCELIUM.md") {
            println!("✅ Global (~/.claude/CLAUDE.md): @MYCELIUM.md reference");
        } else if content.contains("<!-- mycelium-instructions") {
            println!(
                "⚠️  Global (~/.claude/CLAUDE.md): old Mycelium block (run: mycelium init -g to migrate)"
            );
        } else {
            println!("⚪ Global (~/.claude/CLAUDE.md): exists but mycelium not configured");
        }
    } else {
        println!("⚪ Global (~/.claude/CLAUDE.md): not found");
    }

    // Check local CLAUDE.md
    if local_claude_md.exists() {
        let content = fs::read_to_string(&local_claude_md)?;
        if content.contains("mycelium") {
            println!("✅ Local (./CLAUDE.md): mycelium enabled");
        } else {
            println!("⚪ Local (./CLAUDE.md): exists but mycelium not configured");
        }
    } else {
        println!("⚪ Local (./CLAUDE.md): not found");
    }

    // Check settings.json
    let settings_path = claude_dir.join("settings.json");
    if settings_path.exists() {
        let content = fs::read_to_string(&settings_path)?;
        if !content.trim().is_empty() {
            if let Ok(root) = serde_json::from_str::<serde_json::Value>(&content) {
                let hook_command = hook_path.display().to_string();
                if hook_already_present(&root, &hook_command) {
                    println!("✅ settings.json: Mycelium hook configured");
                } else {
                    println!("⚠️  settings.json: exists but Mycelium hook not configured");
                    println!("    Run: mycelium init -g --auto-patch");
                }
            } else {
                println!("⚠️  settings.json: exists but invalid JSON");
            }
        } else {
            println!("⚪ settings.json: empty");
        }
    } else {
        println!("⚪ settings.json: not found");
    }

    println!("\nUsage:");
    println!("  mycelium init              # Full injection into local CLAUDE.md");
    println!(
        "  mycelium init -g           # Hook + MYCELIUM.md + @MYCELIUM.md + settings.json (recommended)"
    );
    println!("  mycelium init -g --auto-patch    # Same as above but no prompt");
    println!("  mycelium init -g --no-patch      # Skip settings.json (manual setup)");
    println!("  mycelium init -g --uninstall     # Remove all Mycelium artifacts");
    println!(
        "  mycelium init -g --claude-md     # Legacy: full injection into ~/.claude/CLAUDE.md"
    );
    println!("  mycelium init -g --hook-only     # Hook only, no MYCELIUM.md");

    Ok(())
}

/// Default mode: hook + slim MYCELIUM.md + @MYCELIUM.md reference
#[cfg(not(unix))]
fn run_default_mode(_global: bool, _patch_mode: PatchMode, _verbose: u8) -> Result<()> {
    eprintln!("⚠️  Hook-based mode requires Unix (macOS/Linux).");
    eprintln!("    Windows: use --claude-md mode for full injection.");
    eprintln!("    Falling back to --claude-md mode.");
    run_claude_md_mode(_global, _verbose)
}

#[cfg(unix)]
fn run_default_mode(global: bool, patch_mode: PatchMode, verbose: u8) -> Result<()> {
    if !global {
        // Local init: unchanged behavior (full injection into ./CLAUDE.md)
        return run_claude_md_mode(false, verbose);
    }

    let claude_dir = resolve_claude_dir()?;
    let mycelium_md_path = claude_dir.join("MYCELIUM.md");
    let claude_md_path = claude_dir.join("CLAUDE.md");

    // 1. Prepare hook directory and install hook
    let (_hook_dir, hook_path) = prepare_hook_paths()?;
    let hook_changed = ensure_hook_installed(&hook_path, verbose)?;

    // 2. Write MYCELIUM.md
    write_if_changed(&mycelium_md_path, MYCELIUM_SLIM, "MYCELIUM.md", verbose)?;

    // 3. Patch CLAUDE.md (add @MYCELIUM.md, migrate if needed)
    let migrated = patch_claude_md(&claude_md_path, verbose)?;

    // 4. Print success message
    let hook_status = if hook_changed {
        "installed/updated"
    } else {
        "already up to date"
    };
    println!("\nMycelium hook {} (global).\n", hook_status);
    println!("  Hook:      {}", hook_path.display());
    println!(
        "  MYCELIUM.md:    {} (10 lines)",
        mycelium_md_path.display()
    );
    println!("  CLAUDE.md: @MYCELIUM.md reference added");

    if migrated {
        println!("\n  ✅ Migrated: removed 137-line Mycelium block from CLAUDE.md");
        println!("              replaced with @MYCELIUM.md (10 lines)");
    }

    // 5. Patch settings.json
    let patch_result = patch_settings_json(&hook_path, patch_mode, verbose)?;

    // Report result
    match patch_result {
        PatchResult::Patched => {
            // Already printed by patch_settings_json
        }
        PatchResult::AlreadyPresent => {
            println!("\n  settings.json: hook already present");
            println!("  Restart Claude Code. Test with: git status");
        }
        PatchResult::Declined | PatchResult::Skipped => {
            // Manual instructions already printed by patch_settings_json
        }
    }

    println!(); // Final newline

    Ok(())
}

/// Hook-only mode: just the hook, no MYCELIUM.md
#[cfg(not(unix))]
fn run_hook_only_mode(_global: bool, _patch_mode: PatchMode, _verbose: u8) -> Result<()> {
    anyhow::bail!("Hook install requires Unix (macOS/Linux). Use WSL or --claude-md mode.")
}

#[cfg(unix)]
fn run_hook_only_mode(global: bool, patch_mode: PatchMode, verbose: u8) -> Result<()> {
    if !global {
        eprintln!("⚠️  Warning: --hook-only only makes sense with --global");
        eprintln!("    For local projects, use default mode or --claude-md");
        return Ok(());
    }

    // Prepare and install hook
    let (_hook_dir, hook_path) = prepare_hook_paths()?;
    let hook_changed = ensure_hook_installed(&hook_path, verbose)?;

    let hook_status = if hook_changed {
        "installed/updated"
    } else {
        "already up to date"
    };
    println!("\nMycelium hook {} (hook-only mode).\n", hook_status);
    println!("  Hook: {}", hook_path.display());
    println!(
        "  Note: No MYCELIUM.md created. Claude won't know about meta commands (gain, discover, proxy)."
    );

    // Patch settings.json
    let patch_result = patch_settings_json(&hook_path, patch_mode, verbose)?;

    // Report result
    match patch_result {
        PatchResult::Patched => {
            // Already printed by patch_settings_json
        }
        PatchResult::AlreadyPresent => {
            println!("\n  settings.json: hook already present");
            println!("  Restart Claude Code. Test with: git status");
        }
        PatchResult::Declined | PatchResult::Skipped => {
            // Manual instructions already printed by patch_settings_json
        }
    }

    println!(); // Final newline

    Ok(())
}

/// Legacy mode: full 137-line injection into CLAUDE.md
fn run_claude_md_mode(global: bool, verbose: u8) -> Result<()> {
    let path = if global {
        resolve_claude_dir()?.join("CLAUDE.md")
    } else {
        PathBuf::from("CLAUDE.md")
    };

    if global && let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    if verbose > 0 {
        eprintln!("Writing mycelium instructions to: {}", path.display());
    }

    if path.exists() {
        let existing = fs::read_to_string(&path)?;
        // upsert_mycelium_block handles all 4 cases: add, update, unchanged, malformed
        let (new_content, action) = upsert_mycelium_block(&existing, MYCELIUM_INSTRUCTIONS);

        match action {
            MyceliumBlockUpsert::Added => {
                fs::write(&path, new_content)?;
                println!(
                    "✅ Added mycelium instructions to existing {}",
                    path.display()
                );
            }
            MyceliumBlockUpsert::Updated => {
                fs::write(&path, new_content)?;
                println!("✅ Updated mycelium instructions in {}", path.display());
            }
            MyceliumBlockUpsert::Unchanged => {
                println!(
                    "✅ {} already contains up-to-date mycelium instructions",
                    path.display()
                );
                return Ok(());
            }
            MyceliumBlockUpsert::Malformed => {
                eprintln!(
                    "⚠️  Warning: Found '<!-- mycelium-instructions' without closing marker in {}",
                    path.display()
                );

                if let Some((line_num, _)) = existing
                    .lines()
                    .enumerate()
                    .find(|(_, line)| line.contains("<!-- mycelium-instructions"))
                {
                    eprintln!("    Location: line {}", line_num + 1);
                }

                eprintln!("    Action: Manually remove the incomplete block, then re-run:");
                if global {
                    eprintln!("            mycelium init -g --claude-md");
                } else {
                    eprintln!("            mycelium init --claude-md");
                }
                return Ok(());
            }
        }
    } else {
        fs::write(&path, MYCELIUM_INSTRUCTIONS)?;
        println!("✅ Created {} with mycelium instructions", path.display());
    }

    if global {
        println!("   Claude Code will now use mycelium in all sessions");
    } else {
        println!("   Claude Code will use mycelium in this project");
    }

    Ok(())
}

// Need with_context for uninstall
use anyhow::Context;
