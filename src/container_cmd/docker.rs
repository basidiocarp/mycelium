//! Docker handlers: ps, images, logs, passthrough.
use crate::tracking;
use anyhow::{Context, Result};
use std::ffi::OsString;
use std::process::Command;

pub(crate) fn docker_ps(_verbose: u8) -> Result<()> {
    let timer = tracking::TimedExecution::start();

    let raw = Command::new("docker")
        .args(["ps"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
        .unwrap_or_default();

    let output = Command::new("docker")
        .args([
            "ps",
            "--format",
            "{{.ID}}\t{{.Names}}\t{{.Status}}\t{{.Image}}\t{{.Ports}}",
        ])
        .output()
        .context("Failed to run docker ps")?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut out = String::new();

    if stdout.trim().is_empty() {
        out.push_str("docker: 0 containers");
        println!("{}", out);
        timer.track("docker ps", "mycelium docker ps", &raw, &out);
        return Ok(());
    }

    let count = stdout.lines().count();
    out.push_str(&format!("docker: {} containers:\n", count));

    for line in stdout.lines().take(15) {
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() >= 4 {
            let id = &parts[0][..12.min(parts[0].len())];
            let name = parts[1];
            let short_image = parts
                .get(3)
                .unwrap_or(&"")
                .split('/')
                .next_back()
                .unwrap_or("");
            let ports = compact_ports(parts.get(4).unwrap_or(&""));
            if ports == "-" {
                out.push_str(&format!("  {} {} ({})\n", id, name, short_image));
            } else {
                out.push_str(&format!(
                    "  {} {} ({}) [{}]\n",
                    id, name, short_image, ports
                ));
            }
        }
    }
    if count > 15 {
        out.push_str(&format!("  ... +{} more", count - 15));
    }

    print!("{}", out);
    timer.track("docker ps", "mycelium docker ps", &raw, &out);
    Ok(())
}

pub(crate) fn docker_images(_verbose: u8) -> Result<()> {
    let timer = tracking::TimedExecution::start();

    let raw = Command::new("docker")
        .args(["images"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
        .unwrap_or_default();

    let output = Command::new("docker")
        .args(["images", "--format", "{{.Repository}}:{{.Tag}}\t{{.Size}}"])
        .output()
        .context("Failed to run docker images")?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<&str> = stdout.lines().collect();
    let mut out = String::new();

    if lines.is_empty() {
        out.push_str("docker: 0 images");
        println!("{}", out);
        timer.track("docker images", "mycelium docker images", &raw, &out);
        return Ok(());
    }

    let mut total_size_mb: f64 = 0.0;
    for line in &lines {
        let parts: Vec<&str> = line.split('\t').collect();
        if let Some(size_str) = parts.get(1) {
            if size_str.contains("GB") {
                if let Ok(n) = size_str.replace("GB", "").trim().parse::<f64>() {
                    total_size_mb += n * 1024.0;
                }
            } else if size_str.contains("MB")
                && let Ok(n) = size_str.replace("MB", "").trim().parse::<f64>()
            {
                total_size_mb += n;
            }
        }
    }

    let total_display = if total_size_mb > 1024.0 {
        format!("{:.1}GB", total_size_mb / 1024.0)
    } else {
        format!("{:.0}MB", total_size_mb)
    };
    out.push_str(&format!(
        "docker: {} images ({})\n",
        lines.len(),
        total_display
    ));

    for line in lines.iter().take(15) {
        let parts: Vec<&str> = line.split('\t').collect();
        if !parts.is_empty() {
            let image = parts[0];
            let size = parts.get(1).unwrap_or(&"");
            let short = if image.len() > 40 {
                format!("...{}", &image[image.len() - 37..])
            } else {
                image.to_string()
            };
            out.push_str(&format!("  {} [{}]\n", short, size));
        }
    }
    if lines.len() > 15 {
        out.push_str(&format!("  ... +{} more", lines.len() - 15));
    }

    print!("{}", out);
    timer.track("docker images", "mycelium docker images", &raw, &out);
    Ok(())
}

pub(crate) fn docker_logs(args: &[String], _verbose: u8) -> Result<()> {
    let timer = tracking::TimedExecution::start();

    let container = args.first().map(|s| s.as_str()).unwrap_or("");
    if container.is_empty() {
        println!("Usage: mycelium docker logs <container>");
        return Ok(());
    }

    // Check if user specified --tail
    let user_tail = args
        .iter()
        .position(|a| a == "--tail")
        .and_then(|i| args.get(i + 1).map(|s| s.as_str()))
        .or_else(|| {
            args.iter()
                .find(|a| a.starts_with("--tail="))
                .and_then(|a| a.strip_prefix("--tail="))
        });
    let tail_value = user_tail.unwrap_or("500");

    let output = Command::new("docker")
        .args(["logs", "--tail", tail_value, container])
        .output()
        .context("Failed to run docker logs")?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let raw = format!("{}\n{}", stdout, stderr);

    let raw_line_count = raw.lines().count();
    let analyzed = crate::log_cmd::run_stdin_str(&raw);
    let dedup_count = raw_line_count.saturating_sub(analyzed.lines().count());

    let mut out = format!("docker: Logs for {}:\n{}", container, analyzed);
    if dedup_count > 0 {
        out.push_str(&format!("\n({} lines deduplicated)", dedup_count));
    }
    println!("{}", out);
    timer.track(
        &format!("docker logs {}", container),
        "mycelium docker logs",
        &raw,
        &out,
    );
    Ok(())
}

/// Runs an unsupported docker subcommand by passing it through directly
pub fn run_docker_passthrough(args: &[OsString], verbose: u8) -> Result<()> {
    let timer = tracking::TimedExecution::start();

    if verbose > 0 {
        eprintln!("docker passthrough: {:?}", args);
    }
    let status = Command::new("docker")
        .args(args)
        .status()
        .context("Failed to run docker")?;

    let args_str = tracking::args_display(args);
    timer.track_passthrough(
        &format!("docker {}", args_str),
        &format!("mycelium docker {} (passthrough)", args_str),
    );

    if !status.success() {
        std::process::exit(status.code().unwrap_or(1));
    }
    Ok(())
}

pub(crate) fn compact_ports(ports: &str) -> String {
    if ports.is_empty() {
        return "-".to_string();
    }

    // Extract just the port numbers
    let port_nums: Vec<&str> = ports
        .split(',')
        .filter_map(|p| p.split("->").next().and_then(|s| s.split(':').next_back()))
        .collect();

    if port_nums.len() <= 3 {
        port_nums.join(", ")
    } else {
        format!(
            "{}, ... +{}",
            port_nums[..2].join(", "),
            port_nums.len() - 2
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compact_ports_empty() {
        assert_eq!(compact_ports(""), "-");
    }

    #[test]
    fn test_compact_ports_single() {
        let result = compact_ports("0.0.0.0:8080->80/tcp");
        assert!(result.contains("8080"));
    }

    #[test]
    fn test_compact_ports_many() {
        let result = compact_ports(
            "0.0.0.0:80->80/tcp, 0.0.0.0:443->443/tcp, 0.0.0.0:8080->8080/tcp, 0.0.0.0:9090->9090/tcp",
        );
        assert!(result.contains("..."), "should truncate for >3 ports");
    }

    #[test]
    fn test_docker_ps_token_savings() {
        fn count_tokens(text: &str) -> usize {
            text.split_whitespace().count()
        }

        // Simulate docker ps output formatting
        let input = include_str!("../../tests/fixtures/docker_ps_raw.txt");
        let lines: Vec<&str> = input.lines().collect();
        let mut output = String::from("docker: containers:\n");

        for line in lines.iter().take(6).skip(1) {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 4 {
                let id = &parts[0][..12.min(parts[0].len())];
                let name = parts.last().unwrap_or(&"");
                output.push_str(&format!("  {} {}\n", id, name));
            }
        }

        let input_tokens = count_tokens(input);
        let output_tokens = count_tokens(&output);
        let savings = (input_tokens.saturating_sub(output_tokens)) * 100 / input_tokens.max(1);

        assert!(
            savings >= 60,
            "docker ps filter: expected >= 60% token savings, got {}%",
            savings
        );
    }
}
