//! Kubectl handlers: pods, services, logs, passthrough.
use crate::tracking;
use anyhow::{Context, Result};
use std::ffi::OsString;
use std::process::Command;

pub(crate) fn kubectl_pods(args: &[String], _verbose: u8) -> Result<()> {
    let timer = tracking::TimedExecution::start();

    let mut cmd = Command::new("kubectl");
    cmd.args(["get", "pods", "-o", "json"]);
    for arg in args {
        cmd.arg(arg);
    }

    let output = cmd.output().context("Failed to run kubectl get pods")?;
    let raw = String::from_utf8_lossy(&output.stdout).to_string();
    let mut out = String::new();

    let json: serde_json::Value = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(_) => {
            out.push_str("k8s: No pods found");
            println!("{}", out);
            timer.track("kubectl get pods", "mycelium kubectl pods", &raw, &out);
            return Ok(());
        }
    };

    let pods = match json["items"].as_array() {
        Some(items) if !items.is_empty() => items,
        _ => {
            out.push_str("k8s: No pods found");
            println!("{}", out);
            timer.track("kubectl get pods", "mycelium kubectl pods", &raw, &out);
            return Ok(());
        }
    };
    let (mut running, mut pending, mut failed, mut restarts_total) = (0, 0, 0, 0i64);
    let mut issues: Vec<String> = Vec::new();

    for pod in pods {
        let ns = pod["metadata"]["namespace"].as_str().unwrap_or("-");
        let name = pod["metadata"]["name"].as_str().unwrap_or("-");
        let phase = pod["status"]["phase"].as_str().unwrap_or("Unknown");

        if let Some(containers) = pod["status"]["containerStatuses"].as_array() {
            for c in containers {
                restarts_total += c["restartCount"].as_i64().unwrap_or(0);
            }
        }

        match phase {
            "Running" => running += 1,
            "Pending" => {
                pending += 1;
                issues.push(format!("{}/{} Pending", ns, name));
            }
            "Failed" | "Error" => {
                failed += 1;
                issues.push(format!("{}/{} {}", ns, name, phase));
            }
            _ => {
                if let Some(containers) = pod["status"]["containerStatuses"].as_array() {
                    for c in containers {
                        if let Some(w) = c["state"]["waiting"]["reason"].as_str()
                            && (w.contains("CrashLoop") || w.contains("Error"))
                        {
                            failed += 1;
                            issues.push(format!("{}/{} {}", ns, name, w));
                        }
                    }
                }
            }
        }
    }

    let mut parts = Vec::new();
    if running > 0 {
        parts.push(format!("{} ✓", running));
    }
    if pending > 0 {
        parts.push(format!("{} pending", pending));
    }
    if failed > 0 {
        parts.push(format!("{} ✗", failed));
    }
    if restarts_total > 0 {
        parts.push(format!("{} restarts", restarts_total));
    }

    out.push_str(&format!("k8s: {} pods: {}\n", pods.len(), parts.join(", ")));
    if !issues.is_empty() {
        out.push_str("[!] Issues:\n");
        for issue in issues.iter().take(10) {
            out.push_str(&format!("  {}\n", issue));
        }
        if issues.len() > 10 {
            out.push_str(&format!("  ... +{} more", issues.len() - 10));
        }
    }

    print!("{}", out);
    timer.track("kubectl get pods", "mycelium kubectl pods", &raw, &out);
    Ok(())
}

pub(crate) fn kubectl_services(args: &[String], _verbose: u8) -> Result<()> {
    let timer = tracking::TimedExecution::start();

    let mut cmd = Command::new("kubectl");
    cmd.args(["get", "services", "-o", "json"]);
    for arg in args {
        cmd.arg(arg);
    }

    let output = cmd.output().context("Failed to run kubectl get services")?;
    let raw = String::from_utf8_lossy(&output.stdout).to_string();
    let mut out = String::new();

    let json: serde_json::Value = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(_) => {
            out.push_str("k8s: No services found");
            println!("{}", out);
            timer.track("kubectl get svc", "mycelium kubectl svc", &raw, &out);
            return Ok(());
        }
    };

    let services = match json["items"].as_array() {
        Some(items) if !items.is_empty() => items,
        _ => {
            out.push_str("k8s: No services found");
            println!("{}", out);
            timer.track("kubectl get svc", "mycelium kubectl svc", &raw, &out);
            return Ok(());
        }
    };
    out.push_str(&format!("k8s: {} services:\n", services.len()));

    for svc in services.iter().take(15) {
        let ns = svc["metadata"]["namespace"].as_str().unwrap_or("-");
        let name = svc["metadata"]["name"].as_str().unwrap_or("-");
        let svc_type = svc["spec"]["type"].as_str().unwrap_or("-");
        let ports: Vec<String> = svc["spec"]["ports"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .map(|p| {
                        let port = p["port"].as_i64().unwrap_or(0);
                        let target = p["targetPort"]
                            .as_i64()
                            .or_else(|| p["targetPort"].as_str().and_then(|s| s.parse().ok()))
                            .unwrap_or(port);
                        if port == target {
                            format!("{}", port)
                        } else {
                            format!("{}→{}", port, target)
                        }
                    })
                    .collect()
            })
            .unwrap_or_default();
        out.push_str(&format!(
            "  {}/{} {} [{}]\n",
            ns,
            name,
            svc_type,
            ports.join(",")
        ));
    }
    if services.len() > 15 {
        out.push_str(&format!("  ... +{} more", services.len() - 15));
    }

    print!("{}", out);
    timer.track("kubectl get svc", "mycelium kubectl svc", &raw, &out);
    Ok(())
}

pub(crate) fn kubectl_logs(args: &[String], _verbose: u8) -> Result<()> {
    let timer = tracking::TimedExecution::start();

    let pod = args.first().map(|s| s.as_str()).unwrap_or("");
    if pod.is_empty() {
        println!("Usage: mycelium kubectl logs <pod>");
        return Ok(());
    }

    let mut cmd = Command::new("kubectl");
    cmd.args(["logs", "--tail", "100", pod]);
    for arg in args.iter().skip(1) {
        cmd.arg(arg);
    }

    let output = cmd.output().context("Failed to run kubectl logs")?;
    let raw = String::from_utf8_lossy(&output.stdout).to_string();
    let analyzed = crate::log_cmd::run_stdin_str(&raw);
    let out = format!("k8s: Logs for {}:\n{}", pod, analyzed);
    println!("{}", out);
    timer.track(
        &format!("kubectl logs {}", pod),
        "mycelium kubectl logs",
        &raw,
        &out,
    );
    Ok(())
}

/// Runs an unsupported kubectl subcommand by passing it through directly
pub fn run_kubectl_passthrough(args: &[OsString], verbose: u8) -> Result<()> {
    let timer = tracking::TimedExecution::start();

    if verbose > 0 {
        eprintln!("kubectl passthrough: {:?}", args);
    }
    let status = Command::new("kubectl")
        .args(args)
        .status()
        .context("Failed to run kubectl")?;

    let args_str = tracking::args_display(args);
    timer.track_passthrough(
        &format!("kubectl {}", args_str),
        &format!("mycelium kubectl {} (passthrough)", args_str),
    );

    if !status.success() {
        std::process::exit(status.code().unwrap_or(1));
    }
    Ok(())
}
