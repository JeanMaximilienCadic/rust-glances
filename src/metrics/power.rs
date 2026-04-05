//! Power usage monitoring via Intel RAPL (Running Average Power Limit).
//!
//! Reads cumulative energy counters from /sys/class/powercap/intel-rapl*/
//! and computes instantaneous power draw in watts.

use std::collections::HashMap;
use std::fs;
use std::time::Duration;

use serde::Serialize;

/// Power domain metrics.
#[derive(Clone, Default, Serialize)]
pub struct PowerMetrics {
    /// Per-domain power readings.
    pub domains: Vec<PowerDomain>,
    /// Total system power (sum of all package domains) in watts.
    pub total_power_w: f64,
    /// Whether RAPL data is available.
    pub available: bool,
}

/// A single RAPL power domain.
#[derive(Clone, Default, Serialize)]
pub struct PowerDomain {
    pub name: String,
    pub power_w: f64,
    pub max_power_w: f64,
}

/// Tracks previous energy readings for delta computation.
#[derive(Default)]
pub struct RaplState {
    /// Map from sysfs path -> last energy_uj reading.
    prev_energy: HashMap<String, u64>,
}

/// Discover and read RAPL power domains.
pub fn collect_power_metrics(state: &mut RaplState, elapsed: Duration) -> PowerMetrics {
    let elapsed_secs = elapsed.as_secs_f64();
    if elapsed_secs <= 0.0 {
        return PowerMetrics::default();
    }

    let mut domains = Vec::new();
    let mut total_power = 0.0;

    // Scan /sys/class/powercap/ for intel-rapl domains
    let powercap = "/sys/class/powercap";
    let entries = match fs::read_dir(powercap) {
        Ok(e) => e,
        Err(_) => return PowerMetrics::default(),
    };

    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        // Match top-level packages: intel-rapl:0, intel-rapl:1, etc.
        if !name.starts_with("intel-rapl:") || name.matches(':').count() != 1 {
            continue;
        }

        let base = entry.path();

        // Read the package domain itself
        if let Some(domain) = read_domain(&base, state, elapsed_secs) {
            total_power += domain.power_w;
            domains.push(domain);
        }

        // Read sub-domains (core, uncore, dram)
        if let Ok(sub_entries) = fs::read_dir(&base) {
            for sub in sub_entries.flatten() {
                let sub_name = sub.file_name().to_string_lossy().to_string();
                if sub_name.starts_with("intel-rapl:") {
                    if let Some(domain) = read_domain(&sub.path(), state, elapsed_secs) {
                        domains.push(domain);
                    }
                }
            }
        }
    }

    PowerMetrics {
        available: !domains.is_empty(),
        total_power_w: total_power,
        domains,
    }
}

fn read_domain(
    path: &std::path::Path,
    state: &mut RaplState,
    elapsed_secs: f64,
) -> Option<PowerDomain> {
    let name = fs::read_to_string(path.join("name")).ok()?.trim().to_string();
    let energy_uj = fs::read_to_string(path.join("energy_uj"))
        .ok()?
        .trim()
        .parse::<u64>()
        .ok()?;

    let max_power_w = fs::read_to_string(path.join("constraint_0_max_power_uw"))
        .ok()
        .and_then(|s| s.trim().parse::<u64>().ok())
        .map(|uw| uw as f64 / 1_000_000.0)
        .unwrap_or(0.0);

    let key = path.to_string_lossy().to_string();
    let power_w = if let Some(&prev) = state.prev_energy.get(&key) {
        // Handle counter wraparound
        let delta = if energy_uj >= prev {
            energy_uj - prev
        } else {
            // Counter wrapped — read max range
            let max_range = fs::read_to_string(path.join("max_energy_range_uj"))
                .ok()
                .and_then(|s| s.trim().parse::<u64>().ok())
                .unwrap_or(u64::MAX);
            max_range - prev + energy_uj
        };
        delta as f64 / 1_000_000.0 / elapsed_secs
    } else {
        0.0
    };

    state.prev_energy.insert(key, energy_uj);

    Some(PowerDomain {
        name,
        power_w,
        max_power_w,
    })
}
