//! glances - A modern system and GPU monitoring TUI written in Rust

mod app;
mod cli;
mod metrics;
mod types;
mod ui;
mod utils;

use std::io;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use clap::Parser;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

use app::App;
use cli::Cli;
use ui::render_ui;

#[cfg(feature = "gpu")]
fn debug_gpu() {
    use metrics::GpuHandle;
    use sysinfo::{System, Users};

    println!("GPU Detection Debug");
    println!("===================\n");

    let handle = GpuHandle::new();

    #[cfg(not(target_os = "macos"))]
    {
        if handle.nvml.is_some() {
            println!("NVML: Initialized successfully!");
        } else {
            println!("NVML: Failed to initialize");
            println!("\nPossible causes:");
            println!("  - NVIDIA drivers not installed");
            println!("  - libnvidia-ml.so not found in library path");
            println!("  - Permission denied\n");
            println!("Try: ls -la /usr/lib/x86_64-linux-gnu/libnvidia-ml*");
            return;
        }
    }

    let system = System::new_all();
    let users = Users::new_with_refreshed_list();

    if let Some(gpu_metrics) = metrics::collect_gpu_metrics(&handle, &system, &users) {
        println!("Driver: {}", gpu_metrics.driver_version);
        println!("CUDA:   {}", gpu_metrics.api_version);
        println!("\nGPUs found: {}", gpu_metrics.gpus.len());

        for gpu in &gpu_metrics.gpus {
            println!("\n  [{}] {}", gpu.index, gpu.name);
            println!("      Temp: {}°C, Fan: {}%", gpu.temperature, gpu.fan_speed);
            println!("      Power: {}W / {}W", gpu.power_usage, gpu.power_limit);
            println!("      GPU: {}%, Mem: {}%", gpu.gpu_utilization, gpu.memory_utilization);
            println!("      VRAM: {} / {} MB",
                gpu.memory_used / 1024 / 1024,
                gpu.memory_total / 1024 / 1024);
        }

        if !gpu_metrics.processes.is_empty() {
            println!("\nGPU Processes: {}", gpu_metrics.processes.len());
            for proc in gpu_metrics.processes.iter().take(5) {
                println!("  PID {} (GPU {}): {} - {} MB",
                    proc.pid, proc.gpu_index, proc.name,
                    proc.gpu_memory / 1024 / 1024);
            }
        }
    } else {
        println!("Failed to collect GPU metrics");
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Debug GPU detection
    #[cfg(feature = "gpu")]
    if cli.debug_gpu {
        debug_gpu();
        return Ok(());
    }

    // Setup terminal
    enable_raw_mode().context("Failed to enable raw mode")?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)
        .context("Failed to enter alternate screen")?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).context("Failed to create terminal")?;

    // Create app
    let mut app = App::new(&cli).context("Failed to initialize application")?;

    // Main loop
    let result = run_app(&mut terminal, &mut app);

    // Restore terminal
    disable_raw_mode().context("Failed to disable raw mode")?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )
    .context("Failed to leave alternate screen")?;
    terminal.show_cursor().context("Failed to show cursor")?;

    result
}

fn run_app(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, app: &mut App) -> Result<()> {
    let mut last_tick = Instant::now();

    while app.running {
        terminal.draw(|f| render_ui(f, app))?;

        let timeout = app
            .refresh_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or(Duration::from_millis(0));

        if event::poll(timeout).context("Failed to poll events")? {
            match event::read().context("Failed to read event")? {
                Event::Key(key) => {
                    app.handle_key(key.code, key.modifiers);
                }
                Event::Mouse(mouse) => {
                    app.handle_mouse(mouse.kind, mouse.column, mouse.row);
                }
                _ => {}
            }
        }

        if last_tick.elapsed() >= app.refresh_rate {
            app.refresh_all()?;
            last_tick = Instant::now();
        }

        // Clear old status messages
        app.clear_old_status();
    }

    Ok(())
}
