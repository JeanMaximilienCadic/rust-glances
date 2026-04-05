//! Metrics collection modules.

#[cfg(feature = "docker")]
pub mod docker;

#[cfg(feature = "gpu")]
mod gpu;

pub mod ports;
pub mod power;
mod system;

#[cfg(feature = "gpu")]
pub use gpu::{collect_gpu_metrics, GpuHandle};

pub use system::collect_system_metrics;

// Stub types when features are disabled
#[cfg(not(feature = "gpu"))]
pub struct GpuHandle;

#[cfg(not(feature = "gpu"))]
impl GpuHandle {
    pub fn new() -> Self {
        Self
    }
}

#[cfg(not(feature = "gpu"))]
pub fn collect_gpu_metrics(
    _handle: &GpuHandle,
    _system: &sysinfo::System,
    _users: &sysinfo::Users,
) -> Option<crate::types::GpuMetrics> {
    None
}
