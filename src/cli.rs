//! CLI argument parsing.

use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "glances", version, about = "A modern system monitor in Rust")]
pub struct Cli {
    /// Refresh rate in milliseconds
    #[arg(short, long, default_value_t = 1000)]
    pub refresh: u64,

    /// Disable GPU monitoring
    #[arg(long)]
    pub no_gpu: bool,

    /// Disable Docker monitoring
    #[arg(long)]
    pub no_docker: bool,

    /// Start in compact mode
    #[arg(short, long)]
    pub compact: bool,

    /// Disable graphs
    #[arg(long)]
    pub no_graphs: bool,

    /// Show all processes (including idle)
    #[arg(short, long)]
    pub all: bool,

    /// Show per-core CPU bars
    #[arg(long)]
    pub per_core: bool,

    /// Print GPU detection info and exit
    #[arg(long, hide = true)]
    pub debug_gpu: bool,

    /// Run as a web server instead of TUI
    #[arg(short, long)]
    pub web: bool,

    /// Web server port (default: 61208)
    #[arg(long, default_value_t = 61208)]
    pub port: u16,

    /// Web server bind address (default: 0.0.0.0)
    #[arg(long, default_value = "0.0.0.0")]
    pub bind: String,

    /// TLS certificate file (PEM). If provided with --tls-key, serves HTTPS with this cert instead of self-signed.
    #[arg(long)]
    pub tls_cert: Option<String>,

    /// TLS private key file (PEM). Required with --tls-cert.
    #[arg(long)]
    pub tls_key: Option<String>,
}
