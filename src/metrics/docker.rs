//! Docker container metrics collection.

use std::collections::HashMap;
use tokio::runtime::Runtime;

/// Information about a running Docker container.
#[derive(Clone, Default, Debug)]
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
    pub created: i64,
    pub uptime_secs: u64,
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

        // Port mappings
        let ports = container
            .ports
            .as_ref()
            .map(|ports| {
                ports
                    .iter()
                    .filter_map(|p| {
                        let private = p.private_port;
                        let public = p.public_port.unwrap_or(0);
                        let proto = p.typ.as_ref().map(|t| format!("{t}")).unwrap_or_default();
                        if public > 0 {
                            Some(format!("{}->{}/{}", public, private, proto))
                        } else {
                            Some(format!("{}/{}", private, proto))
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .unwrap_or_default();

        // Uptime from created timestamp
        let created = container.created.unwrap_or(0);
        let uptime_secs = if created > 0 {
            (chrono::Utc::now().timestamp() - created).max(0) as u64
        } else {
            0
        };

        result.push(ContainerInfo {
            id: short_id,
            name,
            image,
            status,
            state,
            ports,
            created,
            uptime_secs,
            ..Default::default()
        });
    }

    result
}

/// Collect stats for containers (CPU/memory/network) — called separately to keep list fast.
pub fn collect_docker_stats(
    handle: &DockerHandle,
    containers: &mut [ContainerInfo],
    _prev_cpu: &mut HashMap<String, (u64, u64)>,
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
            // CPU
            let cpu_delta = stats
                .cpu_stats
                .cpu_usage
                .total_usage
                .saturating_sub(stats.precpu_stats.cpu_usage.total_usage);
            let system_delta = stats
                .cpu_stats
                .system_cpu_usage
                .unwrap_or(0)
                .saturating_sub(stats.precpu_stats.system_cpu_usage.unwrap_or(0));
            let num_cpus = stats.cpu_stats.online_cpus.unwrap_or(1);

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
