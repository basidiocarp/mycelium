/// Format crate name + version into a display string
pub(crate) fn format_crate_info(name: &str, version: &str, fallback: &str) -> String {
    if name.is_empty() {
        fallback.to_string()
    } else if version.is_empty() {
        name.to_string()
    } else {
        format!("{} {}", name, version)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_crate_info() {
        assert_eq!(
            format_crate_info("mycelium", "v0.11.0", ""),
            "mycelium v0.11.0"
        );
        assert_eq!(format_crate_info("mycelium", "", ""), "mycelium");
        assert_eq!(format_crate_info("", "", "package"), "package");
        assert_eq!(format_crate_info("", "v0.1.0", "fallback"), "fallback");
    }
}
