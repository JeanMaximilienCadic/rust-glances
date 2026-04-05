//! Web frontend server (axum-based).

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use axum::extract::State;
use axum::http::{header, StatusCode};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use sysinfo::{Components, Disks, Networks, Pid, Signal, System, Users};
use tower_http::cors::CorsLayer;

use crate::metrics::ports::{collect_port_processes, PortProcessInfo};
use crate::metrics::power::{collect_power_metrics, RaplState};
use crate::metrics::{collect_gpu_metrics, collect_system_metrics, GpuHandle};
use crate::types::{GpuMetrics, SystemMetrics};

#[cfg(feature = "docker")]
use crate::metrics::docker::{
    collect_docker_metrics, collect_docker_stats, ContainerInfo, DockerHandle,
};
#[cfg(not(feature = "docker"))]
use crate::app::ContainerInfo;

const FRONTEND_HTML: &str = include_str!("frontend/index.html");

/// JSON response for /api/v1/all
#[derive(Serialize)]
struct AllMetrics {
    system: SystemMetrics,
    gpu: Option<GpuMetrics>,
    docker: Vec<ContainerInfo>,
    ports: Vec<PortProcessInfo>,
}

/// Shared application state for the web server.
struct WebState {
    system: System,
    networks: Networks,
    disks: Disks,
    components: Components,
    users: Users,
    gpu_handle: GpuHandle,
    last_network_stats: HashMap<String, (u64, u64)>,
    last_disk_stats: HashMap<String, (u64, u64)>,
    rapl_state: RaplState,
    last_update: Instant,
    gpu_enabled: bool,
    docker_enabled: bool,
    // Cached metrics
    system_metrics: SystemMetrics,
    gpu_metrics: Option<GpuMetrics>,
    docker_containers: Vec<ContainerInfo>,
    port_processes: Vec<PortProcessInfo>,
    refresh_count: u64,
    #[cfg(feature = "docker")]
    docker_handle: DockerHandle,
    #[cfg(feature = "docker")]
    last_docker_cpu: HashMap<String, (u64, u64)>,
}

type SharedState = Arc<Mutex<WebState>>;

impl WebState {
    fn refresh(&mut self) {
        let elapsed = self.last_update.elapsed();
        self.last_update = Instant::now();
        self.refresh_count = self.refresh_count.wrapping_add(1);

        // Every cycle: CPU + memory + processes + networks
        self.system.refresh_cpu_usage();
        self.system
            .refresh_processes(sysinfo::ProcessesToUpdate::All, true);
        self.system.refresh_memory();
        self.networks.refresh();

        // Every 5 cycles: disks, components/temps
        if self.refresh_count % 5 == 0 {
            self.disks.refresh();
            self.components.refresh();
        }

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

        // Parallel collection of independent metrics
        let gpu_enabled = self.gpu_enabled;
        let do_slow = self.refresh_count % 5 == 0;

        std::thread::scope(|s| {
            // GPU (independent)
            let gpu_handle = s.spawn(|| {
                if gpu_enabled {
                    collect_gpu_metrics(&self.gpu_handle, &self.system, &self.users)
                } else {
                    None
                }
            });

            // Power (independent — only reads sysfs)
            let power_handle = s.spawn(|| {
                collect_power_metrics(&mut self.rapl_state, elapsed)
            });

            // Ports (independent — reads /proc, expensive)
            let port_handle = s.spawn(|| {
                if do_slow {
                    Some(collect_port_processes(&self.system, &self.users))
                } else {
                    None
                }
            });

            // Docker (independent — has its own runtime)
            #[cfg(feature = "docker")]
            let docker_handle = s.spawn(|| {
                if self.docker_enabled && do_slow {
                    let mut containers = collect_docker_metrics(&self.docker_handle);
                    collect_docker_stats(
                        &self.docker_handle,
                        &mut containers,
                        &mut self.last_docker_cpu,
                    );
                    Some(containers)
                } else {
                    None
                }
            });

            // Collect results
            self.gpu_metrics = gpu_handle.join().unwrap_or(None);
            self.system_metrics.power = power_handle.join().unwrap_or_default();
            if let Some(ports) = port_handle.join().unwrap_or(None) {
                self.port_processes = ports;
            }
            #[cfg(feature = "docker")]
            if let Some(containers) = docker_handle.join().unwrap_or(None) {
                self.docker_containers = containers;
            }
        });
    }
}

/// Start the web server. This must NOT be called from within a tokio runtime.
pub fn run_web_server(
    bind: &str,
    port: u16,
    refresh_ms: u64,
    gpu_enabled: bool,
    docker_enabled: bool,
) -> anyhow::Result<()> {
    // Build state outside of any async runtime (DockerHandle creates its own)
    let mut system = System::new_all();
    system.refresh_all();

    let mut web_state = WebState {
        system,
        networks: Networks::new_with_refreshed_list(),
        disks: Disks::new_with_refreshed_list(),
        components: Components::new_with_refreshed_list(),
        users: Users::new_with_refreshed_list(),
        gpu_handle: GpuHandle::new(),
        last_network_stats: HashMap::new(),
        last_disk_stats: HashMap::new(),
        rapl_state: RaplState::default(),
        last_update: Instant::now(),
        gpu_enabled,
        docker_enabled,
        system_metrics: SystemMetrics::default(),
        gpu_metrics: None,
        docker_containers: Vec::new(),
        port_processes: Vec::new(),
        refresh_count: u64::MAX,  // ensures first refresh triggers all collections
        #[cfg(feature = "docker")]
        docker_handle: DockerHandle::new(),
        #[cfg(feature = "docker")]
        last_docker_cpu: HashMap::new(),
    };

    // Initial refresh (uses DockerHandle's internal runtime)
    web_state.refresh();
    let state = Arc::new(Mutex::new(web_state));

    let addr = format!("{}:{}", bind, port);
    println!("Glances web server running at http://{}", addr);
    println!("  Frontend: http://{}/", addr);
    println!("  API:      http://{}/api/v1/all", addr);

    // Now build a tokio runtime for the HTTP server only
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?
        .block_on(async {
            // Background refresh task using spawn_blocking to avoid runtime nesting
            let bg_state = Arc::clone(&state);
            tokio::spawn(async move {
                let mut interval = tokio::time::interval(Duration::from_millis(refresh_ms));
                loop {
                    interval.tick().await;
                    let s = Arc::clone(&bg_state);
                    tokio::task::spawn_blocking(move || {
                        s.lock().unwrap().refresh();
                    })
                    .await
                    .ok();
                }
            });

            let app = Router::new()
                .route("/", get(serve_frontend))
                .route("/frontend/", get(serve_frontend))
                .route("/frontend/{*path}", get(serve_frontend))
                .route("/api/v1/all", get(api_all))
                .route("/api/v1/system", get(api_system))
                .route("/api/v1/gpu", get(api_gpu))
                .route("/api/v1/docker", get(api_docker))
                .route("/api/v1/ports", get(api_ports))
                .route("/api/v1/kill", post(api_kill))
                .layer(CorsLayer::permissive())
                .with_state(state);

            let listener = tokio::net::TcpListener::bind(&addr).await?;
            axum::serve(listener, app).await?;

            Ok::<_, anyhow::Error>(())
        })?;

    Ok(())
}

async fn serve_frontend() -> impl IntoResponse {
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
        FRONTEND_HTML,
    )
}

async fn api_all(State(state): State<SharedState>) -> impl IntoResponse {
    let s = state.lock().unwrap();
    Json(AllMetrics {
        system: s.system_metrics.clone(),
        gpu: s.gpu_metrics.clone(),
        docker: s.docker_containers.clone(),
        ports: s.port_processes.clone(),
    })
}

async fn api_system(State(state): State<SharedState>) -> impl IntoResponse {
    let s = state.lock().unwrap();
    Json(s.system_metrics.clone())
}

async fn api_gpu(State(state): State<SharedState>) -> impl IntoResponse {
    let s = state.lock().unwrap();
    Json(s.gpu_metrics.clone())
}

async fn api_docker(State(state): State<SharedState>) -> impl IntoResponse {
    let s = state.lock().unwrap();
    Json(s.docker_containers.clone())
}

async fn api_ports(State(state): State<SharedState>) -> impl IntoResponse {
    let s = state.lock().unwrap();
    Json(s.port_processes.clone())
}

#[derive(Deserialize)]
struct KillRequest {
    pid: u32,
    signal: String,
}

#[derive(Serialize)]
struct KillResponse {
    ok: bool,
    message: String,
}

async fn api_kill(
    State(state): State<SharedState>,
    Json(req): Json<KillRequest>,
) -> impl IntoResponse {
    let mut s = state.lock().unwrap();
    s.system.refresh_processes(sysinfo::ProcessesToUpdate::All, true);

    let signal = match req.signal.to_uppercase().as_str() {
        "KILL" | "SIGKILL" => Signal::Kill,
        "INT" | "SIGINT" => Signal::Interrupt,
        _ => Signal::Term,
    };
    let signal_name = match signal {
        Signal::Kill => "SIGKILL",
        Signal::Term => "SIGTERM",
        Signal::Interrupt => "SIGINT",
        _ => "signal",
    };

    let pid = Pid::from_u32(req.pid);
    if let Some(process) = s.system.process(pid) {
        if process.kill_with(signal).unwrap_or(false) {
            Json(KillResponse {
                ok: true,
                message: format!("Sent {} to PID {}", signal_name, req.pid),
            })
        } else {
            Json(KillResponse {
                ok: false,
                message: format!("Failed to send {} to PID {}", signal_name, req.pid),
            })
        }
    } else {
        Json(KillResponse {
            ok: false,
            message: format!("Process {} not found", req.pid),
        })
    }
}
