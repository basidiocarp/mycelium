//! Rhizome integration — optional code intelligence for the read command.

use std::sync::OnceLock;

static RHIZOME_AVAILABLE: OnceLock<bool> = OnceLock::new();
static RHIZOME_PATH: OnceLock<Option<String>> = OnceLock::new();

/// Check if the Rhizome binary is available in PATH. Cached after first call.
pub fn is_available() -> bool {
    *RHIZOME_AVAILABLE.get_or_init(|| detect_rhizome().is_some())
}

/// Returns the cached path to the rhizome binary, if available.
pub fn rhizome_binary() -> Option<&'static str> {
    RHIZOME_PATH.get_or_init(detect_rhizome).as_deref()
}

/// Check config override, then auto-detection.
pub fn should_use_rhizome() -> bool {
    if let Ok(config) = crate::config::Config::load()
        && let Some(rhizome_config) = &config.filters.rhizome
        && let Some(enabled) = rhizome_config.enabled
    {
        return enabled && is_available();
    }
    is_available()
}

fn detect_rhizome() -> Option<String> {
    #[cfg(target_os = "windows")]
    let result = std::process::Command::new("where").arg("rhizome").output();

    #[cfg(not(target_os = "windows"))]
    let result = std::process::Command::new("which").arg("rhizome").output();

    match result {
        Ok(output) if output.status.success() => {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if path.is_empty() {
                None
            } else {
                Some(path.lines().next().unwrap_or(&path).to_string())
            }
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_rhizome_returns_option() {
        let result = detect_rhizome();
        if let Some(path) = &result {
            assert!(!path.is_empty());
        }
    }
}
