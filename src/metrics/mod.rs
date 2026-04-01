//! Metrics collection modules.

pub mod docker;
mod gpu;
mod system;

pub use gpu::{collect_gpu_metrics, GpuHandle};
pub use system::collect_system_metrics;
