//! Rhizome integration — optional code intelligence for the read command.

use spore::{Tool, discover};

/// Check if the Rhizome binary is available in PATH. Cached by spore.
pub fn is_available() -> bool {
    discover(Tool::Rhizome).is_some()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_available_does_not_panic() {
        let _available = is_available();
    }
}
