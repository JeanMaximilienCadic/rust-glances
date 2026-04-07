//! Docker container metrics collection.

use std::collections::HashMap;
use tokio::runtime::Runtime;

/// Port mapping info for a container.
#[derive(Clone, Debug, serde::Serialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct PortMapping {
    pub host_port: u16,
    pub container_port: u16,
    pub protocol: String,
    pub host_ip: String,
}

/// Volume mount info for a container.
#[derive(Clone, Debug, serde::Serialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct VolumeMount {
    pub source: String,
    pub destination: String,
    pub mode: String, // "rw" or "ro"
    pub mount_type: String, // "bind", "volume", "tmpfs"
}

/// Information about a running Docker container.
#[derive(Clone, Default, Debug, serde::Serialize)]
#[allow(dead_code)]
pub struct ContainerInfo {
    pub id: String,
    pub name: String,
    pub image: String,
    pub status: String,
    pub state: String,
    pub cpu_percent: f64,
    pub memory_used: u64,
    pub memory_limit: u64,
    pub memory_percent: f64,
    pub net_rx: u64,
    pub net_tx: u64,
    pub block_read: u64,
    pub block_write: u64,
    pub ports: String,
    pub port_mappings: Vec<PortMapping>,
    pub volume_mounts: Vec<VolumeMount>,
    pub created: i64,
    pub uptime_secs: u64,
    pub compose_project: String,
    pub compose_service: String,
    pub compose_dir: String,
}

/// Docker metrics handle — owns its own tokio runtime for async bollard calls.
pub struct DockerHandle {
    docker: Option<bollard::Docker>,
    runtime: Option<Runtime>,
}

impl DockerHandle {
    pub fn new() -> Self {
        let runtime = Runtime::new().ok();
        let docker = runtime.as_ref().and_then(|rt| {
            rt.block_on(async {
                bollard::Docker::connect_with_local_defaults().ok()
            })
        });
        Self { docker, runtime }
    }
}

/// Collect Docker container list (fast — no stats).
pub fn collect_docker_metrics(handle: &DockerHandle) -> Vec<ContainerInfo> {
    let Some(ref docker) = handle.docker else {
        return Vec::new();
    };
    let Some(ref rt) = handle.runtime else {
        return Vec::new();
    };

    // List running containers
    let containers = match rt.block_on(docker.list_containers(Some(
        bollard::container::ListContainersOptions::<String> {
            all: false,
            ..Default::default()
        },
    ))) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };

    let mut result = Vec::new();

    for container in containers {
        let id = container.id.clone().unwrap_or_default();
        let short_id = id[..12.min(id.len())].to_string();
        let name = container
            .names
            .as_ref()
            .and_then(|n| n.first())
            .map(|n| n.trim_start_matches('/').to_string())
            .unwrap_or_else(|| short_id.clone());
        let image = container.image.clone().unwrap_or_default();
        let status = container.status.clone().unwrap_or_default();
        let state = container.state.clone().unwrap_or_default();

        // Port mappings - parse into structs and sort
        let mut port_mappings: Vec<PortMapping> = container
            .ports
            .as_ref()
            .map(|ports| {
                ports
                    .iter()
                    .filter_map(|p| {
                        let container_port = p.private_port;
                        let host_port = p.public_port.unwrap_or(0);
                        let protocol = p.typ.as_ref().map(|t| format!("{t}")).unwrap_or_else(|| "tcp".into());
                        let host_ip = p.ip.clone().unwrap_or_default();
                        if host_port > 0 {
                            Some(PortMapping {
                                host_port,
                                container_port,
                                protocol,
                                host_ip,
                            })
                        } else {
                            None
                        }
                    })
                    .collect()
            })
            .unwrap_or_default();

        // Sort ports by host_port for consistent display
        port_mappings.sort();

        // Format ports string from sorted mappings
        let ports = port_mappings
            .iter()
            .map(|p| {
                if p.host_ip.is_empty() || p.host_ip == "0.0.0.0" {
                    format!("{}:{}/{}", p.host_port, p.container_port, p.protocol)
                } else {
                    format!("{}:{}:{}/{}", p.host_ip, p.host_port, p.container_port, p.protocol)
                }
            })
            .collect::<Vec<_>>()
            .join(", ");

        // Volume mounts
        let mut volume_mounts: Vec<VolumeMount> = container
            .mounts
            .as_ref()
            .map(|mounts| {
                mounts
                    .iter()
                    .map(|m| {
                        VolumeMount {
                            source: m.source.clone().unwrap_or_default(),
                            destination: m.destination.clone().unwrap_or_default(),
                            mode: if m.rw.unwrap_or(true) { "rw".into() } else { "ro".into() },
                            mount_type: m.typ.as_ref().map(|t| format!("{t}")).unwrap_or_else(|| "bind".into()),
                        }
                    })
                    .collect()
            })
            .unwrap_or_default();

        // Sort mounts by destination for consistent display
        volume_mounts.sort_by(|a, b| a.destination.cmp(&b.destination));

        // Uptime from created timestamp
        let created = container.created.unwrap_or(0);
        let uptime_secs = if created > 0 {
            (chrono::Utc::now().timestamp() - created).max(0) as u64
        } else {
            0
        };

        // Extract docker-compose labels
        let labels = container.labels.as_ref();
        let compose_project = labels
            .and_then(|l| l.get("com.docker.compose.project").cloned())
            .unwrap_or_default();
        let compose_service = labels
            .and_then(|l| l.get("com.docker.compose.service").cloned())
            .unwrap_or_default();
        let compose_dir = labels
            .and_then(|l| l.get("com.docker.compose.project.working_dir").cloned())
            .unwrap_or_default();

        result.push(ContainerInfo {
            id: short_id,
            name,
            image,
            status,
            state,
            ports,
            port_mappings,
            volume_mounts,
            created,
            uptime_secs,
            compose_project,
            compose_service,
            compose_dir,
            ..Default::default()
        });
    }

    result
}

/// Collect stats for containers (CPU/memory/network) — called separately to keep list fast.
/// `prev_cpu` stores (container_cpu_usage, system_cpu_usage) per container for delta calculation.
pub fn collect_docker_stats(
    handle: &DockerHandle,
    containers: &mut [ContainerInfo],
    prev_cpu: &mut HashMap<String, (u64, u64)>,
) {
    let Some(ref docker) = handle.docker else { return };
    let Some(ref rt) = handle.runtime else { return };

    for container in containers.iter_mut() {
        let id = container.id.clone();
        let stats_future = async {
            use futures_util::StreamExt;
            let mut stream = docker.stats(
                &id,
                Some(bollard::container::StatsOptions {
                    stream: false,
                    one_shot: true,
                }),
            );
            // Timeout: skip stats if Docker is slow
            tokio::time::timeout(
                std::time::Duration::from_millis(500),
                stream.next(),
            ).await
        };

        let stats_result = rt.block_on(stats_future);

        if let Ok(Some(Ok(stats))) = stats_result {
            // CPU: use our own stored previous values for reliable delta
            let current_cpu = stats.cpu_stats.cpu_usage.total_usage;
            let current_system = stats.cpu_stats.system_cpu_usage.unwrap_or(0);
            let num_cpus = stats.cpu_stats.online_cpus.unwrap_or(1);

            // Get previous values or use precpu_stats as fallback
            let (prev_cpu_usage, prev_system_usage) = prev_cpu
                .get(&id)
                .copied()
                .unwrap_or((
                    stats.precpu_stats.cpu_usage.total_usage,
                    stats.precpu_stats.system_cpu_usage.unwrap_or(0),
                ));

            // Store current values for next iteration
            prev_cpu.insert(id.clone(), (current_cpu, current_system));

            // Calculate deltas
            let cpu_delta = current_cpu.saturating_sub(prev_cpu_usage);
            let system_delta = current_system.saturating_sub(prev_system_usage);

            if system_delta > 0 {
                container.cpu_percent =
                    (cpu_delta as f64 / system_delta as f64) * num_cpus as f64 * 100.0;
            }

            // Memory
            if let Some(mem_usage) = stats.memory_stats.usage {
                container.memory_used = mem_usage;
            }
            if let Some(mem_limit) = stats.memory_stats.limit {
                if mem_limit > 0 {
                    container.memory_limit = mem_limit;
                    container.memory_percent =
                        (container.memory_used as f64 / mem_limit as f64) * 100.0;
                }
            }

            // Network I/O
            if let Some(networks) = stats.networks {
                for (_name, net) in networks {
                    container.net_rx += net.rx_bytes;
                    container.net_tx += net.tx_bytes;
                }
            }

            // Block I/O
            if let Some(blk_stats) = stats.blkio_stats.io_service_bytes_recursive {
                for entry in blk_stats {
                    match entry.op.as_str() {
                        "read" | "Read" => container.block_read += entry.value,
                        "write" | "Write" => container.block_write += entry.value,
                        _ => {}
                    }
                }
            }
        }
    }
}
