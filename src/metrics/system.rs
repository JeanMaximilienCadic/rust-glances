//! System metrics collection (CPU, memory, disk, network, processes).

use std::collections::HashMap;
use std::time::Duration;
use sysinfo::{Components, Disks, Networks, ProcessStatus, System, Users};

use crate::types::{CpuBreakdown, CpuInfo, DiskInfo, MemoryInfo, NetworkInfo, PortForwardRule, ProcessInfo, SystemMetrics};

/// Detect if a process is running inside a container.
/// Returns "D" for Docker, "L" for LXC, "C" for other containers, or empty string for host.
#[cfg(target_os = "linux")]
fn detect_virtualization(pid: u32) -> String {
    use std::fs;

    // Read /proc/{pid}/cgroup to check container membership
    let cgroup_path = format!("/proc/{}/cgroup", pid);
    if let Ok(content) = fs::read_to_string(&cgroup_path) {
        let content_lower = content.to_lowercase();
        if content_lower.contains("/docker/") || content_lower.contains("/docker-") {
            return "D".to_string();
        }
        if content_lower.contains("/lxc/") || content_lower.contains("/lxc.") {
            return "L".to_string();
        }
        if content_lower.contains("/containerd/") || content_lower.contains("/cri-containerd-") {
            return "C".to_string();
        }
        if content_lower.contains("/kubepods/") || content_lower.contains("/kubepods.") {
            return "K".to_string();
        }
        if content_lower.contains("/podman-") || content_lower.contains("/libpod-") {
            return "P".to_string();
        }
    }
    String::new()
}

#[cfg(not(target_os = "linux"))]
fn detect_virtualization(_pid: u32) -> String {
    // Virtualization detection not available on non-Linux platforms
    String::new()
}

/// Collect port forwarding rules from iptables NAT table (Linux only).
#[cfg(target_os = "linux")]
pub fn collect_port_forwards() -> Vec<PortForwardRule> {
    use std::process::Command;

    let mut rules = Vec::new();

    // Try iptables -t nat -L PREROUTING -n --line-numbers
    if let Ok(output) = Command::new("iptables")
        .args(["-t", "nat", "-L", "PREROUTING", "-n"])
        .output()
    {
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines().skip(2) {
                // Skip header lines
                if let Some(rule) = parse_iptables_dnat_line(line) {
                    rules.push(rule);
                }
            }
        }
    }

    // Also check DOCKER chain if it exists (Docker uses this for port forwarding)
    if let Ok(output) = Command::new("iptables")
        .args(["-t", "nat", "-L", "DOCKER", "-n"])
        .output()
    {
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines().skip(2) {
                if let Some(rule) = parse_iptables_dnat_line(line) {
                    rules.push(rule);
                }
            }
        }
    }

    // Sort and deduplicate
    rules.sort();
    rules.dedup();
    rules
}

#[cfg(target_os = "linux")]
fn parse_iptables_dnat_line(line: &str) -> Option<PortForwardRule> {
    // Example lines:
    // DNAT       tcp  --  0.0.0.0/0            0.0.0.0/0            tcp dpt:8080 to:172.17.0.2:80
    // DNAT       tcp  --  anywhere             anywhere             tcp dpt:443 to:192.168.1.100:443

    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.is_empty() || parts[0] != "DNAT" {
        return None;
    }

    let protocol = parts.get(1)?.to_string();

    // Find dpt: (destination port)
    let mut src_port: u16 = 0;
    let mut dest_ip = String::new();
    let mut dest_port: u16 = 0;

    for (i, part) in parts.iter().enumerate() {
        if part.starts_with("dpt:") {
            src_port = part.strip_prefix("dpt:")?.parse().ok()?;
        } else if part.starts_with("to:") {
            // Format: to:IP:PORT or to:IP
            let to_part = part.strip_prefix("to:")?;
            if let Some((ip, port)) = to_part.rsplit_once(':') {
                dest_ip = ip.to_string();
                dest_port = port.parse().ok()?;
            } else {
                dest_ip = to_part.to_string();
                dest_port = src_port;
            }
        } else if *part == "to" && i + 1 < parts.len() {
            // Handle "to 172.17.0.2:80" format (space separated)
            let next = parts[i + 1];
            if let Some((ip, port)) = next.rsplit_once(':') {
                dest_ip = ip.to_string();
                dest_port = port.parse().ok()?;
            }
        }
    }

    if src_port == 0 || dest_ip.is_empty() {
        return None;
    }

    // Source IP (usually 0.0.0.0/0 for any)
    let src_ip = parts.get(3).map(|s| {
        if *s == "0.0.0.0/0" || *s == "anywhere" {
            "any".to_string()
        } else {
            s.to_string()
        }
    }).unwrap_or_else(|| "any".to_string());

    Some(PortForwardRule {
        protocol,
        src_ip,
        src_port,
        dest_ip,
        dest_port,
        interface: String::new(),
    })
}

#[cfg(not(target_os = "linux"))]
pub fn collect_port_forwards() -> Vec<PortForwardRule> {
    // Port forwarding collection not available on non-Linux platforms
    Vec::new()
}

/// Collect battery information.
pub fn collect_battery() -> (Option<f64>, String, Option<f64>) {
    let Ok(manager) = battery::Manager::new() else {
        return (None, String::new(), None);
    };
    let Ok(mut batteries) = manager.batteries() else {
        return (None, String::new(), None);
    };
    if let Some(Ok(bat)) = batteries.next() {
        let pct = bat.state_of_charge().value as f64 * 100.0;
        let state = match bat.state() {
            battery::State::Charging => "Charging",
            battery::State::Discharging => "Discharging",
            battery::State::Full => "Full",
            battery::State::Empty => "Empty",
            _ => "Unknown",
        }.to_string();
        let tte = bat.time_to_empty().map(|t| t.value as f64 / 60.0); // minutes
        (Some(pct), state, tte)
    } else {
        (None, String::new(), None)
    }
}

/// Collect all system metrics.
pub fn collect_system_metrics(
    system: &System,
    networks: &Networks,
    disks: &Disks,
    components: &Components,
    users: &Users,
    last_network_stats: &mut HashMap<String, (u64, u64)>,
    last_disk_stats: &mut HashMap<String, (u64, u64)>,
    elapsed: Duration,
) -> SystemMetrics {
    let elapsed_secs = elapsed.as_secs_f64().max(0.001);

    // Hostname and OS info
    let hostname = System::host_name().unwrap_or_else(|| "unknown".into());
    let os_name = System::long_os_version().unwrap_or_else(|| "Unknown OS".into());
    let kernel_version = System::kernel_version().unwrap_or_else(|| "?".into());

    // Uptime and load
    let uptime = System::uptime();
    let load = System::load_average();
    let load_avg = (load.one, load.five, load.fifteen);

    // CPUs
    let cpus: Vec<CpuInfo> = system
        .cpus()
        .iter()
        .map(|cpu| CpuInfo {
            name: cpu.name().to_string(),
            usage: cpu.cpu_usage(),
            frequency: cpu.frequency(),
        })
        .collect();

    let cpu_count = cpus.len();
    let cpu_global = system.global_cpu_usage();

    // CPU breakdown placeholders (computed in single-pass below)
    let total_capacity = cpu_count as f64 * 100.0;

    // Memory - detailed breakdown
    let total_mem = system.total_memory();
    let used_mem = system.used_memory();
    let available_mem = system.available_memory();
    let free_mem = system.free_memory();

    // Inactive memory: difference between available and free
    let inactive = available_mem.saturating_sub(free_mem);

    let memory = MemoryInfo {
        total: total_mem,
        used: used_mem,
        free: free_mem,
        available: available_mem,
        inactive,
        swap_total: system.total_swap(),
        swap_used: system.used_swap(),
        swap_free: system.free_swap(),
    };

    // Single-pass: CPU breakdown + disk I/O aggregation + process list
    let mut total_user: f64 = 0.0;
    let mut total_disk_read: u64 = 0;
    let mut total_disk_write: u64 = 0;

    // User map for process info (built once)
    let user_map: HashMap<_, _> = users
        .iter()
        .map(|u| (u.id().clone(), u.name().to_string()))
        .collect();

    let mut processes: Vec<ProcessInfo> = Vec::with_capacity(system.processes().len());

    for (pid, proc) in system.processes() {
        // CPU breakdown
        let cpu = proc.cpu_usage() as f64;
        if matches!(proc.status(), ProcessStatus::Run) {
            total_user += cpu;
        }

        // Disk I/O aggregation
        let du = proc.disk_usage();
        total_disk_read += du.total_read_bytes;
        total_disk_write += du.total_written_bytes;

        // Build process info
        let user = proc
            .user_id()
            .and_then(|uid| user_map.get(uid))
            .cloned()
            .unwrap_or_else(|| "?".into());

        let status = match proc.status() {
            ProcessStatus::Run => "R",
            ProcessStatus::Sleep => "S",
            ProcessStatus::Idle => "I",
            ProcessStatus::Zombie => "Z",
            ProcessStatus::Stop => "T",
            _ => "?",
        }
        .to_string();

        let cmd: Vec<_> = proc
            .cmd()
            .iter()
            .map(|s| s.to_string_lossy().to_string())
            .collect();
        let command = if cmd.is_empty() {
            proc.name().to_string_lossy().to_string()
        } else {
            cmd.join(" ")
        };

        // Detect if process is running in a container (Linux only)
        let virtualized = detect_virtualization(pid.as_u32());

        processes.push(ProcessInfo {
            pid: pid.as_u32(),
            name: proc.name().to_string_lossy().to_string(),
            user,
            cpu_usage: proc.cpu_usage(),
            memory_usage: (proc.memory() as f32 / total_mem as f32) * 100.0,
            memory_bytes: proc.memory(),
            disk_read_rate: du.read_bytes as f64,
            disk_write_rate: du.written_bytes as f64,
            status,
            command,
            virtualized,
        });
    }

    let user_pct = (total_user / total_capacity * 100.0).min(100.0);
    let system_pct = ((cpu_global as f64) - user_pct).clamp(0.0, 100.0);
    let idle_pct = (100.0 - cpu_global as f64).max(0.0);
    let cpu_breakdown = CpuBreakdown {
        user: user_pct,
        system: system_pct,
        idle: idle_pct,
        nice: 0.0,
    };

    let process_count = processes.len();
    let thread_count = process_count;

    let (prev_total_r, prev_total_w) = last_disk_stats
        .get("_total")
        .copied()
        .unwrap_or((total_disk_read, total_disk_write));
    let total_disk_read_rate = total_disk_read.saturating_sub(prev_total_r) as f64 / elapsed_secs;
    let total_disk_write_rate = total_disk_write.saturating_sub(prev_total_w) as f64 / elapsed_secs;

    last_disk_stats.insert("_total".to_string(), (total_disk_read, total_disk_write));

    // Disks — group by filesystem name with all mount points (lsblk-style)
    let mut disk_map: indexmap::IndexMap<String, DiskInfo> = indexmap::IndexMap::new();
    for disk in disks.iter() {
        let mp = disk.mount_point().to_string_lossy();
        let name = disk.name().to_string_lossy();
        // Filter out pseudo-filesystems but keep real ones
        if mp.starts_with("/System/Volumes/")
            && mp != "/System/Volumes/Data"
            && !name.contains("disk")
        {
            continue;
        }
        let key = name.to_string();
        disk_map
            .entry(key)
            .and_modify(|info| {
                info.mount_points.push(mp.to_string());
            })
            .or_insert_with(|| DiskInfo {
                name: name.to_string(),
                mount_points: vec![mp.to_string()],
                total: disk.total_space(),
                used: disk.total_space() - disk.available_space(),
                fs_type: disk.file_system().to_string_lossy().to_string(),
                read_bytes: total_disk_read,
                write_bytes: total_disk_write,
                read_rate: total_disk_read_rate,
                write_rate: total_disk_write_rate,
            });
    }
    let disks_info: Vec<DiskInfo> = disk_map.into_values().collect();

    // Networks — show ALL interfaces, no filtering
    let networks_info: Vec<NetworkInfo> = networks
        .iter()
        .map(|(name, data)| {
            let (prev_rx, prev_tx) = last_network_stats
                .get(name)
                .copied()
                .unwrap_or((data.total_received(), data.total_transmitted()));

            let rx_bytes = data.total_received();
            let tx_bytes = data.total_transmitted();
            let rx_rate = (rx_bytes.saturating_sub(prev_rx)) as f64 / elapsed_secs;
            let tx_rate = (tx_bytes.saturating_sub(prev_tx)) as f64 / elapsed_secs;

            last_network_stats.insert(name.clone(), (rx_bytes, tx_bytes));

            NetworkInfo {
                interface: name.clone(),
                rx_bytes,
                tx_bytes,
                rx_rate,
                tx_rate,
            }
        })
        .collect();

    // Temperatures
    let temperatures: Vec<(String, f32)> = components
        .iter()
        .filter_map(|c| {
            let temp = c.temperature();
            if temp > 0.0 {
                Some((c.label().to_string(), temp))
            } else {
                None
            }
        })
        .collect();

    let (battery_pct, battery_state, battery_time_to_empty) = collect_battery();

    // Collect port forwarding rules (iptables NAT)
    let port_forwards = collect_port_forwards();

    SystemMetrics {
        hostname,
        os_name,
        kernel_version,
        uptime,
        load_avg,
        cpu_count,
        cpus,
        cpu_global,
        cpu_breakdown,
        memory,
        disks: disks_info,
        networks: networks_info,
        port_forwards,
        processes,
        process_count,
        thread_count,
        temperatures,
        total_disk_read_rate,
        total_disk_write_rate,
        battery_pct,
        battery_state,
        battery_time_to_empty,
        power: crate::metrics::power::PowerMetrics::default(),
    }
}
