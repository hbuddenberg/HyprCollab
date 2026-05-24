# HyprCollab

**Agentic AI Chat Platform — 100% Rust (2024 Edition)**

A fullstack agentic chat platform combining the best of LibreChat (UX + Artifacts + MCP), AnythingLLM (RAG + workspaces), ClaudeCodeUI (ACP visualization + mobile), and Hermes Agent (evolutive memory + skills + learning) — with a terminal-first aesthetic.

![CI](https://github.com/hbuddenberg/HyprCollab/actions/workflows/ci.yml/badge.svg)
![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)
![Crates](https://img.shields.io/badge/crates-30-orange)

## Architecture

```
Backend (Axum)          Frontend (Dioxus 0.6 WASM)      Platforms
├── LLM Router          ├── Sidebar (3 panels)           ├── Tauri 2.0 (Desktop)
├── Agent Runtime       ├── Chat Messages                ├── PWA (Web)
├── RAG Pipeline        ├── Artifacts Panel              ├── TUI (ratatui)
├── Memory (SQLite)     ├── Status Bar                   └── Mobile (future)
├── MCP Client          └── Input Area
├── ACP Connector
├── Persona System
├── Skills Engine
├── Playwright Browser
└── Approval Engine
```

## Tech Stack

| Layer | Technology |
|---|---|
| Backend | Axum 0.8, rig-rs, SQLite, LanceDB |
| Frontend | Dioxus 0.6 (WASM), terminal-first CSS |
| Desktop | Tauri 2.0 |
| TUI | ratatui + kitty image protocol |
| LLM | OpenAI, Anthropic, Ollama, Native GGUF |
| Config | Lua (mlua) + YAML fallback |
| Extensions | WASM + Lua |

## Quick Start

```bash
# Build
cargo build --workspace

# Test
cargo test --workspace

# Lint
cargo clippy --workspace -- -D warnings
cargo fmt --check --all
```

## Project Structure

```
crates/
├── hyprcollab-core/              # Shared types, traits, errors
├── hyprcollab-server/            # Axum HTTP + SSE + WS
├── hyprcollab-agent/             # Agent runtime (rig-rs)
├── hyprcollab-memory/            # SQLite memory engine
├── hyprcollab-rag/               # RAG pipeline
├── hyprcollab-artifacts/         # Artifact types + render
├── hyprcollab-acp/               # ACP connector
├── hyprcollab-skills/            # Skills engine
├── hyprcollab-tools/             # Built-in tools
├── hyprcollab-mcp/               # MCP client
├── hyprcollab-commands/          # Slash commands
├── hyprcollab-browser/           # Playwright + scraping
├── hyprcollab-approval/          # Approval engine
├── hyprcollab-media-preview/     # Kitty/Sixel preview
├── hyprcollab-image/             # Image generation
├── hyprcollab-voice/             # STT + TTS
├── hyprcollab-config/            # Config resolver
├── hyprcollab-lua/               # Lua engine (mlua)
├── hyprcollab-ext/               # Extensions (WASM + Lua)
├── hyprcollab-marketplace/       # Package registry
├── hyprcollab-persona/           # Persona + Agent Roles
├── hyprcollab-ui/                # UI registry + themes
├── hyprcollab-provider-openai/   # OpenAI provider
├── hyprcollab-provider-anthropic/# Anthropic provider
├── hyprcollab-provider-ollama/   # Ollama provider
├── hyprcollab-provider-openrouter/# OpenRouter provider
└── hyprcollab-provider-native/   # Native GGUF inference
platforms/
├── hyprcollab-tauri/             # Desktop app
├── hyprcollab-tui/               # Terminal UI
└── hyprcollab-pwa/               # Web PWA
```

## License

MIT
