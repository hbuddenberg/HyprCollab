# HyprCollab — Fase 6: Desktop + Web Terminal-Like Architecture

> **Plan detallado** · Rust 2024 Edition · Semanas 26-29
> Filosofía: **Web + Desktop se sienten como una terminal real**

---

## Principio Fundamental

HyprCollab NO es un chat con tema oscuro. Es un **emulador de terminal** donde el flujo principal es texto, ANSI colors, y glifos Nerd Font. La UI de chat, la sidebar, los artifacts — todo respira terminal.

### Stack Frontend (las 3 capas)

```
┌──────────────────────────────────────────────────┐
│                 XTERM.JS v6.0                     │
│  Grilla de terminal real (chars + ANSI + colors)  │
│  + addon-webgl (GPU 60fps)                        │
│  + addon-fit (auto-resize)                        │
│  + addon-image (kitty protocol / Sixel)           │
├──────────────────────────────────────────────────┤
│              DIOXUS 0.6 (WASM)                    │
│  Sidebar, panels, input bar, modals               │
│  Montados como overlays sobre xterm               │
│  Estilo: JetBrainsMono NF, bg #0d1117            │
├──────────────────────────────────────────────────┤
│              TAURI 2.0 / AXUM WEB                 │
│  IPC: Commands (request-response)                 │
│  Events: streaming bidireccional (PTY output)     │
│  portable-pty: shell real (bash/zsh/fish)         │
└──────────────────────────────────────────────────┘
```

### Arquitectura del layout

```
┌─────────────┬──────────────────────────────────┐
│             │  ┌──────────────────────────────┐ │
│  SIDEBAR    │  │     XTERM.JS CANVAS          │ │
│  (Dioxus)   │  │  Chat messages como ANSI     │ │
│             │  │  Agent output con colores     │ │
│  💬 Chats   │  │  Tool calls como bloques      │ │
│  📁 Projs   │  │  Artifacts inline (img/code)  │ │
│  🎭 Personas│  │                              │ │
│             │  │  $ user input here_           │ │
│             │  └──────────────────────────────┘ │
│             │  ┌──────────────────────────────┐ │
│             │  │ STATUS BAR (model, tokens)   │ │
│             │  └──────────────────────────────┘ │
│             │  ┌──────────────────────────────┐ │
│             │  │ INPUT BAR (Dioxus overlay)   │ │
│             │  │ > type message...      [Send] │ │
│             │  └──────────────────────────────┘ │
└─────────────┴──────────────────────────────────┘
```

**Key insight:** El área de mensajes NO es un div con scroll — es un canvas xterm.js. Los mensajes se renderizan como líneas de terminal con ANSI styling. El input es un overlay Dioxus (textarea real) montado debajo del canvas.

---

## Semana 26: Tauri 2.0 Setup + PTY Bridge

### Crate: `hyprcollab-tauri`

**Objetivo:** App desktop nativa con ventana Tauri, PTY bridge funcional, xterm.js renderizando shell real.

#### 26.1 Scaffold Tauri 2.0
```
platforms/hyprcollab-tauri/
├── src-tauri/
│   ├── Cargo.toml          # tauri 2.0, portable-pty, tokio
│   ├── tauri.conf.json     # window config, permissions
│   ├── capabilities/
│   │   └── main.json       # IPC permissions
│   ├── src/
│   │   ├── main.rs         # Entry point
│   │   ├── lib.rs          # Builder setup
│   │   ├── pty.rs          # PTY management
│   │   ├── commands.rs     # Tauri IPC commands
│   │   └── state.rs        # AppState
│   └── icons/
├── src/                     # Frontend
│   ├── index.html
│   ├── main.ts
│   ├── terminal.ts          # xterm.js setup
│   ├── app.tsx              # Dioxus root
│   └── styles/
│       └── terminal.css     # Terminal aesthetic
├── package.json
└── vite.config.ts
```

#### 26.2 PTY Bridge (Rust → Frontend)
```rust
// src-tauri/src/pty.rs
use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use tauri::{AppHandle, Emitter};
use tokio::io::AsyncReadExt;

pub struct PtySession {
    writer: Box<dyn portable_pty::MasterPty + Send>,
    id: String,
}

#[tauri::command]
async fn pty_create(
    state: tauri::State<'_, AppState>,
    app: AppHandle,
    shell: Option<String>,
) -> Result<String, String> {
    let pty = native_pty_system();
    let pair = pty.openpty(PtySize {
        rows: 24, cols: 80,
        pixel_width: 0, pixel_height: 0,
    }).map_err(|e| e.to_string())?;

    let shell_cmd = shell.unwrap_or_else(|| "/bin/bash".into());
    let cmd = CommandBuilder::new(shell_cmd);
    let _child = pair.slave.spawn_command(cmd).map_err(|e| e.to_string())?;

    let id = uuid::Uuid::new_v4().to_string();
    let reader = pair.master.try_clone_reader().map_err(|e| e.to_string())?;
    let writer = pair.master.take_writer().map_err(|e| e.to_string())?;

    // Spawn reader task → emit events to frontend
    let app_clone = app.clone();
    let id_clone = id.clone();
    tokio::spawn(async move {
        let mut reader = tokio::io::BufReader::new(reader);
        let mut buf = [0u8; 4096];
        loop {
            match reader.read(&mut buf).await {
                Ok(0) => break,
                Ok(n) => {
                    let data = buf[..n].to_vec();
                    let _ = app_clone.emit("pty-output", serde_json::json!({
                        "id": id_clone,
                        "data": base64::encode(&data)
                    }));
                }
                Err(_) => break,
            }
        }
    });

    state.sessions.lock().unwrap().insert(id.clone(), PtySession { writer, id: id.clone() });
    Ok(id)
}

#[tauri::command]
async fn pty_write(
    state: tauri::State<'_, AppState>,
    id: String,
    data: String,
) -> Result<(), String> {
    let sessions = state.sessions.lock().unwrap();
    let session = sessions.get(&id).ok_or("session not found")?;
    let bytes = base64::decode(&data).map_err(|e| e.to_string())?;
    session.writer.write_all(&bytes).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
async fn pty_resize(
    state: tauri::State<'_, AppState>,
    id: String,
    cols: u16,
    rows: u16,
) -> Result<(), String> {
    let sessions = state.sessions.lock().unwrap();
    let session = sessions.get(&id).ok_or("session not found")?;
    session.writer.resize(PtySize { rows, cols, pixel_width: 0, pixel_height: 0 })
        .map_err(|e| e.to_string())
}
```

#### 26.3 Frontend xterm.js
```typescript
// src/terminal.ts
import { Terminal } from '@xterm/xterm';
import { WebglAddon } from '@xterm/addon-webgl';
import { FitAddon } from '@xterm/addon-fit';
import { ImageAddon } from '@xterm/addon-image';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

export async function createTerminal(container: HTMLElement): Promise<string> {
    const term = new Terminal({
        theme: {
            background: '#0d1117',
            foreground: '#e6edf3',
            cursor: '#58a6ff',
            cursorAccent: '#0d1117',
            selectionBackground: '#58a6ff55',
            black: '#0d1117',
            red: '#f85149',
            green: '#3fb950',
            yellow: '#d29922',
            blue: '#58a6ff',
            magenta: '#bc8cff',
            cyan: '#39d2c0',
            white: '#e6edf3',
            brightBlack: '#8b949e',
            brightRed: '#f85149',
            brightGreen: '#3fb950',
            brightYellow: '#d29922',
            brightBlue: '#58a6ff',
            brightMagenta: '#bc8cff',
            brightCyan: '#39d2c0',
            brightWhite: '#e6edf3',
        },
        fontFamily: "'JetBrainsMono Nerd Font', 'JetBrains Mono', monospace",
        fontSize: 13,
        lineHeight: 1.2,
        cursorBlink: true,
        cursorStyle: 'block',
        scrollback: 10000,
        allowImages: true,
    });

    const fitAddon = new FitAddon();
    const webglAddon = new WebglAddon();
    const imageAddon = new ImageAddon();

    term.loadAddon(fitAddon);
    term.loadAddon(imageAddon);
    term.open(container);
    webglAddon.setOptions({ ... }); // GPU rendering
    term.loadAddon(webglAddon);
    fitAddon.fit();

    // Create PTY session
    const sessionId = await invoke('pty_create', { shell: '/bin/fish' });

    // Pipe PTY output → xterm
    await listen(`pty-output-${sessionId}`, (event) => {
        const data = base64decode(event.payload.data);
        term.write(data);
    });

    // Pipe xterm input → PTY
    term.onData((data) => {
        invoke('pty_write', { id: sessionId, data: base64encode(data) });
    });

    // Resize
    term.onResize(({ cols, rows }) => {
        invoke('pty_resize', { id: sessionId, cols, rows });
    });

    window.addEventListener('resize', () => fitAddon.fit());

    return sessionId;
}
```

#### 26.4 Tauri Commands (IPC)
- `pty_create(shell?)` → crea sesión PTY, devuelve ID
- `pty_write(id, data)` → envía keystrokes al PTY
- `pty_resize(id, cols, rows)` → resize del PTY
- `pty_kill(id)` → cierra sesión
- `pty_list()` → lista sesiones activas
- `chat_send(message, model)` → envía mensaje al backend (usa Axum server embedded)
- `chat_stream(conversation_id)` → SSE events → xterm output
- `theme_get()` → devuelve tema activo
- `theme_set(name)` → cambia tema

#### 26.5 System Tray + Window
- Tray icon con menú: New Chat, Settings, Quit
- Global shortcut configurable (default: Super+Alt+H)
- Window state persistence (position, size)
- Native notifications (new messages)
- Auto-update (Tauri updater plugin)

**Tests:** 15+ tests PTY (create, write, read, resize, kill, multi-session)

---

## Semana 27: Chat Terminal Renderer + ACP Connector

### Objetivo: Chat se renderiza como ANSI en xterm, ACP conecta con Claude Code/Codex/AGY

#### 27.1 Chat → ANSI Renderer (`hyprcollab-terminal-render`)

Nuevo crate que convierte mensajes de chat a ANSI strings:

```rust
pub struct ChatRenderer {
    theme: ThemeColors,
    persona_avatars: HashMap<PersonaId, String>, // emoji o ANSI art
}

impl ChatRenderer {
    /// Render user message como bloque ANSI
    pub fn render_user_message(&self, msg: &Message, avatar: &str) -> String {
        format!(
            "\x1b[38;2;88;166;255m▌ You\x1b[0m {}\n{}\n\n",
            msg.timestamp.format("%H:%M"),
            msg.content
        )
    }

    /// Render assistant message con artifact detection
    pub fn render_assistant_message(&self, msg: &Message, persona_name: &str, avatar: &str) -> String {
        let mut out = format!(
            "\x1b[38;2;188;140;255m▌ {}\x1b[0m {}\n",
            persona_name,
            msg.timestamp.format("%H:%M")
        );
        // Parse markdown → ANSI
        out.push_str(&self.markdown_to_ansi(&msg.content));
        // Artifacts
        for artifact in &msg.artifacts {
            out.push_str(&self.render_artifact(artifact));
        }
        out.push('\n');
        out
    }

    /// Render tool call como bloque colapsable
    pub fn render_tool_call(&self, tc: &ToolCall) -> String {
        format!(
            "\x1b[38;2;57;210;192m╭─ {} ─────────────────\x1b[0m\n\
             \x1b[38;2;139;148;158m│ {}\x1b[0m\n\
             \x1b[38;2;57;210;192m╰─────────────────────────\x1b[0m\n",
            tc.name,
            self.truncate(&tc.input, 200)
        )
    }

    /// Markdown → ANSI (headers bold, code blocks cyan bg, links blue underline)
    pub fn markdown_to_ansi(&self, md: &str) -> String {
        // pulldown-cmark parser → ANSI escapes
    }

    /// Artifact inline (código con syntax highlighting, imágenes via kitty protocol)
    pub fn render_artifact(&self, artifact: &Artifact) -> String {
        match artifact.artifact_type {
            ArtifactType::Code => self.render_code_artifact(artifact),
            ArtifactType::Mermaid => self.render_mermaid_placeholder(artifact),
            ArtifactType::Html => self.render_html_preview_note(artifact),
        }
    }
}
```

#### 27.2 ACP Connector (`hyprcollab-acp`)

```rust
// crates/hyprcollab-acp/src/lib.rs
pub trait AcpConnector: Send + Sync {
    fn name(&self) -> &str;
    fn spawn(&mut self, working_dir: &Path) -> Result<Child>;
    fn send_input(&mut self, input: &str) -> Result<()>;
    fn read_output(&mut self) -> Result<Vec<AcpEvent>>;
    fn is_running(&self) -> bool;
    fn terminate(&mut self) -> Result<()>;
}

pub enum AcpEvent {
    Token(String),           // streamed text
    ToolCallStart(ToolCall),  // tool invocation started
    ToolCallEnd(ToolResult),  // tool result
    Error(String),
    Done,
}

// Implementaciones:
// - ClaudeCodeConnector: `claude -p --output-format stream-json`
// - CodexConnector: `codex --acp --stdio`
// - AgYConnector: `agy -p --output-format stream-json`
```

**Tests:** 20+ tests (ANSI rendering, markdown→ANSI, artifact rendering, ACP spawn/mock)

---

## Semana 28: Sidebar Dioxus + Chat Integration + ACP Monitor

### Objetivo: Sidebar funcional con Dioxus, chat end-to-end via terminal renderer, ACP monitor panel

#### 28.1 Sidebar Component (Dioxus)

```rust
// platforms/hyprcollab-tauri/src/sidebar.rs
#[component]
fn Sidebar() -> Element {
    let mut active = use_signal(|| "chats");
    rsx! {
        div { class: "sidebar",
            // Tab bar
            div { class: "sidebar-tabs",
                button { class: if active() == "chats" { "active" },
                    onclick: move |_| active.set("chats"),
                    "💬 Chats"
                }
                button { class: if active() == "projects" { "active" },
                    onclick: move |_| active.set("projects"),
                    "📁 Projects"
                }
                button { class: if active() == "personas" { "active" },
                    onclick: move |_| active.set("personas"),
                    "🎭 Personas"
                }
            }
            // Content panels
            match active() {
                "chats" => rsx! { ChatList {} },
                "projects" => rsx! { ProjectList {} },
                "personas" => rsx! { PersonaList {} },
                _ => rsx! {},
            }
        }
    }
}
```

#### 28.2 Chat Input (Dioxus overlay sobre xterm)
- Textarea con syntax highlighting (slash commands)
- Autocomplete de `/commands`
- File drag & drop
- Adjuntar imágenes (preview via kitty protocol)
- Botón Send (también Enter)

#### 28.3 ACP Monitor Panel
- Panel lateral derecho (colapsable)
- Lista de sesiones ACP activas (Claude Code, Codex, AGY)
- Stream de output en mini-xterm
- Tool call cards (diff archivos, comandos ejecutados)
- Botones: Start, Stop, Restart, Open in Editor

#### 28.4 Integración Chat
- User escribe en input → `invoke('chat_send')` → Axum backend → LLM
- LLM response (SSE) → Tauri Event `chat-stream` → ChatRenderer → ANSI → xterm.write()
- Artifacts se renderizan inline en el canvas xterm
- Tool calls se muestran como bloques ANSI colapsables

**Tests:** 15+ tests (sidebar rendering, input handling, ACP mock sessions)

---

## Semana 29: Voice + Multi-model Compare + PWA + Polish

### Objetivo: Voice mode, comparar modelos, PWA mobile, polish visual

#### 29.1 Voice Mode
- `hyprcollab-voice`: Whisper.cpp bindgen (via whisper-rs) + cpal audio capture
- Push-to-talk: mantener Space → grabar → transcribe → envía como mensaje
- TTS: edge-tts integration → reproduce respuesta como audio
- Visual: indicador de grabación en status bar (mic icon + waveform ANSI)

#### 29.2 Multi-model Compare
- ParallelProvider: envía mismo prompt a N modelos simultáneamente
- Split view: divide xterm en paneles (tmux-style)
- Cada panel muestra output de un modelo diferente con su color
- Diff view: comparar respuestas lado a lado

#### 29.3 PWA Mobile (`hyprcollab-pwa`)
- Service worker para offline
- Manifest.json con theme colors
- Touch-optimized input bar
- Responsive sidebar (drawer en mobile)
- NOT xterm.js en mobile — usar div con estética terminal (CSS monospace + ANSI parser)

#### 29.4 Polish Visual
- Animaciones: cursor blink, fade-in messages, smooth scroll
- Sound effects opcionales (terminal bell, notification)
- Keyboard shortcuts fully functional
- Window transparency (Tauri option)
- Tab management (multiple chat tabs)

**Tests:** 10+ tests (voice capture mock, parallel provider, PWA manifest validation)

---

## Dependencias Crates (Tauri)

### Rust (src-tauri/Cargo.toml)
```toml
[dependencies]
tauri = { version = "2", features = ["tray-icon", "devtools"] }
tauri-plugin-shell = "2"
tauri-plugin-notification = "2"
tauri-plugin-updater = "2"
tauri-plugin-fs = "2"
tauri-plugin-dialog = "2"
portable-pty = "0.9"
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
base64 = "0.22"
uuid = { version = "1", features = ["v4"] }
hyprcollab-core = { path = "../../crates/hyprcollab-core" }
hyprcollab-server = { path = "../../crates/hyprcollab-server" }
hyprcollab-memory = { path = "../../crates/hyprcollab-memory" }
```

### Frontend (package.json)
```json
{
  "dependencies": {
    "@tauri-apps/api": "^2",
    "@tauri-apps/plugin-shell": "^2",
    "@tauri-apps/plugin-notification": "^2",
    "@xterm/xterm": "^6.0.0",
    "@xterm/addon-webgl": "^0.19.0",
    "@xterm/addon-fit": "^0.11.0",
    "@xterm/addon-image": "^0.9.0",
    "@xterm/addon-search": "^0.16.0",
    "@xterm/addon-unicode11": "^0.9.0"
  },
  "devDependencies": {
    "vite": "^6",
    "typescript": "^5"
  }
}
```

---

## Referencia: Proyectos similares

| Proyecto | Stars | Stack | Relevancia |
|----------|-------|-------|------------|
| [Kerminal](https://github.com/klpod221/kerminal) | 434 | Tauri + Vue 3 + xterm | Terminal emulator + SSH manager, PTY completo |
| [skills.sh](https://skills.sh) | — | Next.js + terminal aesthetic | UI terminal-like, monospace grid |
| VS Code Terminal | — | Electron + xterm.js | PTY + xterm reference implementation |

## Skills.sh Skills aplicables

| Skill | Installs | Uso |
|-------|----------|-----|
| `nodnarbnitram/claude-code-extensions/tauri-v2` | 4,687 | Tauri 2.0 architecture |
| `dchuk/claude-code-tauri-skills/*` | ~200 each | 40 skills específicos |
| `dchuk/.../tauri-architecture` | — | Core-Shell pattern |
| `dchuk/.../tauri-ipc` | — | Commands + Events |
| `dchuk/.../tauri-frontend-events` | — | Event streaming |

---

## Milestone 6

- ✅ Desktop app Tauri 2.0 con PTY real
- ✅ xterm.js con GPU rendering
- ✅ Chat renderizado como ANSI terminal
- ✅ Sidebar Dioxus funcional
- ✅ ACP connector (CC/Codex/AGY)
- ✅ Voice mode (STT + TTS)
- ✅ Multi-model compare (split pane)
- ✅ PWA mobile responsive
- ✅ Estética terminal-first consistente en todas las plataformas
