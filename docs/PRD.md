     1|# HyprCollab — Product Requirements Document
     2|
     3|> **OpenSpec SSD v2.0** · Agentic AI Chat Platform · Rust-Native · Multi-Platform
     4|
     5|---
     6|
     7|## 1. Resumen Ejecutivo
     8|
     9|**HyprCollab** es una plataforma de chat agéntico open-source escrita 100% en Rust que fusiona lo mejor de cuatro proyectos de referencia:
    10|
    11|- **LibreChat** → Visual UX + Artifacts + MCP
    12|- **AnythingLLM** → Pipeline RAG + gestión documental + workspaces
    13|- **ClaudeCodeUI** → Visualización ACP + agent sessions
    14|- **Hermes Agent** → Memoria evolutiva + skills + aprendizaje automático
    15|
    16|**Diferencial:** Multi-plataforma nativa con estética terminal-first. Desktop app (Tauri), web server, mobile (PWA), y TUI (terminal). No es un ChatGPT clone — es una herramienta para desarrolladores que respira consola.
    17|
    18|**Nombre:** HyprCollab (plataforma de colaboración humano+IA para desarrolladores)
    19|**Stack:** Rust fullstack — Axum + Tauri 2.0 + Dioxus WASM + rig-rs
    20|**Licencia:** MIT
    21|**Deploy:** Self-hosted first, AUR + Docker + Nix + AppImage
    22|
    23|---
    24|
    25|## 2. Diagramas de Arquitectura
    26|
    27|### 2.1 Arquitectura General
    28|
    29|```mermaid
    30|graph TB
    31|    subgraph "Plataformas"
    32|        DESKTOP["🖥️ Desktop<br/>(Tauri 2.0)"]
    33|        WEB["🌐 Web Server<br/>(Axum + WASM)"]
    34|        MOBILE["📱 Mobile<br/>(Tauri 2.0 / PWA)"]
    35|        TUI["⌨️ Terminal TUI<br/>(ratatui)"]
    36|    end
    37|
    38|    subgraph "Frontend Layer"
    39|        UI["Dioxus 0.6 / ratatui"]
    40|        CANVAS["Artifacts Canvas"]
    41|        CHATUI["Chat Panel"]
    42|        SIDEBAR["Sidebar + Nav"]
    43|        BROWSERUI["Browser View"]
    44|    end
    45|
    46|    subgraph "Backend Core (Axum)"
    47|        API["API Gateway<br/>REST + SSE + WS"]
    48|        AGENT["Agent Runtime<br/>(rig-rs)"]
    49|        LLM["LLM Router<br/>Multi-provider"]
    50|        MEMORY["Memory Engine<br/>(SQLite)"]
    51|        RAG["RAG Pipeline<br/>(LanceDB)"]
    52|        TOOLS["Tool System"]
    53|        SKILLS["Skills Engine"]
    54|        ACP["ACP Connector"]
    55|        CMDS["Slash Commands"]
    56|        APPROVAL["Approval Engine"]
    57|    end
    58|
    59|    subgraph "Storage"
    60|        SQLITE["SQLite + FTS5"]
    61|        LANCE["LanceDB<br/>(vectors)"]
    62|        FS["Filesystem<br/>(artifacts, configs)"]
    63|    end
    64|
    65|    subgraph "External Agents"
    66|        CLAUDE["Claude Code"]
    67|        CODEX["Codex CLI"]
    68|        AGY["Antigravity CLI"]
    69|    end
    70|
    71|    DESKTOP --> UI
    72|    WEB --> UI
    73|    MOBILE --> UI
    74|    TUI --> UI
    75|    UI --> CANVAS
    76|    UI --> CHATUI
    77|    UI --> SIDEBAR
    78|    UI --> BROWSERUI
    79|    UI --> API
    80|    API --> AGENT
    81|    API --> LLM
    82|    API --> MEMORY
    83|    API --> RAG
    84|    API --> TOOLS
    85|    API --> SKILLS
    86|    API --> ACP
    87|    API --> CMDS
    88|    TOOLS --> APPROVAL
    89|    AGENT --> LLM
    90|    AGENT --> MEMORY
    91|    AGENT --> RAG
    92|    AGENT --> TOOLS
    93|    MEMORY --> SQLITE
    94|    RAG --> LANCE
    95|    TOOLS --> FS
    96|    ACP --> CLAUDE
    97|    ACP --> CODEX
    98|    ACP --> AGY
    99|```
   100|
   101|### 2.2 User Journey
   102|
   103|```mermaid
   104|flowchart LR
   105|    A[Usuario abre HyprCollab] --> B{Plataforma?}
   106|    B -->|Desktop| C[Tauri Window]
   107|    B -->|Web| D[Browser WASM]
   108|    B -->|Terminal| E[TUI ratatui]
   109|    B -->|Mobile| F[PWA / Tauri Mobile]
   110|    
   111|    C --> G[Chat Interface]
   112|    D --> G
   113|    E --> G
   114|    F --> G
   115|    
   116|    G --> H{Modo?}
   117|    H -->|Chat| I[Streaming conversation]
   118|    H -->|/.agent| J[Carga agent.md]
   119|    H -->|/.skill| K[Carga skill.md]
   120|    H -->|/.design| L[Design canvas]
   121|    H -->|/.browse| M[Web browser]
   122|    H -->|/.run| N[System command]
   123|    
   124|    I --> O[Agent responde<br/>con tools + reasoning]
   125|    J --> O
   126|    K --> O
   127|    L --> P[Canvas colaborativo]
   128|    M --> Q[Preview web]
   129|    N --> R{Aprobación?}
   130|    R -->|Auto| S[Ejecuta]
   131|    R -->|Manual| T[Dialog de approval]
   132|    T -->|Approve| S
   133|    T -->|Deny| U[Cancelado]
   134|```
   135|
   136|### 2.3 Flujo de Aprobación de Ejecuciones
   137|
   138|```mermaid
   139|sequenceDiagram
   140|    participant Agent
   141|    participant ApprovalEngine
   142|    participant ApprovalUI
   143|    participant User
   144|    participant System
   145|
   146|    Agent->>ApprovalEngine: Tool request (shell, file_write, etc.)
   147|    ApprovalEngine->>ApprovalEngine: Check rules (regex, tool, workspace)
   148|    
   149|    alt Auto-approve match
   150|        ApprovalEngine->>System: Execute directly
   151|        System-->>Agent: Result
   152|    else Requires approval
   153|        ApprovalEngine->>ApprovalUI: Show approval dialog
   154|        ApprovalUI->>User: "Command: rm -rf /tmp/build/*<br/>[Approve] [Deny] [Modify]"
   155|        User->>ApprovalUI: Decision
   156|        alt Approved
   157|            ApprovalUI->>System: Execute
   158|            System-->>Agent: Result
   159|        else Modified
   160|            ApprovalUI->>System: Execute modified command
   161|            System-->>Agent: Result
   162|        else Denied
   163|            ApprovalUI-->>Agent: Denied with reason
   164|        end
   165|    end
   166|    
   167|    ApprovalEngine->>ApprovalEngine: Log to approval_log table
   168|```
   169|
   170|---
   171|
   172|## 3. Tech Stack
   173|
   174|- **Backend Core:** Rust + Axum 0.8 — performance, safety, async-native
   175|- **Agent Runtime:** rig-rs (0xPlaygrounds/rig) ⭐7.4k — framework LLM Rust
   176|- **Frontend Web:** Dioxus 0.6 ⭐36k — fullstack Rust → WASM
   177|- **Desktop App:** Tauri 2.0 — compila a binario nativo (Linux/macOS/Windows)
   178|- **TUI:** ratatui + crossterm — terminal UI para modo consola
   179|- **Mobile:** Tauri 2.0 mobile target + PWA manifest
   180|- **DB Principal:** SQLite (rusqlite) + FTS5 — local-first, zero-config
   181|- **DB Vectorial:** LanceDB (Rust native) — RAG embeddings
   182|- **Browser Engine:** headless_chrome / chromiumoxide — navegación web nativa
   183|- **Image Preview:** kitty graphics protocol + sixel fallback (TUI mode)
   184|- **MCP:** rust-mcp-sdk — Model Context Protocol nativo
   185|- **Voice:** Whisper.cpp (bindgen) + edge-tts — input/output local
   186|- **Auth:** JWT + OAuth2 — multi-user seguro
   187|- **Config:** config.yaml (serde_yaml) — XDG: ~/.config/hyprcollab/
   188|
   189|---
   190|
   191|## 4. Modos de Plataforma
   192|
   193|### 4.1 Desktop App (Tauri 2.0)
   194|
   195|- Binario compilado nativo para Linux (AppImage/AUR), macOS (.dmg), Windows (.msi)
   196|- WebView2/WebKit integrado — sin navegador externo
   197|- Acceso nativo: filesystem, system tray, notifications, global shortcuts
   198|- Ventana con tabs, sidebar resizable, canvas panel
   199|- Auto-update integrado
   200|
   201|### 4.2 Web Server
   202|
   203|- `hyprcollab serve` → Axum sirve frontend WASM compilado
   204|- Accesible desde cualquier navegador en red local
   205|- PWA-ready: manifest.json, service worker, offline mode
   206|- Responsive design para tablets y mobile browsers
   207|
   208|### 4.3 TUI Mode (Terminal)
   209|
   210|- `hyprcollab tui` → interfaz ratatui en terminal
   211|- Kitty image protocol: preview de imágenes, PDFs, código directamente en terminal
   212|- Sixel fallback para terminales que no soportan kitty
   213|- Vim-like keybinds: j/k navegar, Enter enviar, Tab completar
   214|- Split pane: chat + artifact preview lado a lado
   215|- Detección automática de capabilities de la terminal
   216|
   217|### 4.4 Mobile
   218|
   219|- Tauri 2.0 mobile: iOS + Android targets
   220|- PWA: installable desde browser con push notifications
   221|- Touch-optimized: swipe para sidebar, pull-to-refresh
   222|- Voice mode: push-to-talk con STT/TTS
   223|
   224|---
   225|
   226|## 5. Features
   227|
   228|### P0 — MVP (Mes 1-3)
   229|
   230|- **Chat con streaming** — Respuestas token-a-token vía SSE
   231|- **Multi-provider LLM** — OpenAI, Anthropic, Ollama, OpenRouter, OpenAI-compatible
   232|- **Terminal aesthetic UI** — Dark #0d1117, JetBrainsMono NF, glifos NF
   233|- **Artifacts Canvas** — Panel lateral con preview de código, markdown, SVG, Mermaid, React
   234|- **Tool calls visibles** — Mostrar reasoning steps y tool execution inline
   235|- **RAG pipeline** — Upload docs → embeddings → query con contexto
   236|- **Agent system** — Agente con tools (shell, web, files), loops multi-step
   237|- **Memoria persistente** — SQLite: conversaciones, preferencias, contexto cruzado
   238|- **Config YAML** — ~/.config/hyprcollab/config.yaml
   239|- **MCP Client** — Conexión a servidores MCP externos
   240|- **Slash commands básicos** — `/.agent`, `/.skill`, `/.mcp`, `/.model`, `/.workspace`
   241|- **Aprobación de ejecuciones** — Human-in-the-loop para tools peligrosas
   242|
   243|### P1 — v1.1 (Mes 4-6)
   244|
   245|- **Desktop app (Tauri)** — App nativa Linux/macOS/Windows
   246|- **TUI mode** — Interfaz ratatui con kitty image preview
   247|- **Workspaces** — Separación de contexto por proyecto
   248|- **Skills system** — Skills YAML instalables con auto-learning
   249|- **ACP Output viz** — Visualización de sesiones ACP (Claude Code, Codex, AGY)
   250|- **Multi-agent** — Delegación a subagentes con contexto aislado
   251|- **Voice I/O** — Whisper STT + TTS edge/elevenlabs
   252|- **Code interpreter** — Sandbox (Docker/Wasmtime) para ejecución
   253|- **Navegación web nativa** — Headless browser embebido
   254|- **Inline artifacts** — Artefactos embebidos en el chat (no solo panel lateral)
   255|- **Conversation branching** — Fork desde cualquier mensaje
   256|- **Temas** — Terminal-dark, Catppuccin, Nord, Dracula, Tokyo Night
   257|
   258|### P2 — v2.0 (Mes 7-12)
   259|
   260|- **Mobile** — Tauri 2.0 mobile + PWA completa
   261|- **Multi-user** — Auth, roles, shared workspaces
   262|- **Cron/Scheduler** — Tareas programadas con output al chat
   263|- **Collaborative canvas** — Edición simultánea humano+IA
   264|- **Multi-model compare** — Enviar mismo prompt a N modelos lado a lado
   265|- **Export** — Markdown, JSON, PDF, PNG
   266|- **Prompt templates** — Library de prompts reusables con variables
   267|- **i18n** — Multi-idioma (ES, EN, JA, ZH)
   268|- **Observability** — Tracing, metrics, token usage dashboard
   269|- **WASM plugins** — Sistema de plugins cargables en runtime
   270|- **Search avanzado** — FTS5 full-text search + semantic search
   271|- **Bookmarks + Pinned** — Marcar mensajes y fijar conversaciones
   272|
   273|---
   274|
   275|## 6. Slash Commands Agénticos
   276|
   277|Sistema extensible de comandos que cargan configuración desde archivos `.md`:
   278|
   279|- `/.agent <name>` — Carga `~/.config/hyprcollab/agents/<name>.md` con system prompt, tools, config del agente
   280|- `/.skill <name>` — Carga `~/.config/hyprcollab/skills/<name>.md` con procedimiento y steps
   281|- `/.mcp <server>` — Conecta/desconecta servidor MCP
   282|- `/.acp <connector>` — Inicia sesión ACP (claude-code, codex, agy)
   283|- `/.design` — Carga `design.md` y entra en modo diseño (canvas colaborativo)
   284|- `/.review` — Carga `review.md` y entra en modo code review
   285|- `/.run <command>` — Ejecuta comando del sistema (con aprobación previa si aplica)
   286|- `/.browse <url>` — Abre navegador web nativo embebido
   287|- `/.model <provider/model>` — Cambia modelo en runtime
   288|- `/.workspace <name>` — Cambia workspace activo
   289|- `/.theme <name>` — Cambia tema visual
   290|- `/.export <format>` — Exporta conversación (md, json, pdf)
   291|- `/.compare` — Compara dos modelos lado a lado
   292|- `/.bookmark` — Marca mensaje actual
   293|- `/.help` — Lista todos los comandos disponibles
   294|
   295|Cada command carga su `.md` correspondiente. Los archivos tienen formato frontmatter YAML + markdown body con instrucciones.
   296|
   297|---
   298|
   299|## 7. Kitty Image Protocol (TUI Mode)
   300|
   301|Cuando HyprCollab se ejecuta como TUI en terminal:
   302|
   303|- **Detección automática:** Check kitty graphics protocol support via escape sequences, fallback a Sixel
   304|- **Preview de imágenes:** PNG, JPG, JPEG, WebP, BMP renderizados inline en terminal
   305|- **Preview de SVG:** Renderizado via resvg a PNG temporal, luego mostrado
   306|- **Preview de código:** Syntax highlighted via syntect con tema Catppuccin
   307|- **Preview de PDF:** Primera página renderizada como imagen
   308|- **Preview de Markdown:** Renderizado a HTML temporal → screenshot → mostrado
   309|- **Artifact canvas:** En TUI, los artifacts se previsualizan inline usando el protocolo
   310|- **Controls:** Click para zoom, scroll para navegar páginas en PDF
   311|
   312|---
   313|
   314|## 8. Navegación Web Nativa
   315|
   316|Browser engine embebido sin dependencias externas de plugins:
   317|
   318|- **Modo headless:** Scraping, JS execution, screenshots para tools del agente
   319|- **Modo visual:** En desktop/Tauri, renderizado completo en canvas panel
   320|- **Tools del agente:**
   321|  - `web_navigate(url)` — Navega a URL, devuelve PageSnapshot
   322|  - `web_screenshot()` — Captura screenshot de la página actual
   323|  - `web_extract(selector)` — Extrae contenido por CSS selector
   324|  - `web_interact(action, target)` — Click, type, scroll, submit
   325|  - `web_js_execute(code)` — Ejecuta JavaScript sandboxed
   326|- **Cookie/session management:** Persistente entre sesiones
   327|- **RAG auto-indexing:** Opción de auto-indexar páginas visitadas al workspace
   328|
   329|---
   330|
   331|## 9. Ejecución de Comandos del Sistema
   332|
   333|Shell tool mejorado con capacidades avanzadas:
   334|
   335|- **Multi-shell:** Detecta bash, fish, zsh automáticamente
   336|- **Working directory:** CWD persiste entre comandos (tracking continuo)
   337|- **Environment variables:** Set/get/unset con persistencia opcional
   338|- **Long-running processes:** Output streaming en tiempo real
   339|- **Background processes:** Start, stop, check status, get output
   340|- **PTY allocation:** Para comandos interactivos (vim-style editors, REPLs)
   341|- **Timeout configurable:** Por comando, con kill automático
   342|- **Output capture:** Stdout + stderr separados, con encoding detection
   343|
   344|---
   345|
   346|## 10. Aprobación Previa de Ejecuciones
   347|
   348|Sistema human-in-the-loop configurable:
   349|
   350|- **Reglas por tool:** Cada tool puede configurarse como auto-approve o requires-approval
   351|- **Pattern matching:** Regex sobre el comando/acción para auto-approve específico
   352|- **Scope:** Reglas por workspace, global, o por agente
   353|- **UI de aprobación:** Muestra comando completo, diff si es archivo, contexto
   354|- **Acciones:** Approve, Deny, Modify (editar comando antes de ejecutar)
   355|- **Timeout:** Si el usuario no responde en X segundos, deny automáticamente
   356|- **Bulk approval:** Para operaciones batch (ej: "approve all file reads")
   357|- **Log persistente:** Todas las decisiones se registran en SQLite para auditoría
   358|- **Config ejemplo:**
   359|  - `ask_before: ["shell", "file_write", "web_interact"]`
   360|  - `auto_approve_patterns: ["git status", "ls *", "cat *.md"]`
   361|  - `approval_timeout: 300` (5 minutos)
   362|
   363|---
   364|
   365|## 11. Funciones Extra (de otras apps)
   366|
   367|Funcionalidades inspiradas en ChatGPT, OpenClaw, LobeChat, Open WebUI:
   368|
   369|- **Branching:** Fork desde cualquier mensaje, explorar ramas sin perder el hilo original
   370|- **Inline artifacts:** Artefactos embebidos directamente en el chat (no solo panel lateral)
   371|- **Voice mode:** Push-to-talk con Whisper STT + TTS, modo hands-free
   372|- **Export:** Conversaciones a Markdown, JSON, PDF, PNG (screenshot)
   373|- **Temas:** Terminal-dark (default), Catppuccin, Nord, Dracula, Tokyo Night, custom CSS
   374|- **Keyboard-first:** Vim-like keybinds (j/k, :wq, gg/G), shortcuts personalizables
   375|- **Session persistence:** Reabrir exactamente donde se dejó (scroll position, open panels)
   376|- **Token tracking:** Dashboard de consumo por modelo, día, workspace, conversación
   377|- **Prompt templates:** Library con variables `${variable}`, compartibles
   378|- **Multi-model compare:** Mismo prompt a N modelos, respuestas lado a lado
   379|- **Diff view:** Comparar dos respuestas del mismo modelo o de modelos distintos
   380|- **Full-text search:** FTS5 sobre todas las conversaciones con ranking
   381|- **Bookmarks:** Marcar mensajes importantes para acceso rápido
   382|- **Pinned conversations:** Fijar chats frecuentes arriba en sidebar
   383|- **Drag & drop:** Subir archivos, imágenes directamente al chat
   384|- **Code block actions:** Copy, download, open in editor, run en code interpreter
   385|
   386|---
   387|
   388|## 12. User Stories
   389|
   390|**Chat Agéntico:**
   391|> "Como desarrollador, quiero chatear con un agente que use herramientas (shell, web, archivos) y vea cada paso de su razonamiento en tiempo real, con aprobación manual para comandos peligrosos."
   392|
   393|**Artifacts Multi-plataforma:**
   394|> "Como usuario, quiero que el agente genere código, documentos y visualizaciones en un canvas — visible como panel en desktop, inline en TUI, y responsive en mobile."
   395|
   396|**Slash Commands:**
   397|> "Como usuario avanzado, quiero escribir `/.agent security-auditor` y que el agente cargue su configuración desde agent.md, cambiando system prompt, tools disponibles y reglas de aprobación."
   398|
   399|**Kitty Preview:**
   400|> "Como usuario de terminal, quiero ver previews de imágenes generadas por el agente directamente en mi terminal kitty, sin abrir otra aplicación."
   401|
   402|**Aprobación:**
   403|> "Como desarrollador, quiero que el agente me pida confirmación antes de ejecutar `rm -rf` o escribir archivos en producción, con opción de modificar el comando antes de aprobar."
   404|
   405|**Web Nativa:**
   406|> "Como usuario, quiero pedirle al agente que navegue una web, extraiga datos y los indexe en RAG — todo sin plugins externos, directamente desde el chat."
   407|
   408|**RAG:**
   409|> "Como usuario, quiero subir documentos a un workspace y hacer preguntas con contexto del contenido, sin enviar datos a terceros."
   410|
   411|**ACP Visualization:**
   412|> "Como desarrollador, quiero conectar Claude Code o Codex y ver sus outputs formateados en mi interfaz, desde cualquier plataforma."
   413|
   414|**Memoria:**
   415|> "Como usuario recurrente, quiero que el agente recuerde mis preferencias, proyectos y patrones de uso entre sesiones."
   416|
   417|---
   418|
   419|## 13. No-Alcance (v1.0)
   420|
   421|- ❌ Multi-tenant enterprise (SaaS)
   422|- ❌ Fine-tuning de modelos
   423|- ❌ Marketplace público de plugins (solo local)
   424|- ❌ Colaboración multi-usuario en tiempo real → v2.0
   425|- ❌ Soporte Windows nativo (WSL only en v1)
   426|- ❌ Video generation/editing
   427|
   428|---
   429|
   430|## 14. Modos de Operación
   431|
   432|- **Modo Chat** — Interacción estándar usuario↔agente con streaming
   433|- **Modo Canvas** — Chat + panel de artifacts. Agente genera, usuario edita
   434|- **Modo Agent** — Agente autónomo con tools. Muestra reasoning chain + tool calls
   435|- **Modo RAG** — Upload documentos → query con contexto vectorial
   436|- **Modo ACP** — Conecta agentes externos y visualiza su output
   437|- **Modo Design** — Canvas colaborativo con design.md cargado
   438|- **Modo Review** — Code review con diff view + comentarios
   439|- **Modo Voice** — Push-to-talk con STT/TTS, hands-free
   440|- **Modo Browse** — Navegación web con preview visual
   441|
   442|---
   443|
   444|## 15. KPIs y Criterios de Éxito
   445|
   446|- **Latencia primer token:** < 200ms (v1.0), < 100ms (v2.0)
   447|- **Throughput streaming:** > 100 tok/s (v1.0), > 200 tok/s (v2.0)
   448|- **Memory footprint:** < 150MB desktop, < 50MB TUI
   449|- **RAG query latency:** < 500ms local
   450|- **Artifacts render:** < 50ms
   451|- **Test coverage:** > 80%
   452|- **GitHub stars (6 meses):** > 1k
   453|- **Desktop startup:** < 2 segundos
   454|
   455|---
   456|
   457|## 16. Análisis Competitivo
   458|
   459|- **HyprCollab (propuesto):** Rust ✅ · Multi-platform ✅ · Artifacts Canvas ✅ · RAG ✅ · Agentes ✅ · ACP ✅ · Terminal ✅ · Kitty preview ✅ · Web nativa ✅ · Approval ✅
   460|- **LibreChat** ⭐37k (TS): Artifacts ✅ · RAG ❌ · Agentes ✅ · ACP ❌ · Desktop ❌ · Terminal ❌
   461|- **AnythingLLM** ⭐60k (JS): RAG ✅ · Agentes básico ✅ · Desktop ✅ (Electron) · Terminal ❌
   462|- **ClaudeCodeUI** ⭐11k (TS): ACP ✅ · Desktop ❌ · Terminal ❌
   463|- **Hermes Agent** ⭐164k (Python): Agentes full ✅ · Desktop ❌ · Terminal ❌ (messenger)
   464|- **OpenClaw** ⭐374k (TS): Canvas ✅ · Desktop ❌ · Terminal ❌
   465|- **Open WebUI** ⭐85k (Svelte+Py): RAG ✅ · Agentes ✅ · Terminal ❌
   466|- **LobeChat** ⭐62k (Next.js): Artifacts ✅ · Agentes ✅ · Desktop ❌
   467|
   468|**Diferencial HyprCollab:** Único Rust-native + multi-plataforma (desktop/web/TUI/mobile) + terminal aesthetic + kitty preview + web nativa + approval system + slash commands agénticos.
   469|
   470|---
   471|
   472|## 17. Riesgos
   473|
   474|- **rig-rs inmaduro** (Media/Alto) — Fork + contribuir; fallback a bindings Python vía PyO3
   475|- **Tauri 2.0 mobile estable** (Media/Medio) — PWA como fallback, mobile es P2
   476|- **LanceDB Rust bindings** (Media/Medio) — Fallback a sqlite-vec o Qdrant
   477|- **Scope creep** (Alto/Alto) — MVP estricto: solo P0 features, 3 meses
   478|- **Headless Chrome dependency** (Baja/Medio) — Fallback a servo o reqwest+scraper
   479|- **Kitty protocol limited support** (Baja/Bajo) — Sixel fallback, solo TUI mode
   480|
   481|---
   482|
   483|## 18. Roadmap
   484|
   485|- **Fase 0** (Sem 1-2) — Foundation: Repo, Cargo workspace, CI, core types
   486|- **Fase 1** (Sem 3-6) — Core Chat: Streaming, multi-provider, UI terminal, slash commands
   487|- **Fase 2** (Sem 7-11) — Agents + Tools: Runtime, tools, approval, TUI mode, kitty preview
   488|- **Fase 3** (Sem 12-15) — Artifacts Canvas: Canvas, inline artifacts, mermaid render
   489|- **Fase 4** (Sem 16-19) — RAG + Browser: Pipeline RAG, web browser nativo
   490|- **Fase 5** (Sem 20-24) — Memory + Skills: Memory evolutiva, skills, auto-learning
   491|- **Fase 6** (Sem 25-28) — Desktop + ACP: Tauri app, ACP visualization, themes
   492|- **Fase 7** (Sem 29-34) — Polish + Release: Tests, docs, AUR, Docker, mobile PWA
   493|
   494|**Total estimado: ~34 semanas (8.5 meses)**
   495|
   496|---
   497|
   498|## 19. Oportunidades Adicionales
   499|
   500|1. **Zed Editor Plugin** — HyprCollab como assistant panel en Zed (Rust native)
   501|