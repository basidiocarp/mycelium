//! AWS EC2 – Elastic Compute Cloud handler.

use crate::tracking;
use anyhow::Result;
use serde_json::Value;

use super::generic::{MAX_ITEMS, run_aws_json};

/// AWS EC2 – Elastic Compute Cloud.
///
/// `args` is the operation and its flags, e.g. `["describe-instances"]`.
pub fn run_ec2(args: &[String], verbose: u8) -> Result<()> {
    match args.first().map(String::as_str) {
        Some("describe-instances") => run_ec2_describe(&args[1..], verbose),
        _ => super::generic::run_generic("ec2", args, verbose),
    }
}

fn run_ec2_describe(extra_args: &[String], verbose: u8) -> Result<()> {
    let timer = tracking::TimedExecution::start();
    let (raw, stderr, status) = run_aws_json(&["ec2", "describe-instances"], extra_args, verbose)?;

    if !status.success() {
        timer.track(
            "aws ec2 describe-instances",
            "mycelium aws ec2 describe-instances",
            &stderr,
            &stderr,
        );
        std::process::exit(status.code().unwrap_or(1));
    }

    let filtered = match filter_ec2_instances(&raw) {
        Some(f) => f,
        None => raw.clone(),
    };
    println!("{}", filtered);

    timer.track(
        "aws ec2 describe-instances",
        "mycelium aws ec2 describe-instances",
        &raw,
        &filtered,
    );
    Ok(())
}

fn filter_ec2_instances(json_str: &str) -> Option<String> {
    let v: Value = serde_json::from_str(json_str).ok()?;
    let reservations = v["Reservations"].as_array()?;

    let mut instances: Vec<String> = Vec::new();
    for res in reservations {
        if let Some(insts) = res["Instances"].as_array() {
            for inst in insts {
                let id = inst["InstanceId"].as_str().unwrap_or("?");
                let state = inst["State"]["Name"].as_str().unwrap_or("?");
                let itype = inst["InstanceType"].as_str().unwrap_or("?");
                let ip = inst["PrivateIpAddress"].as_str().unwrap_or("-");

                // Extract Name tag
                let name = inst["Tags"]
                    .as_array()
                    .and_then(|tags| tags.iter().find(|t| t["Key"].as_str() == Some("Name")))
                    .and_then(|t| t["Value"].as_str())
                    .unwrap_or("-");

                instances.push(format!("{} {} {} {} ({})", id, state, itype, ip, name));
            }
        }
    }

    let total = instances.len();
    let mut result = format!("EC2: {} instances\n", total);

    for inst in instances.iter().take(MAX_ITEMS) {
        result.push_str(&format!("  {}\n", inst));
    }

    if total > MAX_ITEMS {
        result.push_str(&format!("  ... +{} more\n", total - MAX_ITEMS));
    }

    Some(result.trim_end().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_snapshot_ec2_instances() {
        let json = r#"{"Reservations":[{"Instances":[{"InstanceId":"i-0a1b2c3d4e5f00001","InstanceType":"t3.micro","PrivateIpAddress":"10.0.1.10","State":{"Code":16,"Name":"running"},"Tags":[{"Key":"Name","Value":"web-server-1"}],"BlockDeviceMappings":[],"SecurityGroups":[]},{"InstanceId":"i-0a1b2c3d4e5f00002","InstanceType":"t3.large","PrivateIpAddress":"10.0.2.20","State":{"Code":80,"Name":"stopped"},"Tags":[{"Key":"Name","Value":"worker-1"}],"BlockDeviceMappings":[],"SecurityGroups":[]}]}]}"#;
        let result = filter_ec2_instances(json).unwrap();
        assert!(result.contains("EC2: 2 instances"));
        assert!(result.contains("i-0a1b2c3d4e5f00001 running t3.micro 10.0.1.10 (web-server-1)"));
        assert!(result.contains("i-0a1b2c3d4e5f00002 stopped t3.large 10.0.2.20 (worker-1)"));
    }

    #[test]
    fn test_filter_ec2_instances() {
        let json = r#"{
            "Reservations": [{
                "Instances": [{
                    "InstanceId": "i-abc123",
                    "State": {"Name": "running"},
                    "InstanceType": "t3.micro",
                    "PrivateIpAddress": "10.0.1.5",
                    "Tags": [{"Key": "Name", "Value": "web-server"}]
                }, {
                    "InstanceId": "i-def456",
                    "State": {"Name": "stopped"},
                    "InstanceType": "t3.large",
                    "PrivateIpAddress": "10.0.1.6",
                    "Tags": [{"Key": "Name", "Value": "worker"}]
                }]
            }]
        }"#;
        let result = filter_ec2_instances(json).unwrap();
        assert!(result.contains("EC2: 2 instances"));
        assert!(result.contains("i-abc123 running t3.micro 10.0.1.5 (web-server)"));
        assert!(result.contains("i-def456 stopped t3.large 10.0.1.6 (worker)"));
    }

    #[test]
    fn test_filter_ec2_no_name_tag() {
        let json = r#"{
            "Reservations": [{
                "Instances": [{
                    "InstanceId": "i-abc123",
                    "State": {"Name": "running"},
                    "InstanceType": "t3.micro",
                    "PrivateIpAddress": "10.0.1.5",
                    "Tags": []
                }]
            }]
        }"#;
        let result = filter_ec2_instances(json).unwrap();
        assert!(result.contains("(-)"));
    }

    #[test]
    fn test_filter_ec2_invalid_json() {
        assert!(filter_ec2_instances("not json").is_none());
    }

    fn count_tokens(text: &str) -> usize {
        text.split_whitespace().count()
    }

    #[test]
    fn test_ec2_token_savings() {
        let json = r#"{
    "Reservations": [{
        "ReservationId": "r-001",
        "OwnerId": "123456789012",
        "Groups": [],
        "Instances": [{
            "InstanceId": "i-0a1b2c3d4e5f00001",
            "ImageId": "ami-0abcdef1234567890",
            "InstanceType": "t3.micro",
            "KeyName": "my-key-pair",
            "LaunchTime": "2024-01-15T10:30:00+00:00",
            "Placement": { "AvailabilityZone": "us-east-1a", "GroupName": "", "Tenancy": "default" },
            "PrivateDnsName": "ip-10-0-1-10.ec2.internal",
            "PrivateIpAddress": "10.0.1.10",
            "PublicDnsName": "ec2-54-0-0-10.compute-1.amazonaws.com",
            "PublicIpAddress": "54.0.0.10",
            "State": { "Code": 16, "Name": "running" },
            "SubnetId": "subnet-0abc123def456001",
            "VpcId": "vpc-0abc123def456001",
            "Architecture": "x86_64",
            "BlockDeviceMappings": [{ "DeviceName": "/dev/xvda", "Ebs": { "AttachTime": "2024-01-15T10:30:05+00:00", "DeleteOnTermination": true, "Status": "attached", "VolumeId": "vol-001" } }],
            "EbsOptimized": false,
            "EnaSupport": true,
            "Hypervisor": "xen",
            "NetworkInterfaces": [{ "NetworkInterfaceId": "eni-001", "PrivateIpAddress": "10.0.1.10", "Status": "in-use" }],
            "RootDeviceName": "/dev/xvda",
            "RootDeviceType": "ebs",
            "SecurityGroups": [{ "GroupId": "sg-001", "GroupName": "web-server-sg" }],
            "SourceDestCheck": true,
            "Tags": [{ "Key": "Name", "Value": "web-server-1" }, { "Key": "Environment", "Value": "production" }, { "Key": "Team", "Value": "backend" }],
            "VirtualizationType": "hvm",
            "CpuOptions": { "CoreCount": 1, "ThreadsPerCore": 2 },
            "MetadataOptions": { "State": "applied", "HttpTokens": "required", "HttpEndpoint": "enabled" }
        }]
    }]
}"#;
        let result = filter_ec2_instances(json).unwrap();
        let input_tokens = count_tokens(json);
        let output_tokens = count_tokens(&result);
        let savings = 100.0 - (output_tokens as f64 / input_tokens as f64 * 100.0);
        assert!(
            savings >= 60.0,
            "EC2 filter: expected >=60% savings, got {:.1}%",
            savings
        );
    }
}
