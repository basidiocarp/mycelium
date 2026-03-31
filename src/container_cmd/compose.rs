//! Docker Compose filters and execution functions.
use crate::tracking;
use anyhow::{Context, Result};
use std::ffi::OsString;
use std::process::Command;

use super::docker::compact_ports;

/// Format `docker compose ps --format` output into compact form.
/// Expects tab-separated lines: Name\tImage\tStatus\tPorts
/// (no header row — `--format` output is headerless)
pub fn format_compose_ps(raw: &str) -> String {
    let lines: Vec<&str> = raw.lines().filter(|l| !l.trim().is_empty()).collect();

    if lines.is_empty() {
        return "docker: 0 compose services".to_string();
    }

    let mut result = format!("docker: {} compose services:\n", lines.len());

    for line in lines.iter().take(20) {
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() >= 4 {
            let name = parts[0];
            let image = parts[1];
            let status = parts[2];
            let ports = parts[3];

            let short_image = image.split('/').next_back().unwrap_or(image);

            let port_str = if ports.trim().is_empty() {
                String::new()
            } else {
                let compact = compact_ports(ports.trim());
                if compact == "-" {
                    String::new()
                } else {
                    format!(" [{}]", compact)
                }
            };

            result.push_str(&format!(
                "  {} ({}) {}{}\n",
                name, short_image, status, port_str
            ));
        }
    }
    if lines.len() > 20 {
        result.push_str(&format!("  ... +{} more\n", lines.len() - 20));
    }

    result.trim_end().to_string()
}

/// Format `docker compose logs` output into compact form
pub fn format_compose_logs(raw: &str) -> String {
    if raw.trim().is_empty() {
        return "docker: No logs".to_string();
    }

    // docker compose logs prefixes each line with "service-N  | "
    // Use the existing log deduplication engine
    let analyzed = crate::log_cmd::run_stdin_str(raw);
    format!("docker: Compose logs:\n{}", analyzed)
}

/// Format `docker compose build` output into compact summary
pub fn format_compose_build(raw: &str) -> String {
    if raw.trim().is_empty() {
        return "docker: Build: no output".to_string();
    }

    let mut result = String::new();

    // Extract the summary line: "[+] Building 12.3s (8/8) FINISHED"
    for line in raw.lines() {
        if line.contains("Building") && line.contains("FINISHED") {
            result.push_str(&format!("docker: {}\n", line.trim()));
            break;
        }
    }

    if result.is_empty() {
        // No FINISHED line found — might still be building or errored
        if let Some(line) = raw.lines().find(|l| l.contains("Building")) {
            result.push_str(&format!("docker: {}\n", line.trim()));
        } else {
            result.push_str("docker: Build:\n");
        }
    }

    // Collect unique service names from build steps like "[web 1/4]"
    let mut services: Vec<String> = Vec::new();
    // find('[') returns byte offset — use byte slicing throughout
    // '[' and ']' are single-byte ASCII, so byte arithmetic is safe
    for line in raw.lines() {
        if let Some(start) = line.find('[')
            && let Some(end) = line[start + 1..].find(']')
        {
            let bracket = &line[start + 1..start + 1 + end];
            let svc = bracket.split_whitespace().next().unwrap_or("");
            if !svc.is_empty() && svc != "+" && !services.contains(&svc.to_string()) {
                services.push(svc.to_string());
            }
        }
    }

    if !services.is_empty() {
        result.push_str(&format!("  Services: {}\n", services.join(", ")));
    }

    // Count build steps (lines starting with " => ")
    let step_count = raw
        .lines()
        .filter(|l| l.trim_start().starts_with("=> "))
        .count();
    if step_count > 0 {
        result.push_str(&format!("  Steps: {}", step_count));
    }

    result.trim_end().to_string()
}

/// Run `docker compose ps` with compact output
pub fn run_compose_ps(verbose: u8) -> Result<()> {
    let timer = tracking::TimedExecution::start();

    // Raw output for token tracking
    let raw_output = Command::new("docker")
        .args(["compose", "ps"])
        .output()
        .context("Failed to run docker compose ps")?;

    if !raw_output.status.success() {
        let stderr = String::from_utf8_lossy(&raw_output.stderr);
        eprintln!("{}", stderr);
        std::process::exit(raw_output.status.code().unwrap_or(1));
    }
    let raw = String::from_utf8_lossy(&raw_output.stdout).to_string();

    // Structured output for parsing (same pattern as docker_ps)
    let output = Command::new("docker")
        .args([
            "compose",
            "ps",
            "--format",
            "{{.Name}}\t{{.Image}}\t{{.Status}}\t{{.Ports}}",
        ])
        .output()
        .context("Failed to run docker compose ps --format")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        eprintln!("{}", stderr);
        std::process::exit(output.status.code().unwrap_or(1));
    }
    let structured = String::from_utf8_lossy(&output.stdout).to_string();

    if verbose > 0 {
        eprintln!("raw docker compose ps:\n{}", raw);
    }

    let out = format_compose_ps(&structured);
    println!("{}", out);
    timer.track(
        "docker compose ps",
        "mycelium docker compose ps",
        &raw,
        &out,
    );
    Ok(())
}

/// Run `docker compose logs` with deduplication
pub fn run_compose_logs(service: Option<&str>, verbose: u8) -> Result<()> {
    let timer = tracking::TimedExecution::start();

    let mut cmd = Command::new("docker");
    cmd.args(["compose", "logs", "--tail", "100"]);
    if let Some(svc) = service {
        cmd.arg(svc);
    }

    let output = cmd.output().context("Failed to run docker compose logs")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        eprintln!("{}", stderr);
        std::process::exit(output.status.code().unwrap_or(1));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let raw = format!("{}\n{}", stdout, stderr);

    if verbose > 0 {
        eprintln!("raw docker compose logs:\n{}", raw);
    }

    let out = format_compose_logs(&raw);
    println!("{}", out);
    let svc_label = service.unwrap_or("all");
    timer.track(
        &format!("docker compose logs {}", svc_label),
        "mycelium docker compose logs",
        &raw,
        &out,
    );
    Ok(())
}

/// Run `docker compose build` with summary output
pub fn run_compose_build(service: Option<&str>, verbose: u8) -> Result<()> {
    let timer = tracking::TimedExecution::start();

    let mut cmd = Command::new("docker");
    cmd.args(["compose", "build"]);
    if let Some(svc) = service {
        cmd.arg(svc);
    }

    let output = cmd.output().context("Failed to run docker compose build")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        eprintln!("{}", stderr);
        std::process::exit(output.status.code().unwrap_or(1));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let raw = format!("{}\n{}", stdout, stderr);

    if verbose > 0 {
        eprintln!("raw docker compose build:\n{}", raw);
    }

    let out = format_compose_build(&raw);
    println!("{}", out);
    let svc_label = service.unwrap_or("all");
    timer.track(
        &format!("docker compose build {}", svc_label),
        "mycelium docker compose build",
        &raw,
        &out,
    );
    Ok(())
}

/// Runs an unsupported docker compose subcommand by passing it through directly
pub fn run_compose_passthrough(args: &[OsString], verbose: u8) -> Result<()> {
    let timer = tracking::TimedExecution::start();

    if verbose > 0 {
        eprintln!("docker compose passthrough: {:?}", args);
    }
    let status = Command::new("docker")
        .arg("compose")
        .args(args)
        .status()
        .context("Failed to run docker compose")?;

    let args_str = tracking::args_display(args);
    timer.track_passthrough(
        &format!("docker compose {}", args_str),
        &format!("mycelium docker compose {} (passthrough)", args_str),
    );

    if !status.success() {
        std::process::exit(status.code().unwrap_or(1));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── format_compose_ps ──────────────────────────────────

    #[test]
    fn test_format_compose_ps_basic() {
        // Tab-separated --format output: Name\tImage\tStatus\tPorts
        let raw = "web-1\tnginx:latest\tUp 2 hours\t0.0.0.0:80->80/tcp\n\
                   api-1\tnode:20\tUp 2 hours\t0.0.0.0:3000->3000/tcp\n\
                   db-1\tpostgres:16\tUp 2 hours\t0.0.0.0:5432->5432/tcp";
        let out = format_compose_ps(raw);
        assert!(out.contains("3"), "should show container count");
        assert!(out.contains("web"), "should show service name");
        assert!(out.contains("api"), "should show service name");
        assert!(out.contains("db"), "should show service name");
        assert!(out.contains("Up 2 hours"), "should show status");
        assert!(out.len() < raw.len(), "output should be shorter than raw");
    }

    #[test]
    fn test_format_compose_ps_empty() {
        let out = format_compose_ps("");
        assert!(out.contains("0"), "should show zero containers");
    }

    #[test]
    fn test_format_compose_ps_whitespace_only() {
        let out = format_compose_ps("   \n  \n");
        assert!(out.contains("0"), "should show zero containers");
    }

    #[test]
    fn test_format_compose_ps_exited_service() {
        // Tab-separated --format output
        let raw = "worker-1\tpython:3.12\tExited (1) 2 minutes ago\t";
        let out = format_compose_ps(raw);
        assert!(out.contains("worker"), "should show service name");
        assert!(out.contains("Exited"), "should show exited status");
    }

    #[test]
    fn test_format_compose_ps_no_ports() {
        let raw = "redis-1\tredis:7\tUp 5 hours\t";
        let out = format_compose_ps(raw);
        assert!(out.contains("redis"), "should show service name");
        assert!(
            !out.contains("["),
            "should not show port brackets when empty"
        );
    }

    #[test]
    fn test_format_compose_ps_long_image_path() {
        let raw = "app-1\tghcr.io/myorg/myapp:latest\tUp 1 hour\t0.0.0.0:8080->8080/tcp";
        let out = format_compose_ps(raw);
        assert!(
            out.contains("myapp:latest"),
            "should shorten image to last segment"
        );
        assert!(
            !out.contains("ghcr.io"),
            "should not show full registry path"
        );
    }

    // ── format_compose_logs ────────────────────────────────

    #[test]
    fn test_format_compose_logs_basic() {
        let raw = "\
web-1  | 192.168.1.1 - GET / 200
web-1  | 192.168.1.1 - GET /favicon.ico 404
api-1  | Server listening on port 3000
api-1  | Connected to database";
        let out = format_compose_logs(raw);
        assert!(
            out.contains("Compose logs"),
            "should have compose logs header"
        );
    }

    #[test]
    fn test_format_compose_logs_empty() {
        let out = format_compose_logs("");
        assert!(out.contains("No logs"), "should indicate no logs");
    }

    // ── format_compose_build ───────────────────────────────

    #[test]
    fn test_format_compose_build_basic() {
        let raw = "\
[+] Building 12.3s (8/8) FINISHED
 => [web internal] load build definition from Dockerfile           0.0s
 => [web internal] load metadata for docker.io/library/node:20     1.2s
 => [web 1/4] FROM docker.io/library/node:20@sha256:abc123         0.0s
 => [web 2/4] WORKDIR /app                                         0.1s
 => [web 3/4] COPY package*.json ./                                0.1s
 => [web 4/4] RUN npm install                                      8.5s
 => [web] exporting to image                                       2.3s
 => => naming to docker.io/library/myapp-web                       0.0s";
        let out = format_compose_build(raw);
        assert!(out.contains("12.3s"), "should show total build time");
        assert!(out.contains("web"), "should show service name");
        assert!(out.len() < raw.len(), "should be shorter than raw");
    }

    #[test]
    fn test_format_compose_build_empty() {
        let out = format_compose_build("");
        assert!(
            !out.is_empty(),
            "should produce output even for empty input"
        );
    }

    #[test]
    fn test_compose_ps_token_savings() {
        fn count_tokens(text: &str) -> usize {
            crate::tracking::estimate_tokens(text)
        }

        let input = include_str!("../../tests/fixtures/docker_compose_ps_raw.txt");
        let output = format_compose_ps(input);

        let input_tokens = count_tokens(input);
        let output_tokens = count_tokens(&output);
        let savings = (input_tokens.saturating_sub(output_tokens)) * 100 / input_tokens.max(1);

        assert!(
            savings >= 60,
            "docker compose ps filter: expected >= 60% token savings, got {}%",
            savings
        );
    }
}
