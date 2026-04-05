//! User interface rendering modules.

pub mod alerts;
mod dialogs;
#[cfg(feature = "docker")]
pub mod docker;
mod footer;
#[cfg(feature = "gpu")]
mod gpu;
mod graphs;
mod header;
#[cfg(feature = "docker")]
pub mod http_dialog;
mod layout;
#[cfg(feature = "docker")]
pub mod logs_dialog;
mod ports;
mod processes;
mod system;
pub mod tabs;
pub mod temps;

pub use layout::render_ui;
