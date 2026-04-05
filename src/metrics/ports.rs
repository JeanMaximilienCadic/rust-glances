//! Port-to-process mapping by parsing /proc/net/tcp{,6}.

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use serde::Serialize;
use sysinfo::{System, Users};

/// A process listening on a network port.
#[derive(Clone, Default, Serialize)]
pub struct PortProcessInfo {
    pub pid: u32,
    pub name: String,
    pub user: String,
    pub port: u16,
    pub protocol: String,
    pub bind_address: String,
    pub state: String,
    pub cpu_usage: f32,
    pub memory_bytes: u64,
    pub command: String,
}

/// Collect all processes listening on TCP/TCP6 ports.
pub fn collect_port_processes(system: &System, users: &Users) -> Vec<PortProcessInfo> {
    // Step 1: build inode -> (pid, name, user, cpu, mem, cmd) map
    let inode_to_pid = build_inode_pid_map(system, users);

    // Step 2: parse /proc/net/tcp and /proc/net/tcp6 for LISTEN sockets
    let mut results = Vec::new();
    for (path, proto) in [("/proc/net/tcp", "tcp"), ("/proc/net/tcp6", "tcp6")] {
        if let Ok(contents) = fs::read_to_string(path) {
            for line in contents.lines().skip(1) {
                if let Some(entry) = parse_proc_net_line(line, proto) {
                    // state 0A = LISTEN
                    if entry.state == "LISTEN" {
                        if let Some(proc_info) = inode_to_pid.get(&entry.inode) {
                            results.push(PortProcessInfo {
                                pid: proc_info.0,
                                name: proc_info.1.clone(),
                                user: proc_info.2.clone(),
                                port: entry.port,
                                protocol: proto.to_string(),
                                bind_address: entry.bind_address,
                                state: entry.state,
                                cpu_usage: proc_info.3,
                                memory_bytes: proc_info.4,
                                command: proc_info.5.clone(),
                            });
                        }
                    }
                }
            }
        }
    }

    // Sort by port number
    results.sort_by_key(|p| p.port);
    // Deduplicate: same pid + port (tcp and tcp6 both report same listener)
    results.dedup_by(|a, b| a.pid == b.pid && a.port == b.port);
    results
}

struct NetEntry {
    bind_address: String,
    port: u16,
    state: String,
    inode: u64,
}

fn parse_proc_net_line(line: &str, proto: &str) -> Option<NetEntry> {
    let fields: Vec<&str> = line.split_whitespace().collect();
    if fields.len() < 10 {
        return None;
    }

    // fields[1] = local_address (hex_ip:hex_port)
    let local = fields[1];
    // For tcp6, the format is "HEXIP:HEXPORT" where HEXIP is 32 chars
    // Use rfind to split on the last colon (safe for both tcp and tcp6)
    let colon_pos = local.rfind(':')?;
    let ip_hex = &local[..colon_pos];
    let port_hex = &local[colon_pos + 1..];

    let port = u16::from_str_radix(port_hex, 16).ok()?;
    let state_hex = fields[3];
    let inode = fields[9].parse::<u64>().ok()?;

    let state = match state_hex {
        "0A" => "LISTEN",
        "01" => "ESTABLISHED",
        "06" => "TIME_WAIT",
        "08" => "CLOSE_WAIT",
        _ => "OTHER",
    }
    .to_string();

    let bind_address = if proto == "tcp" {
        parse_ipv4_hex(ip_hex)
    } else {
        parse_ipv6_hex(ip_hex)
    };

    Some(NetEntry {
        bind_address,
        port,
        state,
        inode,
    })
}

fn parse_ipv4_hex(hex: &str) -> String {
    if hex.len() != 8 {
        return hex.to_string();
    }
    let bytes: Vec<u8> = (0..4)
        .filter_map(|i| u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).ok())
        .collect();
    if bytes.len() == 4 {
        // /proc/net/tcp stores in little-endian on little-endian systems
        format!("{}.{}.{}.{}", bytes[3], bytes[2], bytes[1], bytes[0])
    } else {
        hex.to_string()
    }
}

fn parse_ipv6_hex(hex: &str) -> String {
    if hex.len() != 32 {
        return hex.to_string();
    }
    // Check for all-zeros (::)
    if hex == "00000000000000000000000000000000" {
        return "::".to_string();
    }
    // Check for IPv4-mapped ::ffff:x.x.x.x
    let lower = hex.to_lowercase();
    if lower.starts_with("0000000000000000ffff0000") {
        let ipv4_part = &hex[24..];
        return format!("::ffff:{}", parse_ipv4_hex(ipv4_part));
    }
    // /proc/net/tcp6 stores addresses as 4 x 32-bit words in host byte order.
    // Each 8-char group needs byte-reversal on little-endian systems.
    let mut groups = Vec::new();
    for word_idx in 0..4 {
        let word_hex = &hex[word_idx * 8..word_idx * 8 + 8];
        // Reverse bytes within each 32-bit word
        let b0 = &word_hex[6..8];
        let b1 = &word_hex[4..6];
        let b2 = &word_hex[2..4];
        let b3 = &word_hex[0..2];
        // Split into two 16-bit groups
        groups.push(format!("{}{}", b0, b1));
        groups.push(format!("{}{}", b2, b3));
    }
    // Simplify leading zeros per group
    let simplified: Vec<String> = groups
        .iter()
        .map(|g| {
            let trimmed = g.trim_start_matches('0');
            if trimmed.is_empty() { "0".to_string() } else { trimmed.to_string() }
        })
        .collect();
    simplified.join(":")
}

/// Build a map from socket inode -> (pid, name, user, cpu_usage, memory_bytes, command).
fn build_inode_pid_map(
    system: &System,
    users: &Users,
) -> HashMap<u64, (u32, String, String, f32, u64, String)> {
    let mut map = HashMap::new();

    for (pid, proc_info) in system.processes() {
        let pid_num = pid.as_u32();
        let fd_dir = format!("/proc/{}/fd", pid_num);
        let fd_path = Path::new(&fd_dir);

        if let Ok(entries) = fs::read_dir(fd_path) {
            for entry in entries.flatten() {
                if let Ok(link) = fs::read_link(entry.path()) {
                    let link_str = link.to_string_lossy();
                    if let Some(inode_str) = link_str
                        .strip_prefix("socket:[")
                        .and_then(|s| s.strip_suffix(']'))
                    {
                        if let Ok(inode) = inode_str.parse::<u64>() {
                            let user = proc_info
                                .user_id()
                                .and_then(|uid| users.iter().find(|u| u.id() == uid))
                                .map(|u| u.name().to_string())
                                .unwrap_or_default();
                            let cmd_vec: Vec<String> = proc_info
                                .cmd()
                                .iter()
                                .map(|s| s.to_string_lossy().to_string())
                                .collect();
                            map.insert(
                                inode,
                                (
                                    pid_num,
                                    proc_info.name().to_string_lossy().to_string(),
                                    user,
                                    proc_info.cpu_usage(),
                                    proc_info.memory(),
                                    cmd_vec.join(" "),
                                ),
                            );
                        }
                    }
                }
            }
        }
    }

    map
}
