//! Bootstraps Mycelium integration by installing hooks and patching CLAUDE.md.
mod claude_md;
mod hook;
pub(crate) mod host_status;
mod json_patch;
mod onboard;

use anyhow::Result;
use std::fs;
use std::path::PathBuf;

pub use json_patch::PatchMode;

use claude_md::{
    MYCELIUM_INSTRUCTIONS, MyceliumBlockUpsert, remove_mycelium_block, resolve_claude_dir,
    upsert_mycelium_block,
};
use claude_md::{MYCELIUM_SLIM, patch_claude_md};
#[cfg(unix)]
use hook::{extract_hook_version, extract_quoted_assignment};
#[cfg(unix)]
use hook::{prepare_hook_paths, prepare_session_summary_hook_path, write_if_changed};
use json_patch::{clean_double_blanks, hook_already_present, remove_hook_from_settings};

#[cfg(unix)]
const LEGACY_SESSION_SUMMARY_HOOK_NAME: &str = "session-summary.sh";

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

pub fn onboard(global: bool, verbose: u8) -> Result<()> {
    onboard::run(global, verbose)
}

/// Full uninstall: remove hooks, MYCELIUM.md, @MYCELIUM.md reference, settings.json entry
pub fn uninstall(global: bool, verbose: u8) -> Result<()> {
    if !global {
        anyhow::bail!(
            "Uninstall only works with --global flag. For local projects, manually remove Mycelium from CLAUDE.md"
        );
    }

    let claude_dir = resolve_claude_dir()?;
    let mut removed = Vec::new();

    // 1. Remove hook files
    let hook_dir = claude_dir.join("hooks");
    let hook_path = hook_dir.join("mycelium-rewrite.sh");
    if hook_path.exists() {
        fs::remove_file(&hook_path)
            .with_context(|| format!("Failed to remove hook: {}", hook_path.display()))?;
        removed.push(format!("Hook: {}", hook_path.display()));
    }

    let session_summary_hook_path = hook_dir.join("mycelium-session-summary.sh");
    if session_summary_hook_path.exists() {
        fs::remove_file(&session_summary_hook_path).with_context(|| {
            format!(
                "Failed to remove session hook: {}",
                session_summary_hook_path.display()
            )
        })?;
        removed.push(format!(
            "Session hook: {}",
            session_summary_hook_path.display()
        ));
    }

    // 1b. Remove integrity hash file
    if crate::integrity::remove_hash(&hook_path)? {
        removed.push("Integrity hash: removed".to_string());
    }
    if crate::integrity::remove_hash(&session_summary_hook_path)? {
        removed.push("Session hook integrity hash: removed".to_string());
    }

    let legacy_session_summary_hook_path = hook_dir.join(LEGACY_SESSION_SUMMARY_HOOK_NAME);
    if legacy_session_summary_hook_path.exists() {
        fs::remove_file(&legacy_session_summary_hook_path).with_context(|| {
            format!(
                "Failed to remove legacy session hook: {}",
                legacy_session_summary_hook_path.display()
            )
        })?;
        removed.push(format!(
            "Legacy session hook: {}",
            legacy_session_summary_hook_path.display()
        ));
    }
    if crate::integrity::remove_hash(&legacy_session_summary_hook_path)? {
        removed.push("Legacy session hook integrity hash: removed".to_string());
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

    // 3. Remove @MYCELIUM.md reference or legacy instructions block from CLAUDE.md
    let claude_md_path = claude_dir.join("CLAUDE.md");
    if claude_md_path.exists() {
        let content = fs::read_to_string(&claude_md_path)
            .with_context(|| format!("Failed to read CLAUDE.md: {}", claude_md_path.display()))?;

        let (without_legacy_block, removed_legacy_block) =
            if content.contains("<!-- mycelium-instructions") {
                remove_mycelium_block(&content)
            } else {
                (content.clone(), false)
            };

        let mut removed_anything = false;
        let mut new_content = without_legacy_block;

        if content.contains("@MYCELIUM.md") {
            new_content = new_content
                .lines()
                .filter(|line| !line.trim().starts_with("@MYCELIUM.md"))
                .collect::<Vec<_>>()
                .join("\n");
            removed_anything = true;
        }

        if removed_legacy_block {
            removed_anything = true;
        }

        if removed_anything {
            let cleaned = clean_double_blanks(&new_content);
            fs::write(&claude_md_path, cleaned).with_context(|| {
                format!("Failed to write CLAUDE.md: {}", claude_md_path.display())
            })?;
            if content.contains("@MYCELIUM.md") {
                removed.push("CLAUDE.md: removed @MYCELIUM.md reference".to_string());
            }
            if removed_legacy_block {
                removed.push("CLAUDE.md: removed legacy Mycelium instructions".to_string());
            }
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
    let claude_capabilities = host_status::claude_code_capabilities();
    let claude_dir = resolve_claude_dir()?;
    let hook_path = claude_dir.join("hooks").join("mycelium-rewrite.sh");
    let session_summary_hook_path = claude_dir.join("hooks").join("mycelium-session-summary.sh");
    let mycelium_md_path = claude_dir.join("MYCELIUM.md");
    let global_claude_md = claude_dir.join("CLAUDE.md");
    let local_claude_md = PathBuf::from("CLAUDE.md");

    println!("mycelium Configuration:\n");
    println!("Host capabilities:");
    println!(
        "  Claude hook adapter: {}",
        claude_capabilities.hook_adapter.detail
    );
    println!(
        "  Claude settings patch: {}",
        claude_capabilities.settings_patch.detail
    );
    println!(
        "  Claude global slim setup: {}",
        claude_capabilities.slim_global_setup.detail
    );
    println!(
        "  Claude CLAUDE.md mode: {}",
        claude_capabilities.legacy_claude_md.detail
    );
    println!();

    // Check rewrite hook
    if !claude_capabilities.hook_adapter.supported {
        println!("- Hook: unsupported on this platform");
    } else if hook_path.exists() {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let metadata = fs::metadata(&hook_path)?;
            let perms = metadata.permissions();
            let is_executable = perms.mode() & 0o111 != 0;

            let hook_content = fs::read_to_string(&hook_path)?;
            let has_guards = hook_content.contains("_resolve_command()")
                && hook_content.contains("MYCELIUM_BIN=")
                && hook_content.contains("JQ_BIN=")
                && !hook_content.contains("__MYCELIUM_VERSION__")
                && !hook_content.contains("__MYCELIUM_BIN__")
                && !hook_content.contains("__JQ_BIN__");
            let is_thin_delegator = hook_content.contains("mycelium rewrite");
            let hook_version = crate::hook_check::parse_hook_version(&hook_content);
            let installed_version = extract_hook_version(&hook_content);
            let current_version = hook::current_install_version();

            if !is_executable {
                println!(
                    "[!] Hook: {} (NOT executable - run: chmod +x)",
                    hook_path.display()
                );
            } else if !is_thin_delegator {
                println!(
                    "[!] Hook: {} (outdated — inline logic, not thin delegator)",
                    hook_path.display()
                );
                println!(
                    "   → Run `mycelium init -g` to upgrade to the current thin delegator hook"
                );
            } else if is_executable && has_guards {
                println!(
                    "ok Hook: {} (thin delegator, version {})",
                    hook_path.display(),
                    hook_version
                );
                match installed_version.as_deref() {
                    Some(version) if version == current_version => {
                        println!("ok Hook version: {} (current)", version);
                    }
                    Some(version) if hook::version_is_stale(version, current_version) => {
                        println!(
                            "[!] Hook version: {} (stale vs current {}; run `mycelium init -g`)",
                            version, current_version
                        );
                    }
                    Some(version) => {
                        println!(
                            "[!] Hook version: {} (mismatch with current {}; run `mycelium init -g`)",
                            version, current_version
                        );
                    }
                    None => {
                        println!(
                            "[!] Hook version: unknown (run `mycelium init -g` to stamp the current version)"
                        );
                    }
                }
                let mycelium_embedded =
                    extract_quoted_assignment(&hook_content, "MYCELIUM_BIN").unwrap_or_default();
                let jq_embedded =
                    extract_quoted_assignment(&hook_content, "JQ_BIN").unwrap_or_default();
                let mycelium_on_path = crate::platform::command_on_path("mycelium");
                let jq_on_path = crate::platform::command_on_path("jq");

                if mycelium_embedded.is_empty() {
                    println!(
                        "[!] Hook dependency: mycelium was not embedded at install time; PATH fallback is required"
                    );
                    println!("    Repair: mycelium init -g");
                } else {
                    let path = PathBuf::from(&mycelium_embedded);
                    if !path.exists() {
                        println!(
                            "[!] Hook dependency: embedded mycelium path missing: {}",
                            path.display()
                        );
                        println!("    Repair: mycelium init -g");
                    }
                }

                if jq_embedded.is_empty() {
                    println!(
                        "[!] Hook dependency: jq was not embedded at install time; PATH fallback is required"
                    );
                    println!("    Repair: install jq, then run `mycelium init -g`");
                } else {
                    let path = PathBuf::from(&jq_embedded);
                    if !path.exists() {
                        println!(
                            "[!] Hook dependency: embedded jq path missing: {}",
                            path.display()
                        );
                        println!("    Repair: install jq, then run `mycelium init -g`");
                    }
                }

                let mut missing_path = Vec::new();
                if !mycelium_on_path {
                    missing_path.push("mycelium");
                }
                if !jq_on_path {
                    missing_path.push("jq");
                }
                if !missing_path.is_empty() {
                    println!(
                        "[!] Hook PATH: current PATH does not expose {}",
                        missing_path.join(" or ")
                    );
                    if !jq_on_path {
                        println!(
                            "    jq is missing from PATH; the hook will use an embedded path when available, otherwise it will skip rewrites."
                        );
                    }
                    if !mycelium_on_path {
                        println!(
                            "    mycelium is missing from PATH; the hook will use an embedded path when available, otherwise it will skip rewrites."
                        );
                    }
                }
            } else {
                println!("[!] Hook: {} (no guards - outdated)", hook_path.display());
                println!("   → Run `mycelium init -g` to refresh the guarded hook");
            }
        }

        #[cfg(not(unix))]
        {
            println!("ok Hook: {} (exists)", hook_path.display());
        }
    } else {
        println!("- Hook: not found");
    }

    // Check session summary hook
    if !claude_capabilities.hook_adapter.supported {
        println!("- Session hook: unsupported on this platform");
    } else if session_summary_hook_path.exists() {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let metadata = fs::metadata(&session_summary_hook_path)?;
            if metadata.permissions().mode() & 0o111 != 0 {
                println!("ok Session hook: {}", session_summary_hook_path.display());
            } else {
                println!(
                    "[!] Session hook: {} (NOT executable - run: chmod +x)",
                    session_summary_hook_path.display()
                );
            }
        }

        #[cfg(not(unix))]
        {
            println!(
                "ok Session hook: {} (exists)",
                session_summary_hook_path.display()
            );
        }
    } else {
        println!("- Session hook: not found");
    }

    if session_summary_hook_path.exists() {
        match crate::integrity::verify_hook_at(&session_summary_hook_path) {
            Ok(crate::integrity::IntegrityStatus::Verified) => {
                println!("ok Session hook integrity: verified");
            }
            Ok(crate::integrity::IntegrityStatus::Tampered { .. }) => {
                println!("[!] Session hook integrity: FAILED");
                println!("    Run: mycelium init -g to restore the Stop hook");
            }
            Ok(crate::integrity::IntegrityStatus::NoBaseline) => {
                println!("[!] Session hook integrity: no baseline hash");
                println!("    Run: mycelium init -g to record a Stop hook baseline");
            }
            Ok(crate::integrity::IntegrityStatus::NotInstalled)
            | Ok(crate::integrity::IntegrityStatus::OrphanedHash) => {}
            Err(e) => {
                println!("[!] Session hook integrity: error ({e})");
            }
        }
    }

    // Check MYCELIUM.md
    if !claude_capabilities.slim_global_setup.supported {
        println!("- MYCELIUM.md: not used by the docs-only setup on this platform");
    } else if mycelium_md_path.exists() {
        println!("ok MYCELIUM.md: {} (slim mode)", mycelium_md_path.display());
    } else {
        println!("- MYCELIUM.md: not found");
    }

    // Check hook integrity
    if claude_capabilities.hook_adapter.supported {
        match crate::integrity::verify_hook_at(&hook_path) {
            Ok(crate::integrity::IntegrityStatus::Verified) => {
                println!("ok Integrity: hook hash verified");
            }
            Ok(crate::integrity::IntegrityStatus::Tampered { .. }) => {
                println!(
                    "error: Integrity: hook modified outside mycelium init (run: mycelium verify)"
                );
            }
            Ok(crate::integrity::IntegrityStatus::NoBaseline) => {
                println!("[!] Integrity: no baseline hash (run: mycelium init -g to establish)");
            }
            Ok(crate::integrity::IntegrityStatus::NotInstalled)
            | Ok(crate::integrity::IntegrityStatus::OrphanedHash) => {
                // Don't show integrity line if hook isn't installed
            }
            Err(_) => {
                println!("[!] Integrity: check failed");
            }
        }
    }

    // Check global CLAUDE.md
    if global_claude_md.exists() {
        let content = fs::read_to_string(&global_claude_md)?;
        if content.contains("@MYCELIUM.md") {
            println!("ok Global (~/.claude/CLAUDE.md): @MYCELIUM.md reference");
        } else if content.contains("<!-- mycelium-instructions") {
            if claude_capabilities.slim_global_setup.supported {
                println!(
                    "[!] Global (~/.claude/CLAUDE.md): old Mycelium block (run: mycelium init -g to migrate)"
                );
            } else {
                println!(
                    "ok Global (~/.claude/CLAUDE.md): legacy Mycelium instructions (docs-only mode)"
                );
            }
        } else {
            println!("- Global (~/.claude/CLAUDE.md): exists but mycelium not configured");
        }
    } else {
        println!("- Global (~/.claude/CLAUDE.md): not found");
    }

    // Check local CLAUDE.md
    if local_claude_md.exists() {
        let content = fs::read_to_string(&local_claude_md)?;
        if content.contains("mycelium") {
            println!("ok Local (./CLAUDE.md): mycelium enabled");
        } else {
            println!("- Local (./CLAUDE.md): exists but mycelium not configured");
        }
    } else {
        println!("- Local (./CLAUDE.md): not found");
    }

    // Check settings.json
    let settings_path = claude_dir.join("settings.json");
    if !claude_capabilities.settings_patch.supported {
        println!("- settings.json: not managed by Mycelium on this platform");
    } else if settings_path.exists() {
        let content = fs::read_to_string(&settings_path)?;
        if !content.trim().is_empty() {
            if let Ok(root) = serde_json::from_str::<serde_json::Value>(&content) {
                let hook_command = hook_path.display().to_string();
                let session_summary_hook_command = session_summary_hook_path.display().to_string();
                let rewrite_registered = hook_already_present(&root, "PreToolUse", &hook_command);
                let session_registered =
                    hook_already_present(&root, "Stop", &session_summary_hook_command);

                if rewrite_registered && session_registered {
                    println!("ok settings.json: Mycelium hooks configured");
                } else {
                    println!("[!] settings.json: exists but Mycelium hooks are incomplete");
                    if !rewrite_registered {
                        println!("    Missing PreToolUse hook: {}", hook_path.display());
                    }
                    if !session_registered {
                        println!(
                            "    Missing Stop hook: {}",
                            session_summary_hook_path.display()
                        );
                    }
                    println!("    Run: mycelium init -g --auto-patch");
                }
            } else {
                println!("[!] settings.json: exists but invalid JSON");
            }
        } else {
            println!("- settings.json: empty");
        }
    } else {
        println!("- settings.json: not found");
    }

    println!("\nHost adapters:");
    for status in host_status::collect_host_adapter_statuses() {
        let marker = if status.configured { "ok" } else { "-" };
        let detected = if status.detected {
            "detected"
        } else {
            "not detected"
        };
        println!(
            "{} {}: {} ({})",
            marker, status.name, status.detail, detected
        );
    }

    println!("\nUsage:");
    println!("  mycelium init              # Local CLAUDE.md instructions");
    if claude_capabilities.hook_adapter.supported {
        println!(
            "  mycelium init -g           # Claude adapter hook + MYCELIUM.md + @MYCELIUM.md + settings.json"
        );
        println!("  mycelium init -g --auto-patch    # Same as above but no prompt");
        println!("  mycelium init -g --no-patch      # Skip settings.json (manual setup)");
        println!("  mycelium init -g --hook-only     # Hook only, no MYCELIUM.md");
    } else {
        println!("  mycelium init -g --claude-md     # Docs-only global setup on this platform");
    }
    println!("  mycelium init -g --uninstall     # Remove all Mycelium artifacts");
    println!(
        "  mycelium init -g --claude-md     # Legacy: full injection into ~/.claude/CLAUDE.md"
    );
    println!(
        "  {}                 # Preferred first-time host setup / repair flow where supported",
        host_status::operator_setup_hint()
    );

    Ok(())
}

#[cfg(unix)]
fn install_claude_code_adapter(patch_mode: PatchMode, verbose: u8) -> Result<()> {
    use crate::init::json_patch::patch_settings_json;
    use hook::{ensure_hook_installed, ensure_session_summary_hook_installed};

    let claude_dir = resolve_claude_dir()?;
    let mycelium_md_path = claude_dir.join("MYCELIUM.md");
    let claude_md_path = claude_dir.join("CLAUDE.md");

    let (_hook_dir, hook_path) = prepare_hook_paths()?;
    let (_session_hook_dir, session_summary_hook_path) = prepare_session_summary_hook_path()?;
    let hook_changed = ensure_hook_installed(&hook_path, verbose)?;
    let session_hook_changed =
        ensure_session_summary_hook_installed(&session_summary_hook_path, verbose)?;

    write_if_changed(&mycelium_md_path, MYCELIUM_SLIM, "MYCELIUM.md", verbose)?;
    let migrated = patch_claude_md(&claude_md_path, verbose)?;

    let hook_status = if hook_changed || session_hook_changed {
        "installed/updated"
    } else {
        "already up to date"
    };
    println!("\nClaude Code adapter {} (global).\n", hook_status);
    println!("  Hook:           {}", hook_path.display());
    println!("  Session hook:   {}", session_summary_hook_path.display());
    println!(
        "  MYCELIUM.md:    {} (10 lines)",
        mycelium_md_path.display()
    );
    println!("  CLAUDE.md:      @MYCELIUM.md reference added");

    if migrated {
        println!("\n  ok Migrated: removed 137-line Mycelium block from CLAUDE.md");
        println!("              replaced with @MYCELIUM.md (10 lines)");
    }

    let patch_result =
        patch_settings_json(&hook_path, &session_summary_hook_path, patch_mode, verbose)?;
    if matches!(
        patch_result,
        crate::init::json_patch::PatchResult::Patched
            | crate::init::json_patch::PatchResult::AlreadyPresent
    ) {
        remove_legacy_session_summary_artifacts(verbose)?;
    }
    report_settings_patch_result(patch_result);
    println!();

    Ok(())
}

#[cfg(not(unix))]
fn install_claude_code_adapter(_patch_mode: PatchMode, _verbose: u8) -> Result<()> {
    unreachable!("Claude Code hook adapter should be gated by host capabilities")
}

#[cfg(unix)]
fn install_claude_hook_only(patch_mode: PatchMode, verbose: u8) -> Result<()> {
    use crate::init::json_patch::patch_settings_json;
    use hook::{ensure_hook_installed, ensure_session_summary_hook_installed};

    let (_hook_dir, hook_path) = prepare_hook_paths()?;
    let (_session_hook_dir, session_summary_hook_path) = prepare_session_summary_hook_path()?;
    let hook_changed = ensure_hook_installed(&hook_path, verbose)?;
    let session_hook_changed =
        ensure_session_summary_hook_installed(&session_summary_hook_path, verbose)?;

    let hook_status = if hook_changed || session_hook_changed {
        "installed/updated"
    } else {
        "already up to date"
    };
    println!("\nClaude Code adapter {} (hook-only mode).\n", hook_status);
    println!("  Hook: {}", hook_path.display());
    println!("  Session hook: {}", session_summary_hook_path.display());
    println!(
        "  Note: No MYCELIUM.md created. Claude Code will not see Mycelium meta commands (gain, discover, proxy, invoke)."
    );

    let patch_result =
        patch_settings_json(&hook_path, &session_summary_hook_path, patch_mode, verbose)?;
    if matches!(
        patch_result,
        crate::init::json_patch::PatchResult::Patched
            | crate::init::json_patch::PatchResult::AlreadyPresent
    ) {
        remove_legacy_session_summary_artifacts(verbose)?;
    }
    report_settings_patch_result(patch_result);
    println!();

    Ok(())
}

#[cfg(not(unix))]
fn install_claude_hook_only(_patch_mode: PatchMode, _verbose: u8) -> Result<()> {
    unreachable!("Claude Code hook adapter should be gated by host capabilities")
}

#[cfg(unix)]
fn report_settings_patch_result(patch_result: crate::init::json_patch::PatchResult) {
    use crate::init::json_patch::PatchResult;

    match patch_result {
        PatchResult::Patched => {}
        PatchResult::AlreadyPresent => {
            println!("\n  settings.json: Claude hooks already present");
            println!("  Restart Claude Code. Test with: git status");
        }
        PatchResult::Declined | PatchResult::Skipped => {}
    }
}

#[cfg(unix)]
fn remove_legacy_session_summary_artifacts(verbose: u8) -> Result<()> {
    let claude_dir = resolve_claude_dir()?;
    let legacy_hook_path = claude_dir
        .join("hooks")
        .join(LEGACY_SESSION_SUMMARY_HOOK_NAME);
    let removed_hook = if legacy_hook_path.exists() {
        fs::remove_file(&legacy_hook_path).with_context(|| {
            format!(
                "Failed to remove legacy session hook: {}",
                legacy_hook_path.display()
            )
        })?;
        true
    } else {
        false
    };
    let removed_hash = crate::integrity::remove_hash(&legacy_hook_path)?;

    if verbose > 0 && (removed_hook || removed_hash) {
        eprintln!(
            "Removed legacy session hook artifacts: {}",
            legacy_hook_path.display()
        );
    }

    Ok(())
}

#[cfg(not(unix))]
fn report_settings_patch_result(_patch_result: crate::init::json_patch::PatchResult) {}

/// Default mode: hook + slim MYCELIUM.md + @MYCELIUM.md reference where supported.
fn run_default_mode(global: bool, patch_mode: PatchMode, verbose: u8) -> Result<()> {
    if !global {
        // Local init: unchanged behavior (full injection into ./CLAUDE.md)
        return run_claude_md_mode(false, verbose);
    }

    let capabilities = host_status::claude_code_capabilities();
    if !capabilities.hook_adapter.supported {
        eprintln!("[!] Claude Code hook adapter is unsupported on this platform.");
        eprintln!("    {}", capabilities.hook_adapter.detail);
        eprintln!("    Falling back to docs-only global CLAUDE.md setup.");
        return run_claude_md_mode(true, verbose);
    }

    install_claude_code_adapter(patch_mode, verbose)
}

/// Hook-only mode: just the hook, no MYCELIUM.md, where supported.
fn run_hook_only_mode(global: bool, patch_mode: PatchMode, verbose: u8) -> Result<()> {
    if !global {
        eprintln!("[!] Warning: --hook-only only makes sense with --global");
        eprintln!("    For local projects, use default mode or --claude-md");
        return Ok(());
    }

    let capabilities = host_status::claude_code_capabilities();
    if !capabilities.hook_adapter.supported {
        anyhow::bail!(
            "Claude Code hook adapter is unsupported on this platform. {}",
            capabilities.hook_adapter.detail
        );
    }

    install_claude_hook_only(patch_mode, verbose)
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
                    "ok Added mycelium instructions to existing {}",
                    path.display()
                );
            }
            MyceliumBlockUpsert::Updated => {
                fs::write(&path, new_content)?;
                println!("ok Updated mycelium instructions in {}", path.display());
            }
            MyceliumBlockUpsert::Unchanged => {
                println!(
                    "ok {} already contains up-to-date mycelium instructions",
                    path.display()
                );
                return Ok(());
            }
            MyceliumBlockUpsert::Malformed => {
                eprintln!(
                    "[!] Warning: Found '<!-- mycelium-instructions' without closing marker in {}",
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
        println!("ok Created {} with mycelium instructions", path.display());
    }

    if global {
        println!("   Claude Code will now use Mycelium in all sessions");
    } else {
        println!("   Claude Code will use Mycelium in this project");
    }

    Ok(())
}

// Need with_context for uninstall
use anyhow::Context;

pub mod context;
