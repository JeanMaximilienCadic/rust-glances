//! Application state and core logic.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use crossterm::event::{KeyCode, KeyModifiers, MouseButton, MouseEventKind};
use ratatui::layout::Rect;
use ratatui::widgets::TableState;
use sysinfo::{Components, Disks, Networks, Pid, Signal, System, Users};

use crate::cli::Cli;
use crate::metrics::docker::{
    collect_docker_metrics, collect_docker_stats, ContainerInfo, DockerHandle,
};
use crate::metrics::{collect_gpu_metrics, collect_system_metrics, GpuHandle};
use crate::types::{
    ActivePanel, GpuBackend, GpuMetrics, GpuProcessInfo, HistoryData, KillConfirmation,
    ProcessInfo, SortColumn, SystemMetrics,
};

/// HTTP method for API testing.
#[derive(PartialEq, Clone, Copy)]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Delete,
}

impl HttpMethod {
    pub fn as_str(&self) -> &str {
        match self {
            HttpMethod::Get => "GET",
            HttpMethod::Post => "POST",
            HttpMethod::Put => "PUT",
            HttpMethod::Delete => "DELETE",
        }
    }
    pub fn next(&self) -> Self {
        match self {
            HttpMethod::Get => HttpMethod::Post,
            HttpMethod::Post => HttpMethod::Put,
            HttpMethod::Put => HttpMethod::Delete,
            HttpMethod::Delete => HttpMethod::Get,
        }
    }
}

/// HTTP request dialog state.
#[derive(Clone)]
pub struct HttpRequestState {
    pub visible: bool,
    pub container_name: String,
    pub container_port: u16,
    pub method: HttpMethod,
    pub path: String,
    pub body: String,
    pub headers: String,
    pub response: Option<String>,
    pub response_status: Option<u16>,
    pub active_field: usize, // 0=method, 1=path, 2=headers, 3=body
    pub editing: bool,
}

impl Default for HttpRequestState {
    fn default() -> Self {
        Self {
            visible: false,
            container_name: String::new(),
            container_port: 0,
            method: HttpMethod::Get,
            path: "/".into(),
            body: String::new(),
            headers: "Content-Type: application/json".into(),
            response: None,
            response_status: None,
            active_field: 1,
            editing: false,
        }
    }
}

/// Container logs viewer state.
#[derive(Clone)]
pub struct ContainerLogsState {
    pub container_name: String,
    pub container_id: String,
    pub lines: Vec<String>,
    pub scroll: usize,
}

/// Alert/warning event.
#[derive(Clone)]
pub struct AlertEvent {
    pub timestamp: String,
    pub message: String,
    pub level: AlertLevel,
    pub ongoing: bool,
    /// Top processes contributing to this alert (name, value).
    pub top_processes: Vec<(String, String)>,
}

#[derive(Clone, PartialEq)]
pub enum AlertLevel {
    Warning,
    Critical,
}

/// View tabs for the main display.
#[derive(PartialEq, Clone, Copy)]
pub enum ViewTab {
    Overview,
    Processes,
    Network,
    Disks,
    Docker,
    Gpu,
}

/// Main application state.
pub struct App {
    // System data sources
    pub system: System,
    pub networks: Networks,
    pub disks: Disks,
    pub components: Components,
    pub users: Users,
    pub gpu_handle: GpuHandle,

    // Docker
    pub docker_handle: DockerHandle,
    pub docker_containers: Vec<ContainerInfo>,
    pub docker_enabled: bool,
    pub last_docker_cpu: HashMap<String, (u64, u64)>,

    // Collected metrics
    pub system_metrics: SystemMetrics,
    pub gpu_metrics: Option<GpuMetrics>,
    pub gpu_enabled: bool,
    pub history: HistoryData,

    // State tracking
    pub last_network_stats: HashMap<String, (u64, u64)>,
    pub last_disk_stats: HashMap<String, (u64, u64)>,
    pub last_update: Instant,

    // UI state
    pub running: bool,
    pub show_help: bool,
    pub active_panel: ActivePanel,
    pub cpu_process_state: TableState,
    pub gpu_process_state: TableState,
    pub docker_state: TableState,
    pub cpu_sort: SortColumn,
    pub gpu_sort: SortColumn,
    pub sort_ascending: bool,
    pub process_filter: String,
    pub show_all_processes: bool,
    pub compact_mode: bool,
    pub show_graphs: bool,
    pub show_per_core: bool,
    pub show_docker: bool,
    pub show_temps: bool,
    pub active_tab: ViewTab,

    // Settings
    pub refresh_rate: Duration,

    // Kill confirmation dialog
    pub kill_confirm: Option<KillConfirmation>,
    // Status message (shown briefly after actions)
    pub status_message: Option<(String, Instant)>,
    // Track panel areas for mouse support
    pub cpu_process_area: Option<Rect>,
    pub gpu_process_area: Option<Rect>,
    // HTTP request dialog
    pub http_request: HttpRequestState,
    // Container logs viewer
    pub container_logs: Option<ContainerLogsState>,
    // Alert events
    pub alerts: Vec<AlertEvent>,
}

impl App {
    /// Create a new App instance.
    pub fn new(cli: &Cli) -> anyhow::Result<Self> {
        let mut system = System::new_all();
        system.refresh_all();

        let networks = Networks::new_with_refreshed_list();
        let disks = Disks::new_with_refreshed_list();
        let components = Components::new_with_refreshed_list();
        let users = Users::new_with_refreshed_list();

        let gpu_enabled = !cli.no_gpu;
        let gpu_handle = GpuHandle::new();

        let docker_enabled = !cli.no_docker;
        let docker_handle = DockerHandle::new();

        let mut app = Self {
            system,
            networks,
            disks,
            components,
            users,
            gpu_handle,
            docker_handle,
            docker_containers: Vec::new(),
            docker_enabled,
            last_docker_cpu: HashMap::new(),
            system_metrics: SystemMetrics::default(),
            gpu_metrics: None,
            gpu_enabled,
            history: HistoryData::new(),
            last_network_stats: HashMap::new(),
            last_disk_stats: HashMap::new(),
            last_update: Instant::now(),
            running: true,
            show_help: false,
            active_panel: ActivePanel::CpuProcesses,
            cpu_process_state: TableState::default(),
            gpu_process_state: TableState::default(),
            docker_state: TableState::default(),
            cpu_sort: SortColumn::Cpu,
            gpu_sort: SortColumn::GpuMemory,
            sort_ascending: false,
            process_filter: String::new(),
            show_all_processes: cli.all,
            compact_mode: cli.compact,
            show_graphs: !cli.no_graphs,
            show_per_core: cli.per_core,
            show_docker: docker_enabled,
            show_temps: true,
            active_tab: ViewTab::Overview,
            refresh_rate: Duration::from_millis(cli.refresh),
            kill_confirm: None,
            status_message: None,
            cpu_process_area: None,
            gpu_process_area: None,
            http_request: HttpRequestState::default(),
            container_logs: None,
            alerts: Vec::new(),
        };

        app.cpu_process_state.select(Some(0));
        app.gpu_process_state.select(Some(0));
        app.docker_state.select(Some(0));
        app.refresh_all()?;

        Ok(app)
    }

    /// Refresh all metrics.
    pub fn refresh_all(&mut self) -> anyhow::Result<()> {
        let elapsed = self.last_update.elapsed();
        self.last_update = Instant::now();

        self.system.refresh_all();
        self.networks.refresh();
        self.disks.refresh();
        self.components.refresh();

        self.system_metrics = collect_system_metrics(
            &self.system,
            &self.networks,
            &self.disks,
            &self.components,
            &self.users,
            &mut self.last_network_stats,
            &mut self.last_disk_stats,
            elapsed,
        );

        if self.gpu_enabled {
            self.gpu_metrics = collect_gpu_metrics(&self.gpu_handle, &self.system, &self.users);
        }

        if self.docker_enabled {
            self.docker_containers = collect_docker_metrics(&self.docker_handle);
            collect_docker_stats(
                &self.docker_handle,
                &mut self.docker_containers,
                &mut self.last_docker_cpu,
            );
        }

        self.update_history();
        self.check_alerts();

        Ok(())
    }

    /// Get top N processes by a metric.
    fn top_procs_by_cpu(&self, n: usize) -> Vec<(String, String)> {
        let mut procs = self.system_metrics.processes.clone();
        procs.sort_by(|a, b| b.cpu_usage.partial_cmp(&a.cpu_usage).unwrap_or(std::cmp::Ordering::Equal));
        procs.iter().take(n)
            .filter(|p| p.cpu_usage > 1.0)
            .map(|p| (p.name.clone(), format!("{:.1}%", p.cpu_usage)))
            .collect()
    }

    fn top_procs_by_mem(&self, n: usize) -> Vec<(String, String)> {
        let mut procs = self.system_metrics.processes.clone();
        procs.sort_by(|a, b| b.memory_bytes.cmp(&a.memory_bytes));
        procs.iter().take(n)
            .filter(|p| p.memory_usage > 1.0)
            .map(|p| (p.name.clone(), format!("{:.1}%", p.memory_usage)))
            .collect()
    }

    fn top_procs_by_io(&self, n: usize) -> Vec<(String, String)> {
        let mut procs = self.system_metrics.processes.clone();
        procs.sort_by(|a, b| {
            let a_io = a.disk_read_rate + a.disk_write_rate;
            let b_io = b.disk_read_rate + b.disk_write_rate;
            b_io.partial_cmp(&a_io).unwrap_or(std::cmp::Ordering::Equal)
        });
        procs.iter().take(n)
            .filter(|p| p.disk_read_rate + p.disk_write_rate > 0.0)
            .map(|p| {
                let io = p.disk_read_rate + p.disk_write_rate;
                let io_str = if io >= 1_048_576.0 { format!("{:.1}MB/s", io / 1_048_576.0) }
                    else if io >= 1024.0 { format!("{:.0}KB/s", io / 1024.0) }
                    else { format!("{:.0}B/s", io) };
                (p.name.clone(), io_str)
            })
            .collect()
    }

    /// Check for alert conditions and generate events with top contributing processes.
    fn check_alerts(&mut self) {
        let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

        // Memory alert
        let mem = &self.system_metrics.memory;
        let mem_pct = if mem.total > 0 { (mem.used as f64 / mem.total as f64) * 100.0 } else { 0.0 };

        let has_mem_alert = self.alerts.iter().any(|a| a.message.starts_with("MEM") && a.ongoing);
        if mem_pct >= 75.0 && !has_mem_alert {
            let top = self.top_procs_by_mem(3);
            self.alerts.push(AlertEvent {
                timestamp: now.clone(),
                message: format!("MEM ({:.1}%)", mem_pct),
                level: if mem_pct >= 90.0 { AlertLevel::Critical } else { AlertLevel::Warning },
                ongoing: true,
                top_processes: top,
            });
        } else if mem_pct < 70.0 {
            for alert in &mut self.alerts {
                if alert.message.starts_with("MEM") && alert.ongoing {
                    alert.ongoing = false;
                }
            }
        } else if mem_pct >= 75.0 {
            // Update top processes on ongoing alert
            let top = self.top_procs_by_mem(3);
            for alert in &mut self.alerts {
                if alert.message.starts_with("MEM") && alert.ongoing {
                    alert.top_processes = top.clone();
                    alert.message = format!("MEM ({:.1}%)", mem_pct);
                }
            }
        }

        // CPU alert
        let cpu_pct = self.system_metrics.cpu_global as f64;
        let has_cpu_alert = self.alerts.iter().any(|a| a.message.starts_with("CPU") && a.ongoing);
        if cpu_pct >= 85.0 && !has_cpu_alert {
            let top = self.top_procs_by_cpu(3);
            self.alerts.push(AlertEvent {
                timestamp: now.clone(),
                message: format!("CPU ({:.1}%)", cpu_pct),
                level: if cpu_pct >= 95.0 { AlertLevel::Critical } else { AlertLevel::Warning },
                ongoing: true,
                top_processes: top,
            });
        } else if cpu_pct < 80.0 {
            for alert in &mut self.alerts {
                if alert.message.starts_with("CPU") && alert.ongoing {
                    alert.ongoing = false;
                }
            }
        } else if cpu_pct >= 85.0 {
            let top = self.top_procs_by_cpu(3);
            for alert in &mut self.alerts {
                if alert.message.starts_with("CPU") && alert.ongoing {
                    alert.top_processes = top.clone();
                    alert.message = format!("CPU ({:.1}%)", cpu_pct);
                }
            }
        }

        // Load alert
        let cores = self.system_metrics.cpu_count;
        let load_ratio = if cores > 0 { self.system_metrics.load_avg.0 / cores as f64 } else { 0.0 };
        let has_load_alert = self.alerts.iter().any(|a| a.message.starts_with("LOAD") && a.ongoing);
        if load_ratio >= 1.0 && !has_load_alert {
            let top = self.top_procs_by_io(3);
            self.alerts.push(AlertEvent {
                timestamp: now,
                message: format!("LOAD ({:.2})", self.system_metrics.load_avg.0),
                level: if load_ratio >= 2.0 { AlertLevel::Critical } else { AlertLevel::Warning },
                ongoing: true,
                top_processes: top,
            });
        } else if load_ratio < 0.8 {
            for alert in &mut self.alerts {
                if alert.message.starts_with("LOAD") && alert.ongoing {
                    alert.ongoing = false;
                }
            }
        } else if load_ratio >= 1.0 {
            let top = self.top_procs_by_io(3);
            for alert in &mut self.alerts {
                if alert.message.starts_with("LOAD") && alert.ongoing {
                    alert.top_processes = top.clone();
                    alert.message = format!("LOAD ({:.2})", self.system_metrics.load_avg.0);
                }
            }
        }

        // Keep only last 50 alerts
        if self.alerts.len() > 50 {
            self.alerts.drain(0..self.alerts.len() - 50);
        }
    }

    /// Update history data for graphs.
    fn update_history(&mut self) {
        self.history.push_cpu(self.system_metrics.cpu_global as f64);

        // Per-core CPU history
        for (i, cpu) in self.system_metrics.cpus.iter().enumerate() {
            self.history.push_cpu_core(i, cpu.usage as f64);
        }

        let mem = &self.system_metrics.memory;
        let mem_pct = if mem.total > 0 {
            (mem.used as f64 / mem.total as f64) * 100.0
        } else {
            0.0
        };
        self.history.push_memory(mem_pct);

        if let Some(ref gpu_metrics) = self.gpu_metrics {
            for (i, gpu) in gpu_metrics.gpus.iter().enumerate() {
                self.history.push_gpu_util(i, gpu.gpu_utilization as f64);
                let mem_pct = if gpu.memory_total > 0 {
                    (gpu.memory_used as f64 / gpu.memory_total as f64) * 100.0
                } else {
                    0.0
                };
                self.history.push_gpu_mem(i, mem_pct);
            }
        }

        let total_rx: f64 = self.system_metrics.networks.iter().map(|n| n.rx_rate).sum();
        let total_tx: f64 = self.system_metrics.networks.iter().map(|n| n.tx_rate).sum();
        self.history
            .push_network(total_rx / 1024.0 / 1024.0, total_tx / 1024.0 / 1024.0);

        // Disk I/O rates (aggregate from first disk entry or total)
        if let Some(disk) = self.system_metrics.disks.first() {
            self.history
                .push_disk_io(disk.read_rate / 1024.0 / 1024.0, disk.write_rate / 1024.0 / 1024.0);
        }
    }

    /// Get sorted CPU processes based on current sort settings.
    pub fn get_sorted_cpu_processes(&self) -> Vec<ProcessInfo> {
        let mut procs = if self.show_all_processes {
            self.system_metrics.processes.clone()
        } else {
            self.system_metrics
                .processes
                .iter()
                .filter(|p| p.cpu_usage > 0.0 || p.memory_usage > 0.1)
                .cloned()
                .collect()
        };

        if !self.process_filter.is_empty() {
            let filter = self.process_filter.to_lowercase();
            procs.retain(|p| {
                p.name.to_lowercase().contains(&filter)
                    || p.user.to_lowercase().contains(&filter)
                    || p.command.to_lowercase().contains(&filter)
            });
        }

        procs.sort_by(|a, b| {
            let cmp = match self.cpu_sort {
                SortColumn::Pid => a.pid.cmp(&b.pid),
                SortColumn::Name => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
                SortColumn::User => a.user.to_lowercase().cmp(&b.user.to_lowercase()),
                SortColumn::Cpu => a
                    .cpu_usage
                    .partial_cmp(&b.cpu_usage)
                    .unwrap_or(std::cmp::Ordering::Equal),
                SortColumn::Memory | SortColumn::GpuMemory => a
                    .memory_usage
                    .partial_cmp(&b.memory_usage)
                    .unwrap_or(std::cmp::Ordering::Equal),
                SortColumn::DiskIo => {
                    let a_io = a.disk_read_rate + a.disk_write_rate;
                    let b_io = b.disk_read_rate + b.disk_write_rate;
                    a_io.partial_cmp(&b_io).unwrap_or(std::cmp::Ordering::Equal)
                }
            };
            if self.sort_ascending {
                cmp
            } else {
                cmp.reverse()
            }
        });

        procs
    }

    /// Get sorted GPU processes based on current sort settings.
    pub fn get_sorted_gpu_processes(&self) -> Vec<GpuProcessInfo> {
        let Some(ref gpu_metrics) = self.gpu_metrics else {
            return Vec::new();
        };

        let mut procs = gpu_metrics.processes.clone();

        if !self.process_filter.is_empty() {
            let filter = self.process_filter.to_lowercase();
            procs.retain(|p| {
                p.name.to_lowercase().contains(&filter)
                    || p.user.to_lowercase().contains(&filter)
                    || p.command.to_lowercase().contains(&filter)
            });
        }

        procs.sort_by(|a, b| {
            let cmp = match self.gpu_sort {
                SortColumn::Pid => a.pid.cmp(&b.pid),
                SortColumn::Name => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
                SortColumn::User => a.user.to_lowercase().cmp(&b.user.to_lowercase()),
                SortColumn::GpuMemory | SortColumn::Memory | SortColumn::DiskIo => a.gpu_memory.cmp(&b.gpu_memory),
                SortColumn::Cpu => a.sm_utilization.cmp(&b.sm_utilization),
            };
            if self.sort_ascending {
                cmp
            } else {
                cmp.reverse()
            }
        });

        procs
    }

    /// Handle keyboard input.
    pub fn handle_key(&mut self, code: KeyCode, modifiers: KeyModifiers) {
        // Handle kill confirmation dialog
        if let Some(ref confirm) = self.kill_confirm.clone() {
            match code {
                KeyCode::Char('y') | KeyCode::Char('Y') => {
                    self.execute_kill(confirm.pid, confirm.signal);
                    self.kill_confirm = None;
                }
                KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                    self.kill_confirm = None;
                    self.set_status("Kill cancelled".to_string());
                }
                _ => {}
            }
            return;
        }

        if self.show_help {
            self.show_help = false;
            return;
        }

        // Handle HTTP request dialog
        if self.http_request.visible {
            self.handle_http_key(code);
            return;
        }

        // Handle logs viewer
        if self.container_logs.is_some() {
            match code {
                KeyCode::Esc | KeyCode::Char('q') => self.container_logs = None,
                KeyCode::Down | KeyCode::Char('j') => {
                    if let Some(ref mut logs) = self.container_logs {
                        logs.scroll = logs.scroll.saturating_add(1).min(logs.lines.len().saturating_sub(1));
                    }
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    if let Some(ref mut logs) = self.container_logs {
                        logs.scroll = logs.scroll.saturating_sub(1);
                    }
                }
                KeyCode::PageDown => {
                    if let Some(ref mut logs) = self.container_logs {
                        logs.scroll = logs.scroll.saturating_add(20).min(logs.lines.len().saturating_sub(1));
                    }
                }
                KeyCode::PageUp => {
                    if let Some(ref mut logs) = self.container_logs {
                        logs.scroll = logs.scroll.saturating_sub(20);
                    }
                }
                KeyCode::Home => {
                    if let Some(ref mut logs) = self.container_logs { logs.scroll = 0; }
                }
                KeyCode::End => {
                    if let Some(ref mut logs) = self.container_logs {
                        logs.scroll = logs.lines.len().saturating_sub(1);
                    }
                }
                _ => {}
            }
            return;
        }

        // Check for ctrl-modified keys first
        if modifiers.contains(KeyModifiers::CONTROL) {
            match code {
                KeyCode::Char('c') => {
                    self.running = false;
                    return;
                }
                KeyCode::Char('k') => {
                    self.request_kill(Signal::Kill);
                    return;
                }
                KeyCode::Char('t') => {
                    self.request_kill(Signal::Term);
                    return;
                }
                KeyCode::Char('i') => {
                    self.request_kill(Signal::Interrupt);
                    return;
                }
                _ => {}
            }
        }

        match code {
            KeyCode::Char('q') | KeyCode::Esc => self.running = false,
            KeyCode::Char('?') | KeyCode::F(1) => self.show_help = true,
            KeyCode::Char('1') => self.active_tab = ViewTab::Overview,
            KeyCode::Char('2') => self.active_tab = ViewTab::Processes,
            KeyCode::Char('3') => self.active_tab = ViewTab::Network,
            KeyCode::Char('4') => self.active_tab = ViewTab::Disks,
            KeyCode::Char('5') => self.active_tab = ViewTab::Docker,
            KeyCode::Char('6') => self.active_tab = ViewTab::Gpu,
            KeyCode::Tab => {
                // On Metal, skip GPU processes panel since it's not available
                let is_metal = self
                    .gpu_metrics
                    .as_ref()
                    .map(|m| m.backend == GpuBackend::Metal)
                    .unwrap_or(false);

                if !is_metal {
                    self.active_panel = match self.active_panel {
                        ActivePanel::CpuProcesses => ActivePanel::GpuProcesses,
                        ActivePanel::GpuProcesses => ActivePanel::CpuProcesses,
                    };
                }
                // On Metal, Tab does nothing (stays on CPU processes)
            }
            KeyCode::Char('a') => self.show_all_processes = !self.show_all_processes,
            KeyCode::Char('g') => self.show_graphs = !self.show_graphs,
            KeyCode::Char('c') => self.compact_mode = !self.compact_mode,
            KeyCode::Char('d') => self.show_docker = !self.show_docker,
            KeyCode::Char('l') => {
                // View logs for selected docker container
                if self.active_tab == ViewTab::Docker && !self.docker_containers.is_empty() {
                    let idx = self.docker_state.selected().unwrap_or(0);
                    if let Some(container) = self.docker_containers.get(idx) {
                        self.fetch_container_logs(&container.id.clone(), &container.name.clone());
                    }
                }
            }
            KeyCode::Char('t') => self.show_temps = !self.show_temps,
            KeyCode::Char('p') => self.show_per_core = !self.show_per_core,
            // Sort keys: F2-F8 for sort columns in process view
            KeyCode::F(2) => self.set_sort(SortColumn::Pid),
            KeyCode::F(3) => self.set_sort(SortColumn::Name),
            KeyCode::F(4) => self.set_sort(SortColumn::User),
            KeyCode::F(5) => self.set_sort(SortColumn::Cpu),
            KeyCode::F(6) => self.set_sort(SortColumn::Memory),
            KeyCode::F(7) => self.set_sort(SortColumn::DiskIo),
            KeyCode::F(8) => self.set_sort(SortColumn::GpuMemory),
            KeyCode::Char('r') => self.sort_ascending = !self.sort_ascending,
            KeyCode::Char('/') => {
                self.process_filter.clear();
            }
            KeyCode::Down | KeyCode::Char('j') => self.move_selection(1),
            KeyCode::Up | KeyCode::Char('k') => self.move_selection(-1),
            KeyCode::PageDown => self.move_selection(10),
            KeyCode::PageUp => self.move_selection(-10),
            KeyCode::Home => self.move_selection_to(0),
            KeyCode::End => self.move_selection_to(usize::MAX),
            KeyCode::Char('+') | KeyCode::Char('=') => {
                let new_rate = self.refresh_rate.as_millis().saturating_sub(100).max(100);
                self.refresh_rate = Duration::from_millis(new_rate as u64);
            }
            KeyCode::Char('-') => {
                let new_rate = self.refresh_rate.as_millis().saturating_add(100).min(5000);
                self.refresh_rate = Duration::from_millis(new_rate as u64);
            }
            KeyCode::Delete => {
                self.request_kill(Signal::Term);
            }
            KeyCode::Enter => {
                // On Docker tab, open HTTP request dialog for selected container
                if self.active_tab == ViewTab::Docker && !self.docker_containers.is_empty() {
                    let idx = self.docker_state.selected().unwrap_or(0);
                    if let Some(container) = self.docker_containers.get(idx) {
                        // Extract first port mapping
                        let port = container.ports.split("->").next()
                            .and_then(|s| s.trim().parse::<u16>().ok())
                            .unwrap_or(8080);
                        self.http_request = HttpRequestState {
                            visible: true,
                            container_name: container.name.clone(),
                            container_port: port,
                            method: HttpMethod::Get,
                            path: "/".into(),
                            body: String::new(),
                            headers: "Content-Type: application/json".into(),
                            response: None,
                            response_status: None,
                            active_field: 1,
                            editing: false,
                        };
                    }
                }
            }
            _ => {}
        }
    }

    /// Handle keys in the HTTP request dialog.
    fn handle_http_key(&mut self, code: KeyCode) {
        if self.http_request.editing {
            // Text input mode
            match code {
                KeyCode::Esc => self.http_request.editing = false,
                KeyCode::Backspace => {
                    let field = self.get_http_field_mut();
                    field.pop();
                }
                KeyCode::Char(c) => {
                    let field = self.get_http_field_mut();
                    field.push(c);
                }
                KeyCode::Enter => {
                    self.http_request.editing = false;
                }
                _ => {}
            }
            return;
        }

        match code {
            KeyCode::Esc | KeyCode::Char('q') => {
                self.http_request.visible = false;
            }
            KeyCode::Tab | KeyCode::Down => {
                self.http_request.active_field = (self.http_request.active_field + 1) % 4;
            }
            KeyCode::Up | KeyCode::BackTab => {
                self.http_request.active_field =
                    (self.http_request.active_field + 3) % 4;
            }
            KeyCode::Char('m') => {
                // Cycle HTTP method
                self.http_request.method = self.http_request.method.next();
            }
            KeyCode::Enter => {
                if self.http_request.active_field == 0 {
                    // Toggle method
                    self.http_request.method = self.http_request.method.next();
                } else {
                    // Start editing the active field
                    self.http_request.editing = true;
                }
            }
            KeyCode::Char('s') | KeyCode::F(9) => {
                // Send the request
                self.send_http_request();
            }
            _ => {}
        }
    }

    fn get_http_field_mut(&mut self) -> &mut String {
        match self.http_request.active_field {
            1 => &mut self.http_request.path,
            2 => &mut self.http_request.headers,
            3 => &mut self.http_request.body,
            _ => &mut self.http_request.path,
        }
    }

    fn fetch_container_logs(&mut self, id: &str, name: &str) {
        // Use docker CLI to get logs (simpler than bollard streaming)
        let output = std::process::Command::new("docker")
            .args(["logs", "--tail", "200", id])
            .output();

        let lines = match output {
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout);
                let stderr = String::from_utf8_lossy(&out.stderr);
                let mut all: Vec<String> = stdout.lines().map(|l| l.to_string()).collect();
                all.extend(stderr.lines().map(|l| l.to_string()));
                all
            }
            Err(e) => vec![format!("Error fetching logs: {}", e)],
        };

        let scroll = lines.len().saturating_sub(1);
        self.container_logs = Some(ContainerLogsState {
            container_name: name.to_string(),
            container_id: id.to_string(),
            lines,
            scroll,
        });
    }

    fn send_http_request(&mut self) {
        let url = format!(
            "http://localhost:{}{}",
            self.http_request.container_port, self.http_request.path
        );

        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build();

        let Ok(client) = client else {
            self.http_request.response = Some("Error: failed to create HTTP client".into());
            return;
        };

        // Parse headers
        let mut header_map = reqwest::header::HeaderMap::new();
        for line in self.http_request.headers.lines() {
            if let Some((k, v)) = line.split_once(':') {
                if let (Ok(name), Ok(val)) = (
                    reqwest::header::HeaderName::from_bytes(k.trim().as_bytes()),
                    reqwest::header::HeaderValue::from_str(v.trim()),
                ) {
                    header_map.insert(name, val);
                }
            }
        }

        let result = match self.http_request.method {
            HttpMethod::Get => client.get(&url).headers(header_map).send(),
            HttpMethod::Post => client.post(&url).headers(header_map).body(self.http_request.body.clone()).send(),
            HttpMethod::Put => client.put(&url).headers(header_map).body(self.http_request.body.clone()).send(),
            HttpMethod::Delete => client.delete(&url).headers(header_map).send(),
        };

        match result {
            Ok(resp) => {
                self.http_request.response_status = Some(resp.status().as_u16());
                let body = resp.text().unwrap_or_else(|_| "<binary>".into());
                // Truncate long responses
                if body.len() > 2000 {
                    self.http_request.response = Some(format!("{}...\n[truncated]", &body[..2000]));
                } else {
                    self.http_request.response = Some(body);
                }
            }
            Err(e) => {
                self.http_request.response_status = None;
                self.http_request.response = Some(format!("Error: {}", e));
            }
        }
    }

    /// Request to kill a process (shows confirmation dialog).
    fn request_kill(&mut self, signal: Signal) {
        let (pid, name) = match self.active_panel {
            ActivePanel::CpuProcesses => {
                let procs = self.get_sorted_cpu_processes();
                let idx = self.cpu_process_state.selected().unwrap_or(0);
                if let Some(proc) = procs.get(idx) {
                    (proc.pid, proc.name.clone())
                } else {
                    return;
                }
            }
            ActivePanel::GpuProcesses => {
                let procs = self.get_sorted_gpu_processes();
                let idx = self.gpu_process_state.selected().unwrap_or(0);
                if let Some(proc) = procs.get(idx) {
                    (proc.pid, proc.name.clone())
                } else {
                    return;
                }
            }
        };

        self.kill_confirm = Some(KillConfirmation { pid, name, signal });
    }

    /// Execute a kill signal on a process.
    fn execute_kill(&mut self, pid: u32, signal: Signal) {
        let sys_pid = Pid::from_u32(pid);
        if let Some(process) = self.system.process(sys_pid) {
            let signal_name = match signal {
                Signal::Kill => "SIGKILL",
                Signal::Term => "SIGTERM",
                Signal::Interrupt => "SIGINT",
                _ => "signal",
            };
            if process.kill_with(signal).unwrap_or(false) {
                self.set_status(format!("Sent {} to PID {}", signal_name, pid));
            } else {
                self.set_status(format!("Failed to send {} to PID {}", signal_name, pid));
            }
        } else {
            self.set_status(format!("Process {} not found", pid));
        }
    }

    /// Set a status message to display briefly.
    pub fn set_status(&mut self, msg: String) {
        self.status_message = Some((msg, Instant::now()));
    }

    /// Clear expired status message.
    pub fn clear_old_status(&mut self) {
        if let Some((_, time)) = &self.status_message {
            if time.elapsed() > Duration::from_secs(3) {
                self.status_message = None;
            }
        }
    }

    /// Handle mouse input.
    pub fn handle_mouse(&mut self, kind: MouseEventKind, column: u16, row: u16) {
        match kind {
            MouseEventKind::Down(MouseButton::Left) => {
                // Check if click is in CPU process area
                if let Some(area) = self.cpu_process_area {
                    if column >= area.x
                        && column < area.x + area.width
                        && row >= area.y
                        && row < area.y + area.height
                    {
                        self.active_panel = ActivePanel::CpuProcesses;
                        let relative_row = row.saturating_sub(area.y + 2);
                        let procs = self.get_sorted_cpu_processes();
                        if (relative_row as usize) < procs.len() {
                            self.cpu_process_state.select(Some(relative_row as usize));
                        }
                        return;
                    }
                }
                // Check if click is in GPU process area (skip on Metal)
                let is_metal = self
                    .gpu_metrics
                    .as_ref()
                    .map(|m| m.backend == GpuBackend::Metal)
                    .unwrap_or(false);

                if !is_metal {
                    if let Some(area) = self.gpu_process_area {
                        if column >= area.x
                            && column < area.x + area.width
                            && row >= area.y
                            && row < area.y + area.height
                        {
                            self.active_panel = ActivePanel::GpuProcesses;
                            let relative_row = row.saturating_sub(area.y + 2);
                            let procs = self.get_sorted_gpu_processes();
                            if (relative_row as usize) < procs.len() {
                                self.gpu_process_state.select(Some(relative_row as usize));
                            }
                        }
                    }
                }
            }
            MouseEventKind::ScrollDown => {
                self.move_selection(3);
            }
            MouseEventKind::ScrollUp => {
                self.move_selection(-3);
            }
            _ => {}
        }
    }

    /// Set the sort column for the active panel.
    fn set_sort(&mut self, column: SortColumn) {
        match self.active_panel {
            ActivePanel::CpuProcesses => {
                if self.cpu_sort == column {
                    self.sort_ascending = !self.sort_ascending;
                } else {
                    self.cpu_sort = column;
                    self.sort_ascending = false;
                }
            }
            ActivePanel::GpuProcesses => {
                if self.gpu_sort == column {
                    self.sort_ascending = !self.sort_ascending;
                } else {
                    self.gpu_sort = column;
                    self.sort_ascending = false;
                }
            }
        }
    }

    /// Move the selection by a delta.
    fn move_selection(&mut self, delta: i32) {
        // Docker tab: navigate docker containers
        if self.active_tab == ViewTab::Docker {
            let len = self.docker_containers.len();
            if len == 0 { return; }
            let current = self.docker_state.selected().unwrap_or(0);
            let new = if delta > 0 {
                (current + delta as usize).min(len - 1)
            } else {
                current.saturating_sub((-delta) as usize)
            };
            self.docker_state.select(Some(new));
            return;
        }

        let len = match self.active_panel {
            ActivePanel::CpuProcesses => self.get_sorted_cpu_processes().len(),
            ActivePanel::GpuProcesses => self.get_sorted_gpu_processes().len(),
        };

        if len == 0 {
            return;
        }

        let state = match self.active_panel {
            ActivePanel::CpuProcesses => &mut self.cpu_process_state,
            ActivePanel::GpuProcesses => &mut self.gpu_process_state,
        };

        let current = state.selected().unwrap_or(0);
        let new = if delta > 0 {
            (current + delta as usize).min(len - 1)
        } else {
            current.saturating_sub((-delta) as usize)
        };
        state.select(Some(new));
    }

    /// Move the selection to a specific position.
    fn move_selection_to(&mut self, pos: usize) {
        if self.active_tab == ViewTab::Docker {
            let len = self.docker_containers.len();
            if len == 0 { return; }
            self.docker_state.select(Some(pos.min(len - 1)));
            return;
        }

        let len = match self.active_panel {
            ActivePanel::CpuProcesses => self.get_sorted_cpu_processes().len(),
            ActivePanel::GpuProcesses => self.get_sorted_gpu_processes().len(),
        };

        if len == 0 {
            return;
        }

        let state = match self.active_panel {
            ActivePanel::CpuProcesses => &mut self.cpu_process_state,
            ActivePanel::GpuProcesses => &mut self.gpu_process_state,
        };

        state.select(Some(pos.min(len - 1)));
    }
}
