//! AWS RDS – Relational Database Service handler.

use crate::tracking;
use crate::utils::join_with_overflow;
use anyhow::Result;
use serde_json::Value;

use super::generic::{run_aws_json, MAX_ITEMS};

/// AWS RDS – Relational Database Service.
///
/// `args` is the operation and its flags, e.g. `["describe-db-instances"]`.
pub fn run_rds(args: &[String], verbose: u8) -> Result<()> {
    match args.first().map(String::as_str) {
        Some("describe-db-instances") => run_rds_describe(&args[1..], verbose),
        _ => super::generic::run_generic("rds", args, verbose),
    }
}

fn run_rds_describe(extra_args: &[String], verbose: u8) -> Result<()> {
    let timer = tracking::TimedExecution::start();
    let (raw, stderr, status) =
        run_aws_json(&["rds", "describe-db-instances"], extra_args, verbose)?;

    if !status.success() {
        timer.track(
            "aws rds describe-db-instances",
            "mycelium aws rds describe-db-instances",
            &stderr,
            &stderr,
        );
        std::process::exit(status.code().unwrap_or(1));
    }

    let filtered = match filter_rds_instances(&raw) {
        Some(f) => f,
        None => raw.clone(),
    };
    println!("{}", filtered);

    timer.track(
        "aws rds describe-db-instances",
        "mycelium aws rds describe-db-instances",
        &raw,
        &filtered,
    );
    Ok(())
}

fn filter_rds_instances(json_str: &str) -> Option<String> {
    let v: Value = serde_json::from_str(json_str).ok()?;
    let dbs = v["DBInstances"].as_array()?;

    let mut result = Vec::new();
    let total = dbs.len();

    for db in dbs.iter().take(MAX_ITEMS) {
        let name = db["DBInstanceIdentifier"].as_str().unwrap_or("?");
        let engine = db["Engine"].as_str().unwrap_or("?");
        let version = db["EngineVersion"].as_str().unwrap_or("?");
        let class = db["DBInstanceClass"].as_str().unwrap_or("?");
        let status = db["DBInstanceStatus"].as_str().unwrap_or("?");
        result.push(format!(
            "{} {} {} {} {}",
            name, engine, version, class, status
        ));
    }

    Some(join_with_overflow(&result, total, MAX_ITEMS, "instances"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filter_rds_instances() {
        let json = r#"{
            "DBInstances": [{
                "DBInstanceIdentifier": "mydb",
                "Engine": "postgres",
                "EngineVersion": "15.4",
                "DBInstanceClass": "db.t3.micro",
                "DBInstanceStatus": "available"
            }]
        }"#;
        let result = filter_rds_instances(json).unwrap();
        assert_eq!(result, "mydb postgres 15.4 db.t3.micro available");
    }

    #[test]
    fn test_rds_overflow() {
        let mut dbs = Vec::new();
        for i in 1..=25 {
            dbs.push(format!(
                r#"{{"DBInstanceIdentifier": "db-{}", "Engine": "postgres", "EngineVersion": "15.4", "DBInstanceClass": "db.t3.micro", "DBInstanceStatus": "available"}}"#,
                i
            ));
        }
        let json = format!(r#"{{"DBInstances": [{}]}}"#, dbs.join(","));
        let result = filter_rds_instances(&json).unwrap();
        assert!(result.contains("... +5 more instances"));
    }
}
