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
const MANIFEST_JSON: &str = r##"{"name":"Glances","short_name":"Glances","description":"System monitor","start_url":"/","display":"standalone","background_color":"#0d1117","theme_color":"#0d1117","icons":[{"src":"/icon.svg","sizes":"any","type":"image/svg+xml"}]}"##;
const SERVICE_WORKER_JS: &str = r#"self.addEventListener('install',e=>self.skipWaiting());self.addEventListener('activate',e=>e.waitUntil(self.clients.claim()));self.addEventListener('fetch',e=>{if(e.request.url.includes('/api/')){e.respondWith(fetch(e.request).catch(()=>new Response('{"error":"offline"}',{headers:{'Content-Type':'application/json'}})))}});"#;
const ICON_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 128 128"><rect width="128" height="128" rx="24" fill="#0d1117"/><text x="64" y="78" text-anchor="middle" font-family="monospace" font-size="56" font-weight="bold" fill="#58a6ff">G</text><rect x="20" y="90" width="88" height="6" rx="3" fill="#21262d"/><rect x="20" y="90" width="55" height="6" rx="3" fill="#3fb950"/></svg>"##;

/// JSON response for /api/v1/all
#[derive(Serialize)]
struct AllMetrics {
    system: SystemMetrics,
    gpu: Option<GpuMetrics>,
    docker: Vec<ContainerInfo>,
    ports: Vec<PortProcessInfo>,
    history: ChartHistory,
}

#[derive(Serialize)]
struct ChartHistory {
    cpu: Vec<f64>,
    ram_pct: Vec<f64>,
    swap_pct: Vec<f64>,
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
    // Chart history (survives page refresh)
    cpu_history: Vec<f64>,
    ram_pct_history: Vec<f64>,
    swap_pct_history: Vec<f64>,
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

        // Update chart history
        self.cpu_history.remove(0);
        self.cpu_history.push(self.system_metrics.cpu_global as f64);
        let ram_pct = if self.system_metrics.memory.total > 0 {
            self.system_metrics.memory.used as f64 / self.system_metrics.memory.total as f64 * 100.0
        } else { 0.0 };
        self.ram_pct_history.remove(0);
        self.ram_pct_history.push(ram_pct);
        let swap_pct = if self.system_metrics.memory.swap_total > 0 {
            self.system_metrics.memory.swap_used as f64 / self.system_metrics.memory.swap_total as f64 * 100.0
        } else { 0.0 };
        self.swap_pct_history.remove(0);
        self.swap_pct_history.push(swap_pct);

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
        cpu_history: vec![0.0; 60],
        ram_pct_history: vec![0.0; 60],
        swap_pct_history: vec![0.0; 60],
        refresh_count: u64::MAX,  // ensures first refresh triggers all collections
        #[cfg(feature = "docker")]
        docker_handle: DockerHandle::new(),
        #[cfg(feature = "docker")]
        last_docker_cpu: HashMap::new(),
    };

    // Initial refresh (uses DockerHandle's internal runtime)
    web_state.refresh();
    let state = Arc::new(Mutex::new(web_state));

    let https_port = port + 1;
    let addr_http = format!("{}:{}", bind, port);
    let addr_https = format!("{}:{}", bind, https_port);

    // Generate self-signed TLS certificate
    let cert = rcgen::generate_simple_self_signed(vec![
        "localhost".to_string(),
        bind.to_string(),
        "127.0.0.1".to_string(),
    ])
    .expect("Failed to generate self-signed certificate");

    let cert_pem = cert.cert.pem();
    let key_pem = cert.key_pair.serialize_pem();

    let tls_config = {
        let cert_chain = vec![rustls::pki_types::CertificateDer::from(
            cert.cert.der().to_vec(),
        )];
        let key_der = rustls::pki_types::PrivateKeyDer::try_from(
            cert.key_pair.serialize_der(),
        )
        .expect("Failed to parse private key");

        let mut config = rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(cert_chain, key_der)
            .expect("Failed to build TLS config");
        config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];
        config
    };

    println!("Glances web server running:");
    println!("  HTTP:  http://{}/", addr_http);
    println!("  HTTPS: https://{}/ (self-signed)", addr_https);
    println!("  API:   http://{}/api/v1/all", addr_http);

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
                .route("/manifest.json", get(serve_manifest))
                .route("/sw.js", get(serve_sw))
                .route("/icon.svg", get(serve_icon))
                .route("/api/v1/all", get(api_all))
                .route("/api/v1/system", get(api_system))
                .route("/api/v1/gpu", get(api_gpu))
                .route("/api/v1/docker", get(api_docker))
                .route("/api/v1/ports", get(api_ports))
                .route("/api/v1/kill", post(api_kill))
                .layer(CorsLayer::permissive())
                .with_state(state);

            // HTTP server
            let http_app = app.clone();
            tokio::spawn(async move {
                let listener = tokio::net::TcpListener::bind(&addr_http).await.unwrap();
                axum::serve(listener, http_app).await.ok();
            });

            // HTTPS server with self-signed cert
            let rustls_config = axum_server::tls_rustls::RustlsConfig::from_pem(
                cert_pem.into_bytes(),
                key_pem.into_bytes(),
            )
            .await?;

            axum_server::bind_rustls(addr_https.parse().unwrap(), rustls_config)
                .serve(app.into_make_service())
                .await?;

            Ok::<_, anyhow::Error>(())
        })?;

    Ok(())
}

async fn serve_manifest() -> impl IntoResponse {
    (StatusCode::OK, [(header::CONTENT_TYPE, "application/manifest+json")], MANIFEST_JSON)
}

async fn serve_sw() -> impl IntoResponse {
    (StatusCode::OK, [(header::CONTENT_TYPE, "application/javascript")], SERVICE_WORKER_JS)
}

async fn serve_icon() -> impl IntoResponse {
    (StatusCode::OK, [(header::CONTENT_TYPE, "image/svg+xml")], ICON_SVG)
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
        history: ChartHistory {
            cpu: s.cpu_history.clone(),
            ram_pct: s.ram_pct_history.clone(),
            swap_pct: s.swap_pct_history.clone(),
        },
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
