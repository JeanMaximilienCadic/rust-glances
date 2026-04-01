//! System metrics collection (CPU, memory, disk, network, processes).

use std::collections::HashMap;
use std::time::Duration;
use sysinfo::{Components, Disks, Networks, ProcessStatus, System, Users};

use crate::types::{CpuBreakdown, CpuInfo, DiskInfo, MemoryInfo, NetworkInfo, ProcessInfo, SystemMetrics};

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

    // CPU breakdown estimate from process stats
    let mut total_user: f64 = 0.0;
    let mut _total_system: f64 = 0.0;
    for (_pid, proc) in system.processes() {
        let cpu = proc.cpu_usage() as f64;
        match proc.status() {
            ProcessStatus::Run => total_user += cpu,
            _ => _total_system += cpu * 0.1, // rough estimate
        }
    }
    // Normalize to percentage of total CPU capacity
    let total_capacity = cpu_count as f64 * 100.0;
    let user_pct = (total_user / total_capacity * 100.0).min(100.0);
    let system_pct = ((cpu_global as f64) - user_pct).max(0.0).min(100.0);
    let idle_pct = (100.0 - cpu_global as f64).max(0.0);

    let cpu_breakdown = CpuBreakdown {
        user: user_pct,
        system: system_pct,
        idle: idle_pct,
        nice: 0.0,
    };

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

    // Aggregate disk I/O from all processes
    let mut total_disk_read: u64 = 0;
    let mut total_disk_write: u64 = 0;
    for (_pid, proc) in system.processes() {
        let du = proc.disk_usage();
        total_disk_read += du.total_read_bytes;
        total_disk_write += du.total_written_bytes;
    }

    let (prev_total_r, prev_total_w) = last_disk_stats
        .get("_total")
        .copied()
        .unwrap_or((total_disk_read, total_disk_write));
    let total_disk_read_rate = total_disk_read.saturating_sub(prev_total_r) as f64 / elapsed_secs;
    let total_disk_write_rate = total_disk_write.saturating_sub(prev_total_w) as f64 / elapsed_secs;

    last_disk_stats.insert("_total".to_string(), (total_disk_read, total_disk_write));

    // Disks
    let disks_info: Vec<DiskInfo> = disks
        .iter()
        .filter(|disk| {
            let mp = disk.mount_point().to_string_lossy();
            let name = disk.name().to_string_lossy();
            // Filter out pseudo-filesystems but keep real ones
            !mp.starts_with("/System/Volumes/")
                || mp == "/System/Volumes/Data"
                || name.contains("disk")
        })
        .map(|disk| {
            DiskInfo {
                name: disk.name().to_string_lossy().to_string(),
                mount_point: disk.mount_point().to_string_lossy().to_string(),
                total: disk.total_space(),
                used: disk.total_space() - disk.available_space(),
                fs_type: disk.file_system().to_string_lossy().to_string(),
                read_bytes: total_disk_read,
                write_bytes: total_disk_write,
                read_rate: total_disk_read_rate,
                write_rate: total_disk_write_rate,
            }
        })
        .collect();

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

    // User map for process info
    let user_map: HashMap<_, _> = users
        .iter()
        .map(|u| (u.id().clone(), u.name().to_string()))
        .collect();

    // Processes
    let processes: Vec<ProcessInfo> = system
        .processes()
        .iter()
        .map(|(pid, proc)| {
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

            let du = proc.disk_usage();

            ProcessInfo {
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
            }
        })
        .collect();

    let process_count = system.processes().len();
    // Approximate thread count
    let thread_count = system.processes().len();

    let (battery_pct, battery_state, battery_time_to_empty) = collect_battery();

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
        processes,
        process_count,
        thread_count,
        temperatures,
        total_disk_read_rate,
        total_disk_write_rate,
        battery_pct,
        battery_state,
        battery_time_to_empty,
    }
}
