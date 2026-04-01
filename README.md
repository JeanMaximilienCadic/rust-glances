# glances

A modern, feature-rich TUI system monitor written in Rust. Inspired by [glances](https://github.com/nicolargo/glances) with GPU support, Docker integration, and a built-in API tester.

[![Crates.io](https://img.shields.io/crates/v/glances.svg)](https://crates.io/crates/glances)
![License](https://img.shields.io/badge/license-MIT-blue.svg)
![Rust](https://img.shields.io/badge/rust-1.70%2B-orange.svg)

## Install

```bash
cargo install glances
```

## Features

### System Monitoring
- **CPU** — global usage, per-core bars, user/sys/idle breakdown, load averages (1m/5m/15m)
- **Memory** — RAM, swap with total/used/free, gradient bars
- **Disk** — all filesystems, usage, I/O read/write rates
- **Network** — all interfaces (including lo, utun, awdl), rx/tx rates
- **Battery** — charge level and state in header
- **Sensors** — temperature readings with color-coded thresholds
- **Processes** — sortable by CPU, memory, disk I/O, with kill support

### GPU Monitoring

#### NVIDIA GPUs (Linux/Windows via NVML)
- Multi-GPU utilization, temperature, fan, power, clocks, P-states
- VRAM usage, encoder/decoder, PCIe throughput
- Per-process GPU memory tracking

#### Apple Silicon GPUs (macOS via Metal)
- GPU memory usage and allocation
- Metal API version detection

### Docker Integration
- Container list with CPU%, MEM/MAX, ports, uptime
- Block I/O and network I/O per container
- **Container logs viewer** — press `l` on Docker tab
- **Built-in HTTP API tester** — press `Enter` on a container to open a Postman-like overlay (GET/POST/PUT/DELETE with headers, JSON body, response viewer)

### Alert System
- Auto-detects high CPU (>85%), memory (>75%), load (>1.0/core)
- Shows ongoing/resolved alerts with timestamps

### Modern TUI
- Rounded borders, RGB color gradients, smooth braille sparkline graphs
- Tab-based views with number keys
- Mouse support (click to select, scroll to navigate)
- Vim-style keybindings (j/k, PgUp/PgDn)

## Usage

```bash
# Run with defaults
glances

# Custom refresh rate (ms)
glances -r 500

# Disable GPU/Docker monitoring
glances --no-gpu --no-docker

# Start with per-core CPU bars
glances --per-core

# Start in compact mode
glances -c

# Show all processes including idle
glances -a
```

## Views

| Key | View |
|-----|------|
| `1` | Overview — dashboard with all panels |
| `2` | Processes — full-screen process table |
| `3` | Network — all interfaces + throughput graph |
| `4` | Disks — all filesystems with I/O rates |
| `5` | Docker — containers with logs and API testing |
| `6` | GPU — GPU cards, graphs, GPU processes |

## Keyboard Shortcuts

| Key | Action |
|-----|--------|
| `1`-`6` | Switch view |
| `?` | Help |
| `q` / `Esc` | Quit (or close overlay) |
| `j`/`k` / `Up`/`Down` | Navigate |
| `PgUp` / `PgDn` | Page navigation |
| `F2`-`F8` | Sort by column |
| `r` | Reverse sort |
| `a` | Toggle show all processes |
| `g` | Toggle graphs |
| `p` | Toggle per-core CPU bars |
| `d` | Toggle Docker panel |
| `t` | Toggle temperature sensors |
| `+` / `-` | Adjust refresh rate |
| `Del` | Kill selected process (SIGTERM) |
| `Ctrl+K` | Force kill (SIGKILL) |

### Docker View

| Key | Action |
|-----|--------|
| `Enter` | Open HTTP API tester for selected container |
| `l` | View container logs |
| `Tab` | Navigate fields in API tester |
| `m` | Cycle HTTP method (GET/POST/PUT/DELETE) |
| `s` | Send request |

## Architecture

```
src/
├── main.rs              — event loop, terminal setup
├── app.rs               — app state, input handling, alerts
├── cli.rs               — clap argument parser
├── types.rs             — data structures
├── utils.rs             — formatting helpers
├── metrics/
│   ├── system.rs        — CPU/mem/disk/net/battery/process collection
│   ├── gpu.rs           — NVML + Metal GPU backends
│   └── docker.rs        — bollard Docker stats
└── ui/
    ├── layout.rs        — main layout coordinator
    ├── tabs.rs          — tab bar
    ├── header.rs        — top bar with hostname, uptime, battery
    ├── footer.rs        — keybinding hints
    ├── system.rs        — CPU/MEM/SWAP/LOAD inline bars
    ├── gpu.rs           — GPU cards
    ├── graphs.rs        — braille sparkline charts
    ├── processes.rs     — process tables
    ├── docker.rs        — container table
    ├── temps.rs         — sensor panel
    ├── alerts.rs        — alert events panel
    ├── http_dialog.rs   — Postman-like API tester overlay
    ├── logs_dialog.rs   — container logs viewer overlay
    └── dialogs.rs       — help, kill confirm, status
```

## Requirements

- Rust 1.70+
- **Linux/Windows**: NVIDIA drivers for GPU monitoring
- **macOS**: Metal-compatible GPU (Apple Silicon or AMD)
- Docker (optional, for container monitoring)

## License

MIT — see [LICENSE](LICENSE) for details.

## Acknowledgments

- Originally forked from [nvglances](https://github.com/EricLBuehler/nvglances)
- Inspired by [glances](https://github.com/nicolargo/glances)
- Built with [ratatui](https://github.com/ratatui-org/ratatui), [sysinfo](https://github.com/GuillaumeGomez/sysinfo), [bollard](https://github.com/fussybeaver/bollard)
