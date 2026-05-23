# HyprCollab

**Agentic AI chat platform for developers** — Multi-platform, Rust-native, terminal-first.

Combines the best of LibreChat (UX + Artifacts), AnythingLLM (RAG + Workspaces), ClaudeCodeUI (ACP), and Hermes Agent (memory + skills) into one cohesive Rust application.

## Platforms

- 🖥️ **Desktop** — Tauri 2.0 (Linux, macOS, Windows)
- 🌐 **Web Server** — Axum + Dioxus WASM
- ⌨️ **TUI** — ratatui + kitty image protocol
- 📱 **Mobile** — PWA + Tauri 2.0

## Features

- Multi-provider LLM (OpenAI, Anthropic, Ollama, OpenRouter)
- Agent runtime with tools (shell, web, files, browser)
- Artifacts canvas with live preview
- RAG pipeline with LanceDB
- Memory + auto-learning skills
- ACP visualization (Claude Code, Codex, AGY)
- Slash commands (`/.agent`, `/.skill`, `/.design`, `/.browse`)
- Approval system for dangerous operations
- Kitty image protocol preview (TUI)
- Native web browser engine

## Stack

- **Backend:** Rust + Axum + rig-rs
- **Frontend:** Dioxus 0.6 (WASM) + Tauri 2.0
- **DB:** SQLite + LanceDB
- **License:** MIT

## Quick Start

```bash
# Build
cargo build

# Run web server
cargo run -- serve

# Run TUI
cargo run -- tui

# Run tests
cargo test
```

## Documentation

- [PRD](docs/PRD.md) — Product Requirements
- [TRD](docs/TRD.md) — Technical Requirements
- [PLAN](docs/PLAN.md) — Implementation Plan

## License

MIT
