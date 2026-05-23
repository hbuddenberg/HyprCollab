# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**hyprcollab** is a native Hyprland AI chat agent — a GTK4 desktop app with Waybar integration that supports multi-agent conversations (OpenAI-compatible APIs, Claude Code, OpenCode, Gemini CLI) with persistent RAG memory per folder. It runs as a background daemon and exposes a Unix socket IPC interface.

## Build & Run

```bash
# Build (debug)
cargo build

# Build (release)
cargo build --release

# Run tests
cargo test

# Run a single test
cargo test <test_name>

# Run tests in a specific file
cargo test --test state_tests
cargo test --test config_tests

# Check without building
cargo check

# Run linter
cargo clippy

# Start the daemon
./target/debug/hyprcollab daemon start

# Show/toggle GUI (requires daemon running)
./target/debug/hyprcollab gui show
./target/debug/hyprcollab gui toggle

# One-shot setup (config + waybar + keybinds)
./target/debug/hyprcollab setup
```

## Architecture

The binary is both the CLI entry point and the daemon process. When spawned with `HYPR_COLLAB_DAEMON=1`, it runs as the IPC server (`daemon/server.rs`). Otherwise it parses subcommands and communicates with the daemon via Unix socket.

```
main.rs (CLI / daemon entry)
│
├── daemon/          — Daemon lifecycle (start/stop/status/PID file)
│   └── server.rs    — Async Tokio IPC server (Unix socket, JSON-lines protocol)
│
├── state.rs         — In-memory app state + all IPC request handling (State::handle)
├── ipc.rs           — Request/Response enums + encode/decode helpers (JSON-lines)
│
├── gui/mod.rs       — GTK4 + gtk4-layer-shell overlay window; syncs from daemon via ipc_call()
├── storage/mod.rs   — Config (YAML), loaded from ~/.config/hypr-collab/config.yaml
├── waybar/          — Waybar module install/remove + status JSON output
│   └── keybinds.rs  — Hyprland keybind install/remove
└── utils/
    ├── paths.rs     — XDG-based paths (config, data, cache, runtime, logs)
    └── logging.rs   — tracing-subscriber setup
```

### IPC Protocol

JSON-lines over a Unix socket (`$XDG_RUNTIME_DIR/hypr-collab/hypr-collab.sock`). Each request/response is a single JSON object terminated by `\n`. The `Request` and `Response` enums use `serde` tagged unions (`#[serde(tag = "type")]`). The daemon holds a single `Arc<Mutex<State>>` shared across all client connections.

### State Model

`State` (`state.rs`) is the authoritative in-memory model. It holds folders → chats → messages, plus active indices, agent list, model list, and sidebar visibility. `State::handle(req)` is the single dispatch point for all mutations. `State::snapshot()` produces a `StateSnapshot` (the serializable IPC DTO) sent back to clients.

### GUI

The GUI (`gui/mod.rs`) is a stateless GTK4 window built fresh on each `gui show/toggle`. It fetches state via `ipc_call(&Request::GetState)` at startup and calls IPC for mutations (send message, select chat, etc.). The window uses `gtk4-layer-shell` to render as a Wayland overlay anchored to all four edges with margins to center it.

CSS is embedded as a `const` string in `gui/mod.rs`. The font stack assumes **JetBrainsMono Nerd Font** — Nerd Font codepoints are used for icons throughout.

### Config

`~/.config/hypr-collab/config.yaml` (XDG). Created automatically with defaults on first run. `Config::load()` creates it if missing. Sections: `general`, `agents` (OpenAI-compatible endpoints), `ui`, `waybar`, `rag`, `keys`.

### Paths (XDG)

| Purpose | Path |
|---------|------|
| Config file | `~/.config/hypr-collab/config.yaml` |
| Data dir | `~/.local/share/hypr-collab/` |
| Cache | `~/.cache/hypr-collab/` |
| Runtime/socket | `$XDG_RUNTIME_DIR/hypr-collab/hypr-collab.sock` |
| PID file | `$XDG_RUNTIME_DIR/hypr-collab/hypr-collab.pid` |
| Waybar status | `~/.cache/hypr-collab/waybar.json` |
| Logs | `~/.local/share/hypr-collab/logs/` |

## Current Status

The project is early-stage / POC. Key areas not yet implemented (stubs exist):
- `src/agents/mod.rs` — agent backends (OpenAI-compatible HTTP, ACP/stdio for Claude Code/OpenCode)
- `src/rag/mod.rs` — RAG/vector store per folder
- `src/commands/mod.rs` — slash commands (`/run`, `/read`, `/edit`, etc.)
- `State::handle(Request::SendMessage)` returns a hardcoded mock response instead of calling a real agent

The GUI currently builds a static view from daemon state but does not reactively update after sending messages (no UI rebuild after IPC response).

## Tests

Integration tests live in `tests/`. They test `State` directly and `Config` defaults/serialization — no daemon process needed.

```bash
cargo test --test state_tests   # IPC dispatch + state mutation tests
cargo test --test config_tests  # Config defaults, YAML/JSON roundtrip
```
