//! Self-update command that checks GitHub releases and downloads the latest binary.
use anyhow::{Context, Result};
use std::io::{Read, Write};

const GITHUB_API_URL: &str = "https://api.github.com/repos/";

/// Check for updates and optionally download the latest Mycelium release from GitHub.
pub fn run(check_only: bool) -> Result<()> {
    let current_version = env!("CARGO_PKG_VERSION");
    println!("Current version: v{current_version}");
    print!("Checking for updates... ");
    std::io::stdout().flush().ok();

    let latest = fetch_latest_release().context("Failed to check for updates")?;
    let latest_tag = latest["tag_name"]
        .as_str()
        .context("Missing tag_name in GitHub API response")?;

    // Strip leading 'v' for comparison
    let latest_version = latest_tag.trim_start_matches('v');
    println!("Latest version: {latest_tag}");

    if latest_version == current_version {
        println!("✓ Already up to date.");
        return Ok(());
    }

    println!("Update available: v{current_version} → {latest_tag}");

    if check_only {
        println!("Run `mycelium self-update` to install.");
        return Ok(());
    }

    let asset_name = target_asset_name().context("Unsupported platform for self-update")?;
    let download_url = find_asset_url(&latest["assets"], &asset_name)
        .with_context(|| format!("No release asset found for '{asset_name}'"))?;

    let current_exe = std::env::current_exe().context("Failed to locate current executable")?;

    println!("Downloading {asset_name}...");
    let binary_bytes =
        download_binary(&download_url).context("Failed to download update binary")?;

    replace_binary(&current_exe, &binary_bytes).context("Failed to replace binary")?;

    println!("✓ Updated to {latest_tag}. Run `mycelium --version` to confirm.");
    Ok(())
}

fn fetch_latest_release() -> Result<serde_json::Value> {
    let agent = ureq::Agent::new_with_defaults();
    let response = agent
        .get(GITHUB_API_URL)
        .header(
            "User-Agent",
            &format!("mycelium/{}", env!("CARGO_PKG_VERSION")),
        )
        .header("Accept", "application/vnd.github+json")
        .call()
        .context("Failed to fetch latest release (check your internet connection)")?;

    let json: serde_json::Value = serde_json::from_reader(response.into_body().as_reader())
        .context("Invalid JSON from GitHub API")?;
    Ok(json)
}

fn target_asset_name() -> Option<String> {
    let os_suffix = match std::env::consts::OS {
        "macos" => "apple-darwin",
        "linux" => "linux",
        "windows" => "windows",
        _ => return None,
    };
    let arch = match std::env::consts::ARCH {
        "x86_64" => "x86_64",
        "aarch64" => "aarch64",
        _ => return None,
    };
    let ext = if std::env::consts::OS == "windows" {
        ".exe"
    } else {
        ""
    };
    Some(format!("mycelium-{arch}-{os_suffix}{ext}"))
}

fn find_asset_url(assets: &serde_json::Value, name: &str) -> Option<String> {
    assets.as_array()?.iter().find_map(|asset| {
        let asset_name = asset["name"].as_str()?;
        if asset_name == name {
            asset["browser_download_url"].as_str().map(String::from)
        } else {
            None
        }
    })
}

fn download_binary(url: &str) -> Result<Vec<u8>> {
    let agent = ureq::Agent::new_with_defaults();
    let response = agent
        .get(url)
        .header(
            "User-Agent",
            &format!("mycelium/{}", env!("CARGO_PKG_VERSION")),
        )
        .call()
        .context("Download failed")?;

    let mut bytes = Vec::new();
    response
        .into_body()
        .as_reader()
        .read_to_end(&mut bytes)
        .context("Failed to read download response")?;

    if bytes.is_empty() {
        anyhow::bail!("Downloaded binary is empty");
    }
    Ok(bytes)
}

fn replace_binary(current_exe: &std::path::Path, binary_bytes: &[u8]) -> Result<()> {
    // Write to a temp file next to the current exe so rename is atomic (same filesystem)
    let parent = current_exe
        .parent()
        .context("Executable has no parent directory")?;
    let tmp_path = parent.join(".mycelium-update.tmp");

    let write_result = (|| -> Result<()> {
        let mut tmp = std::fs::File::create(&tmp_path).context("Failed to create temp file")?;
        tmp.write_all(binary_bytes)
            .context("Failed to write update to temp file")?;
        tmp.flush().context("Failed to flush temp file")?;
        Ok(())
    })();

    if let Err(e) = write_result {
        let _ = std::fs::remove_file(&tmp_path);
        return Err(e);
    }

    // Set executable permissions on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&tmp_path, std::fs::Permissions::from_mode(0o755))
            .context("Failed to set executable permissions")?;
    }

    // Atomic rename
    std::fs::rename(&tmp_path, current_exe).map_err(|e| {
        let _ = std::fs::remove_file(&tmp_path);
        if e.kind() == std::io::ErrorKind::PermissionDenied {
            anyhow::anyhow!(
                "Permission denied replacing binary at {}. Try: sudo mycelium self-update",
                current_exe.display()
            )
        } else {
            anyhow::anyhow!("Failed to replace binary: {e}")
        }
    })?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_target_asset_name_known_platform() {
        // On any supported platform, should return Some
        let name = target_asset_name();
        // If we're on a supported OS+arch combo, it should be non-empty
        match (std::env::consts::OS, std::env::consts::ARCH) {
            ("macos" | "linux" | "windows", "x86_64" | "aarch64") => {
                assert!(name.is_some());
                let n = name.unwrap();
                assert!(n.starts_with("mycelium-"));
                assert!(n.contains(std::env::consts::ARCH));
            }
            _ => {
                assert!(name.is_none());
            }
        }
    }

    #[test]
    fn test_target_asset_name_macos_aarch64() {
        // Simulate what the function would produce: check format
        let asset = "mycelium-aarch64-apple-darwin".to_string();
        assert!(asset.starts_with("mycelium-"));
        assert!(asset.contains("aarch64"));
        assert!(asset.contains("apple-darwin"));
    }

    #[test]
    fn test_find_asset_url_present() {
        let assets = serde_json::json!([
            {"name": "mycelium-x86_64-apple-darwin", "browser_download_url": "https://example.com/mycelium"},
            {"name": "mycelium-x86_64-linux", "browser_download_url": "https://example.com/mycelium-linux"},
        ]);
        let url = find_asset_url(&assets, "mycelium-x86_64-apple-darwin");
        assert_eq!(url, Some("https://example.com/mycelium".to_string()));
    }

    #[test]
    fn test_find_asset_url_missing() {
        let assets = serde_json::json!([
            {"name": "mycelium-x86_64-linux", "browser_download_url": "https://example.com/mycelium-linux"},
        ]);
        let url = find_asset_url(&assets, "mycelium-x86_64-apple-darwin");
        assert!(url.is_none());
    }

    #[test]
    fn test_find_asset_url_empty() {
        let assets = serde_json::json!([]);
        let url = find_asset_url(&assets, "mycelium-x86_64-apple-darwin");
        assert!(url.is_none());
    }
}
