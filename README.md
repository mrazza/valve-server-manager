# Valve Server Manager (VSM) 🎮

A lightweight, cross-platform terminal user interface (TUI) utility written in Rust that allows players to block and unblock Valve's Steam Datagram Relay (SDR) server regions. 

By manipulating the operating system's firewall, VSM forces matchmaking systems in games like **Counter-Strike 2** and **Deadlock** to select the specific global regions you prefer, avoiding high latency or undesirable routing.

---

## ✨ Features

*   **Cross-Platform Integration**:
    *   **Linux**: Automatically creates and manages a dedicated `VSM_BLOCKS` chain in `iptables` to keep your primary rules clean.
    *   **Windows**: Spawns high-performance rules in the Windows Advanced Firewall using `netsh`.
*   **Dynamic Latency Checking**: Launches a multi-threaded pinger in the background to display live, color-coded round-trip latency to each region's relay nodes.
*   **Configurable Persistence**: Offers a settings overlay to toggle whether firewall blocks persist after exiting the TUI, or automatically clean up.
*   **Zero-Config Startup**: Fetches Valve's latest SDR configuration directly from the Steam API at startup, with an embedded local JSON fallback if offline.
*   **Clean & Safe**: Operates purely at the network firewall level. Does not modify any game files, memory, or system DLLs (VAC safe).
*   **Robust Test Suite**: Over 80% line coverage verified using mock executors and Ratatui's memory-based `TestBackend`.

---

## ⌨️ Controls

| Key | Action |
| :---: | :--- |
| **`Arrow Up / Down`** or **`k / j`** | Navigate through the PoP (Point of Presence) region list |
| **`Space / Enter`** | Toggle firewall block/allow for the selected region |
| **`R` / `r`** | Reset (Clear all VSM firewall blocks and unblock everything) |
| **`S` / `s`** | Toggle Settings overlay panel |
| **`Q` / `Esc` / `Ctrl+C`** | Exit VSM (cleans up rules if ephemeral mode is active) |

---

## 🚀 Getting Started

### Prerequisites
*   [Rust and Cargo](https://rustup.rs/) (edition 2021+)
*   Administrative privileges on your system (required to edit firewall rules)

### Installation & Compilation
Clone the repository and compile the release binary:
```bash
git clone https://github.com/mrazza/valve-server-manager.git
cd valve-server-manager
cargo build --release
```
The compiled binary will be located at `target/release/valve-server-manager`.

### Running VSM
To allow VSM to modify firewall rules, it must be run with elevated privileges.

#### Linux:
```bash
sudo ./target/release/valve-server-manager
```

#### Windows (Run as Administrator):
```powershell
.\target\release\valve-server-manager.exe
```

---

## 🛠️ Architecture

The codebase is structured into isolated, testable modules:
*   `src/main.rs`: Entrypoint, privilege validation, and alternate screen bootstrap.
*   `src/privileges.rs`: Cross-platform checks (Windows UAC admin token, Linux effective UID `0`).
*   `src/config.rs`: Handles loading and saving configuration states (`settings.toml`).
*   `src/sdr.rs`: Manages parsing and continental grouping of Steam relay PoPs.
*   `src/firewall/`: Abstrated firewall operations behind a `FirewallDriver` trait. Communicates via a mockable `CommandExecutor`.
*   `src/ping.rs`: A parallel worker thread batcher that periodically measures ICMP ping latencies.
*   `src/tui/`: Implements the layout rendering (`ui.rs`) and state machine event loop (`mod.rs`).

---

## 🧪 Testing

Running unit tests:
```bash
cargo test
```

Generating test coverage report (requires LLVM tools):
```bash
LLVM_PROFILE_FILE="vsm-%p-%m.profraw" RUSTFLAGS="-C instrument-coverage" cargo test
llvm-profdata merge -sparse vsm-*.profraw -o vsm.profdata
llvm-cov report -instr-profile=vsm.profdata target/debug/deps/valve_server_manager-<hash> src/
```
