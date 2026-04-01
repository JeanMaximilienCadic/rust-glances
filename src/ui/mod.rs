//! User interface rendering modules.

pub mod alerts;
mod dialogs;
pub mod docker;
mod footer;
mod gpu;
mod graphs;
mod header;
pub mod http_dialog;
mod layout;
pub mod logs_dialog;
mod processes;
mod system;
pub mod tabs;
pub mod temps;

pub use layout::render_ui;
