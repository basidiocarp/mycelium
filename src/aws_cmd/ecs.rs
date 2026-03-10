//! AWS ECS – Elastic Container Service handler.

use crate::tracking;
use crate::utils::join_with_overflow;
use anyhow::Result;
use serde_json::Value;

use super::generic::{MAX_ITEMS, run_aws_json};

/// AWS ECS – Elastic Container Service.
///
/// `args` is the operation and its flags, e.g. `["list-services", "--cluster", "my-cluster"]`.
pub fn run_ecs(args: &[String], verbose: u8) -> Result<()> {
    match args.first().map(String::as_str) {
        Some("list-services") => run_ecs_list_services(&args[1..], verbose),
        Some("describe-services") => run_ecs_describe_services(&args[1..], verbose),
        _ => super::generic::run_generic("ecs", args, verbose),
    }
}

fn run_ecs_list_services(extra_args: &[String], verbose: u8) -> Result<()> {
    let timer = tracking::TimedExecution::start();
    let (raw, stderr, status) = run_aws_json(&["ecs", "list-services"], extra_args, verbose)?;

    if !status.success() {
        timer.track(
            "aws ecs list-services",
            "mycelium aws ecs list-services",
            &stderr,
            &stderr,
        );
        std::process::exit(status.code().unwrap_or(1));
    }

    let filtered = match filter_ecs_list_services(&raw) {
        Some(f) => f,
        None => raw.clone(),
    };
    println!("{}", filtered);

    timer.track(
        "aws ecs list-services",
        "mycelium aws ecs list-services",
        &raw,
        &filtered,
    );
    Ok(())
}

fn run_ecs_describe_services(extra_args: &[String], verbose: u8) -> Result<()> {
    let timer = tracking::TimedExecution::start();
    let (raw, stderr, status) = run_aws_json(&["ecs", "describe-services"], extra_args, verbose)?;

    if !status.success() {
        timer.track(
            "aws ecs describe-services",
            "mycelium aws ecs describe-services",
            &stderr,
            &stderr,
        );
        std::process::exit(status.code().unwrap_or(1));
    }

    let filtered = match filter_ecs_describe_services(&raw) {
        Some(f) => f,
        None => raw.clone(),
    };
    println!("{}", filtered);

    timer.track(
        "aws ecs describe-services",
        "mycelium aws ecs describe-services",
        &raw,
        &filtered,
    );
    Ok(())
}

fn filter_ecs_list_services(json_str: &str) -> Option<String> {
    let v: Value = serde_json::from_str(json_str).ok()?;
    let arns = v["serviceArns"].as_array()?;

    let mut result = Vec::new();
    let total = arns.len();

    for arn in arns.iter().take(MAX_ITEMS) {
        let arn_str = arn.as_str().unwrap_or("?");
        // Extract short name from ARN: arn:aws:ecs:...:service/cluster/name -> name
        let short = arn_str.rsplit('/').next().unwrap_or(arn_str);
        result.push(short.to_string());
    }

    Some(join_with_overflow(&result, total, MAX_ITEMS, "services"))
}

fn filter_ecs_describe_services(json_str: &str) -> Option<String> {
    let v: Value = serde_json::from_str(json_str).ok()?;
    let services = v["services"].as_array()?;

    let mut result = Vec::new();
    let total = services.len();

    for svc in services.iter().take(MAX_ITEMS) {
        let name = svc["serviceName"].as_str().unwrap_or("?");
        let status = svc["status"].as_str().unwrap_or("?");
        let running = svc["runningCount"].as_i64().unwrap_or(0);
        let desired = svc["desiredCount"].as_i64().unwrap_or(0);
        let launch = svc["launchType"].as_str().unwrap_or("?");
        result.push(format!(
            "{} {} {}/{} ({})",
            name, status, running, desired, launch
        ));
    }

    Some(join_with_overflow(&result, total, MAX_ITEMS, "services"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filter_ecs_list_services() {
        let json = r#"{
            "serviceArns": [
                "arn:aws:ecs:us-east-1:123:service/cluster/api-service",
                "arn:aws:ecs:us-east-1:123:service/cluster/worker-service"
            ]
        }"#;
        let result = filter_ecs_list_services(json).unwrap();
        assert!(result.contains("api-service"));
        assert!(result.contains("worker-service"));
        assert!(!result.contains("arn:aws"));
    }

    #[test]
    fn test_filter_ecs_describe_services() {
        let json = r#"{
            "services": [{
                "serviceName": "api",
                "status": "ACTIVE",
                "runningCount": 3,
                "desiredCount": 3,
                "launchType": "FARGATE"
            }]
        }"#;
        let result = filter_ecs_describe_services(json).unwrap();
        assert_eq!(result, "api ACTIVE 3/3 (FARGATE)");
    }
}
