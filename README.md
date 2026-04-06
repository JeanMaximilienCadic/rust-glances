# glances

A modern, feature-rich system monitor written in Rust. Provides both a TUI (terminal) and a web frontend. Inspired by [glances](https://github.com/nicolargo/glances) with GPU support, Docker integration, port monitoring, and a built-in API tester.

[![Crates.io](https://img.shields.io/crates/v/glances.svg)](https://crates.io/crates/glances)
![License](https://img.shields.io/badge/license-MIT-blue.svg)
![Rust](https://img.shields.io/badge/rust-1.70%2B-orange.svg)

## Install

```bash
# Full installation (GPU + Docker + Web)
cargo install glances --features full

# Default installation (GPU + Docker, no web)
cargo install glances

# Minimal installation (no GPU/Docker/Web dependencies)
cargo install glances --no-default-features --features minimal
```

### Feature Flags

| Feature | Description | Dependencies |
|---------|-------------|--------------|
| `default` | GPU + Docker monitoring | nvml-wrapper, bollard, reqwest |
| `full` | GPU + Docker + Web frontend | all of the above + axum, rustls |
| `gpu` | NVIDIA (Linux/Windows) and Metal (macOS) GPU monitoring | nvml-wrapper (Linux/Win), metal (macOS) |
| `docker` | Docker container monitoring and API testing | bollard, reqwest |
| `web` | Web frontend with HTTPS support | axum, tower-http, axum-server, rcgen, rustls |
| `minimal` | CPU, memory, disk, network only | None (lightweight) |

### System Dependencies

**Linux (GPU monitoring)**:
```bash
# NVIDIA drivers required for NVML
ls /usr/lib/x86_64-linux-gnu/libnvidia-ml.so.1
```

**Linux (building from source)**:
```bash
# Ubuntu/Debian
sudo apt install pkg-config libssl-dev

# Fedora
sudo dnf install pkg-config openssl-devel
```

**macOS**: Metal GPU support works out of the box on Apple Silicon and AMD GPUs.

## Features

### TUI Mode (default)

A full terminal UI with real-time monitoring, graphs, and interactive process management.

#### System Monitoring
- **CPU** -- global usage, per-core bars, user/sys/idle breakdown, load averages (1m/5m/15m)
- **Memory** -- RAM and swap with total/used/free, gradient bars
- **Disk** -- df-h style display: filesystem, size, used, avail, use%, mount points (grouped by device)
- **Network** -- all interfaces with rx/tx rates, throughput graphs
- **Battery** -- charge level and state in header
- **Sensors** -- temperature readings with color-coded thresholds
- **Power** -- Intel RAPL power draw per domain (package, core, uncore, dram)
- **Processes** -- sortable by CPU, memory, disk I/O, with kill support and filtering

#### GPU Monitoring

**NVIDIA GPUs (Linux/Windows via NVML)**:
- Multi-GPU utilization, temperature, fan speed, power, clocks, P-states
- VRAM usage, encoder/decoder utilization, PCIe throughput
- Per-process GPU memory tracking

**Apple Silicon GPUs (macOS via Metal)**:
- GPU utilization via IOKit, memory usage and allocation
- Metal API version detection
- GPU process listing via IOKit

#### Docker Integration
- Container list with CPU%, MEM/MAX, ports, uptime, compose labels
- Block I/O and network I/O per container
- **Container logs viewer** -- press `l` on Docker tab
- **Built-in HTTP API tester** -- press `Enter` on a container to open a Postman-like overlay (GET/POST/PUT/DELETE with headers, JSON body, pretty-printed responses)

#### Port Monitoring
- Lists all processes listening on TCP/TCP6 ports
- Shows port, protocol, PID, user, CPU%, memory, bind address, and full command
- Kill processes directly from the ports view

#### Alert System
- Auto-detects high CPU (>85%), memory (>75%), load (>1.0/core)
- Shows ongoing/resolved alerts with timestamps
- Displays top contributing processes for each alert

#### Graphs
- Braille sparkline charts for CPU, memory, network, and disk I/O
- Area-fill charts for CPU/memory and GPU utilization history
- 60-second rolling window

### Web Mode

A browser-based dashboard with split-pane multi-server monitoring. Built with Vue 3 and ECharts.

```bash
# Start web server (requires 'full' or 'web' feature)
glances -w

# Custom port and bind address
glances -w --port 8080 --bind 127.0.0.1

# With custom TLS certificate
glances -w --tls-cert /path/to/cert.pem --tls-key /path/to/key.pem
```

#### Web Features
- **Split-pane layout** -- iTerm2-style recursive splitting for monitoring multiple servers
- **Display modes** -- full, normal, and minimal views per pane
- **HTTPS** -- auto-generated self-signed certificate or custom TLS cert/key
- **PWA** -- installable as a Chrome/Progressive Web App
- **Real-time charts** -- CPU, RAM, and swap history with server-side persistence
- **Process management** -- sort, filter, and kill processes from the browser
- **REST API** -- JSON endpoints for integration with other tools

#### API Endpoints

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/api/v1/all` | GET | All metrics (system, GPU, Docker, ports, chart history) |
| `/api/v1/system` | GET | System metrics only |
| `/api/v1/gpu` | GET | GPU metrics only |
| `/api/v1/docker` | GET | Docker container list |
| `/api/v1/ports` | GET | Listening port processes |
| `/api/v1/kill` | POST | Kill a process (`{"pid": 1234, "signal": "TERM"}`) |

## Usage

```bash
# Run TUI with defaults (1s refresh)
glances

# Custom refresh rate (ms)
glances -r 500

# Disable GPU/Docker monitoring
glances --no-gpu --no-docker

# Start with per-core CPU bars and graphs
glances --per-core

# Start in compact mode
glances -c

# Show all processes including idle
glances -a

# Run as web server
glances -w

# Web server with custom TLS
glances -w --tls-cert cert.pem --tls-key key.pem

# Debug GPU detection
glances --debug-gpu
```

### CLI Options

| Flag | Description |
|------|-------------|
| `-r, --refresh <ms>` | Refresh rate in milliseconds (default: 1000) |
| `-c, --compact` | Start in compact mode |
| `-a, --all` | Show all processes including idle |
| `--per-core` | Show per-core CPU bars |
| `--no-gpu` | Disable GPU monitoring |
| `--no-docker` | Disable Docker monitoring |
| `--no-graphs` | Disable graphs |
| `-w, --web` | Run as web server instead of TUI |
| `--port <port>` | Web server port (default: 61208) |
| `--bind <addr>` | Web server bind address (default: 0.0.0.0) |
| `--tls-cert <path>` | Custom TLS certificate file (PEM) |
| `--tls-key <path>` | Custom TLS private key file (PEM) |

## Views

| Key | View |
|-----|------|
| `1` | Overview -- dashboard with all panels |
| `2` | Processes -- full-screen process table |
| `3` | Network -- all interfaces + throughput graph |
| `4` | Disks -- all filesystems (df -h style) |
| `5` | Docker -- containers with logs and API testing |
| `6` | GPU -- GPU cards, graphs, GPU processes |
| `7` | Ports -- listening TCP/TCP6 ports with processes |

## Keyboard Shortcuts

| Key | Action |
|-----|--------|
| `1`-`7` | Switch view |
| `?` / `F1` | Help |
| `q` / `Esc` | Quit (or close overlay) |
| `j`/`k` / `Up`/`Down` | Navigate |
| `PgUp` / `PgDn` | Page navigation |
| `Home` / `End` | Jump to first/last |
| `Tab` | Switch between CPU/GPU process panels |
| `F2`-`F8` | Sort by column (PID, Name, User, CPU, Mem, I/O, GPU) |
| `r` | Reverse sort order |
| `/` | Clear process filter |
| `a` | Toggle show all processes |
| `g` | Toggle graphs |
| `p` | Toggle per-core CPU bars |
| `c` | Toggle compact mode |
| `d` | Toggle Docker panel |
| `t` | Toggle temperature sensors |
| `+` / `-` | Adjust refresh rate (100ms steps) |
| `Del` | Kill selected process (SIGTERM) |
| `Ctrl+K` | Force kill (SIGKILL) |
| `Ctrl+T` | Send SIGTERM |
| `Ctrl+I` | Send SIGINT |
| `Ctrl+C` | Quit |

### Docker View

| Key | Action |
|-----|--------|
| `Enter` | Open HTTP API tester for selected container |
| `l` | View container logs (last 200 lines) |
| `Tab` / `Down` | Navigate fields in API tester |
| `m` | Cycle HTTP method (GET/POST/PUT/DELETE) |
| `Ctrl+S` | Send request |
| `f` | Pretty-print JSON body |

## Architecture

```
src/
├── main.rs              -- entry point, event loop, terminal setup
├── app.rs               -- app state, input handling, alerts, process kill
├── cli.rs               -- clap argument parser
├── types.rs             -- data structures (metrics, history, sort)
├── utils.rs             -- formatting helpers (colors, bars, duration)
├── web.rs               -- axum web server, REST API, HTTPS/TLS
├── frontend/
│   └── index.html       -- Vue 3 + ECharts web dashboard (single-file)
├── metrics/
│   ├── mod.rs           -- module exports, feature-gate stubs
│   ├── system.rs        -- CPU/mem/disk/net/battery/process collection
│   ├── gpu.rs           -- NVML (Linux/Win) + Metal (macOS) backends
│   ├── docker.rs        -- bollard Docker container stats
│   ├── ports.rs         -- /proc/net/tcp parser, port-to-process mapping
│   └── power.rs         -- Intel RAPL power monitoring
└── ui/
    ├── mod.rs           -- module exports
    ├── layout.rs        -- main layout coordinator, view routing
    ├── tabs.rs          -- tab bar renderer
    ├── header.rs        -- top bar (hostname, uptime, battery)
    ├── footer.rs        -- keybinding hints
    ├── system.rs        -- CPU/MEM/SWAP/LOAD bars, network compact, df-h disk
    ├── gpu.rs           -- GPU cards and details
    ├── graphs.rs        -- braille sparkline and area charts
    ├── processes.rs     -- CPU process table with scrollbar
    ├── ports.rs         -- port process table
    ├── docker.rs        -- Docker container table
    ├── temps.rs         -- temperature sensor panel
    ├── alerts.rs        -- alert events panel
    ├── http_dialog.rs   -- Postman-like API tester overlay
    ├── logs_dialog.rs   -- container logs viewer overlay
    └── dialogs.rs       -- help screen, kill confirm, status bar
```

## Performance

- **Differential refresh**: fast-changing metrics (CPU, memory, processes, network) update every cycle; slow metrics (disks, temps, Docker, ports) update every 5th cycle
- **Parallel collection**: GPU, power, ports, and Docker metrics are collected in parallel using scoped threads
- **Optimized release builds**: LTO enabled, single codegen unit, panic=abort

## Requirements

- Rust 1.70+
- **Linux/Windows**: NVIDIA drivers for GPU monitoring (optional)
- **macOS**: Metal-compatible GPU (Apple Silicon or AMD)
- Docker daemon running (optional, for container monitoring)

## License

MIT -- see [LICENSE](LICENSE) for details.

## Acknowledgments

- Originally forked from [nvglances](https://github.com/EricLBuehler/nvglances)
- Inspired by [glances](https://github.com/nicolargo/glances)
- Built with [ratatui](https://github.com/ratatui-org/ratatui), [sysinfo](https://github.com/GuillaumeGomez/sysinfo), [bollard](https://github.com/fussybeaver/bollard), [axum](https://github.com/tokio-rs/axum)
