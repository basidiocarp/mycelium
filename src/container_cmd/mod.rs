//! Compact filters for Docker and Kubernetes commands (ps, images, logs, pods, services).

pub mod compose;
pub mod docker;
pub mod kubectl;

use anyhow::Result;

/// Supported Docker and Kubernetes subcommands with compact output.
#[derive(Debug, Clone, Copy)]
pub enum ContainerCmd {
    DockerPs,
    DockerImages,
    DockerLogs,
    KubectlPods,
    KubectlServices,
    KubectlLogs,
}

/// Route a container command to its specialized compact filter.
pub fn run(cmd: ContainerCmd, args: &[String], verbose: u8) -> Result<()> {
    match cmd {
        ContainerCmd::DockerPs => docker::docker_ps(verbose),
        ContainerCmd::DockerImages => docker::docker_images(verbose),
        ContainerCmd::DockerLogs => docker::docker_logs(args, verbose),
        ContainerCmd::KubectlPods => kubectl::kubectl_pods(args, verbose),
        ContainerCmd::KubectlServices => kubectl::kubectl_services(args, verbose),
        ContainerCmd::KubectlLogs => kubectl::kubectl_logs(args, verbose),
    }
}

pub use compose::{run_compose_build, run_compose_logs, run_compose_passthrough, run_compose_ps};
pub use docker::run_docker_passthrough;
pub use kubectl::run_kubectl_passthrough;
