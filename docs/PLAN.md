     1|# HyprCollab — Implementation Plan
     2|
     3|> **OpenSpec SSD v2.0** · 8 Fases · 34 Semanas
     4|
     5|---
     6|
     7|## Dependencias entre Fases
     8|
     9|```mermaid
    10|graph LR
    11|    M0["M0: Foundation<br/>(sem 1-2)"] --> M1["M1: Core Chat<br/>(sem 3-6)"]
    12|    M1 --> M2["M2: Agents + Tools<br/>(sem 7-11)"]
    13|    M2 --> M3["M3: Artifacts<br/>(sem 12-15)"]
    14|    M2 --> M4["M4: RAG + Browser<br/>(sem 16-19)"]
    15|    M2 --> M5["M5: Memory + Skills<br/>(sem 20-24)"]
    16|    M3 --> M6["M6: Desktop + ACP<br/>(sem 25-28)"]
    17|    M4 --> M6
    18|    M5 --> M6
    19|    M6 --> M7["M7: Polish + Release<br/>(sem 29-34)"]
    20|```
    21|
    22|M3, M4, M5 son **paralelizables** después de M2.
    23|
    24|---
    25|
    26|## Fase 0 — Foundation (Semanas 1-2)
    27|
    28|### Semana 1: Scaffold + CI
    29|
    30|- [ ] `cargo init --name hyprcollab` en ~/developments/hyprcollab/
    31|- [ ] Crear Cargo workspace con todos los crates (20 miembros)
    32|- [ ] Configurar CI: GitHub Actions (fmt, clippy, test, build)
    33|- [ ] Crear repositorio GitHub `hyprcollab`
    34|- [ ] Escribir README.md con badge de CI
    35|- [ ] Crear `deny.toml` para auditoría de deps
    36|- [ ] Crear `flake.nix` para dev shell
    37|- [ ] Crear `Dockerfile` multi-stage
    38|
    39|### Semana 2: Core Types + Traits
    40|
    41|- [ ] `hyprcollab-core`: tipos compartidos (ChatRequest, Message, Artifact, ToolCall, etc.)
    42|- [ ] `hyprcollab-core`: traits (LlmProvider, Tool, AcpConnector, PlatformAdapter, SlashCommand)
    43|- [ ] `hyprcollab-core`: error types (thiserror)
    44|- [ ] `hyprcollab-core`: tests unitarios de tipos
    45|- [ ] `hyprcollab-memory`: schema SQLite + migraciones (todas las tablas)
    46|- [ ] `hyprcollab-memory`: CRUD básico de conversaciones y mensajes
    47|- [ ] `hyprcollab-memory`: tests con DB in-memory
    48|- [ ] `hyprcollab-approval`: ApprovalRule types + ApprovalEngine skeleton
    49|
    50|**Milestone 0:** Workspace compila, CI verde, SQLite migrations corren, ~20 crates.
    51|
    52|---
    53|
    54|## Fase 1 — Core Chat + Slash Commands (Semanas 3-6)
    55|
    56|### Semana 3: LLM Router
    57|
    58|- [ ] `hyprcollab-provider-openai`: trait LlmProvider para OpenAI
    59|- [ ] `hyprcollab-provider-openai`: streaming SSE (reqwest + tokio-stream)
    60|- [ ] `hyprcollab-provider-openai`: embeddings endpoint
    61|- [ ] `hyprcollab-provider-anthropic`: Messages API + streaming
    62|- [ ] Tests unitarios por provider con mock server
    63|
    64|### Semana 4: Más Providers + Router + Slash Commands
    65|
    66|- [ ] `hyprcollab-provider-ollama`: API compatible
    67|- [ ] `hyprcollab-provider-openrouter`: aggregator
    68|- [ ] Router: selección por model name, fallback config
    69|- [ ] Config loading (serde_yaml) con env var interpolation
    70|- [ ] `hyprcollab-commands`: SlashCommand trait + parser básico
    71|- [ ] Implementar `/.model`, `/.workspace`, `/.help`
    72|
    73|### Semana 5: Axum Server
    74|
    75|- [ ] `hyprcollab-server`: Axum app con routes
    76|- [ ] POST `/api/chat/completions` → SSE stream
    77|- [ ] GET/DELETE `/api/conversations`
    78|- [ ] GET `/api/conversations/:id`
    79|- [ ] AppState con DB pool + LLM router
    80|- [ ] CORS middleware
    81|
    82|### Semana 6: Frontend Chat + Terminal UI
    83|
    84|- [ ] Dioxus setup con `dx serve`
    85|- [ ] Chat panel: lista de mensajes
    86|- [ ] Input con auto-resize
    87|- [ ] SSE consumer: streaming de tokens
    88|- [ ] Estilo terminal-dark (#0d1117, JetBrainsMono NF)
    89|- [ ] Sidebar de conversaciones
    90|- [ ] Slash command autocomplete en input
    91|
    92|**Milestone 1:** Chat funciona end-to-end con streaming. Multi-provider. UI terminal. Slash commands básicos.
    93|
    94|---
    95|
    96|## Fase 2 — Agents + Tools + Approval (Semanas 7-11)
    97|
    98|### Semana 7: Agent Runtime Core
    99|
   100|- [ ] `hyprcollab-agent`: integrar rig-rs
   101|- [ ] AgentSession con system prompt dinámico
   102|- [ ] Agent loop: prompt → LLM → tool call → execute → loop
   103|- [ ] Max-turns limit
   104|- [ ] Tool registry (HashMap<String, Box<dyn Tool>>)
   105|
   106|### Semana 8: Built-in Tools + Approval
   107|
   108|- [ ] `hyprcollab-tools/shell`: ejecutar comandos (sandboxed)
   109|- [ ] `hyprcollab-tools/file_ops`: leer/escribir archivos
   110|- [ ] `hyprcollab-tools/web_search`: Brave/SearXNG API
   111|- [ ] `hyprcollab-tools/web_fetch`: HTTP fetch + parse
   112|- [ ] `hyprcollab-tools/memory_tools`: store/search en memoria
   113|- [ ] `hyprcollab-approval`: ApprovalEngine completo
   114|- [ ] `hyprcollab-approval`: rule matching (tool, pattern, workspace)
   115|- [ ] API: GET/POST `/api/approval/pending`, approve, deny, modify
   116|
   117|### Semana 9: Shell Tool Enhanced + Background Processes
   118|
   119|- [ ] Shell tool con CWD tracking
   120|- [ ] Environment variables management
   121|- [ ] Background process: start, stop, status, output streaming
   122|- [ ] PTY allocation para comandos interactivos
   123|- [ ] Timeout configurable por comando
   124|- [ ] Output capture: stdout + stderr separados
   125|
   126|### Semana 10: Tool Visualization + Approval UI
   127|
   128|- [ ] Frontend: componente ToolCall (nombre, params, output, duración)
   129|- [ ] Frontend: thinking/reasoning display (colapsable)
   130|- [ ] Frontend: Approval dialog (Approve/Deny/Modify)
   131|- [ ] Backend: estructurar tool calls en SSE events
   132|- [ ] Approval log en SQLite
   133|
   134|### Semana 11: Slash Commands Agénticos
   135|
   136|- [ ] `hyprcollab-commands`: file-based loading (.md con frontmatter YAML)
   137|- [ ] `/.agent <name>`: carga agent.md → injecta system prompt + tools
   138|- [ ] `/.skill <name>`: carga skill.md → injecta procedure
   139|- [ ] `/.run <cmd>`: ejecuta con approval check
   140|- [ ] `/.browse <url>`: abre browser (placeholder)
   141|- [ ] `/.design`, `/.review`: carga .md y cambia modo
   142|- [ ] Autocompletado de comandos + argumentos
   143|
   144|**Milestone 2:** Agente autónomo con tools, approval system, slash commands completos.
   145|
   146|---
   147|
   148|## Fase 3 — Artifacts Canvas + TUI (Semanas 12-15)
   149|
   150|### Semana 12: Artifact Types + Backend
   151|
   152|- [ ] `hyprcollab-artifacts`: enum Artifact + serde
   153|- [ ] POST/GET/PUT `/api/artifacts`
   154|- [ ] Artifact creation desde agent (tool `artifact_create`)
   155|- [ ] Almacenamiento en filesystem + metadata SQLite
   156|- [ ] Inline artifacts: embebidos en mensajes del chat
   157|
   158|### Semana 13: Canvas Frontend
   159|
   160|- [ ] Panel lateral de canvas (resizable, toggleable)
   161|- [ ] Code preview: syntax highlighting (tree-sitter WASM)
   162|- [ ] Markdown preview: pulldown-cmark → HTML
   163|- [ ] SVG preview: inline render via resvg
   164|- [ ] Mermaid diagram render (mermaid.js WASM)
   165|- [ ] Artifact tabs (múltiples artifacts por conversación)
   166|
   167|### Semana 14: Canvas Advanced + Media Preview
   168|
   169|- [ ] HTML preview en iframe sandboxed
   170|- [ ] LaTeX/KaTeX math render
   171|- [ ] Edit mode: usuario edita artifact → auto-save → agente ve cambios
   172|- [ ] Diff view (cambios del agente vs versión usuario)
   173|- [ ] `hyprcollab-media-preview`: terminal capability detection
   174|- [ ] `hyprcollab-media-preview`: kitty graphics protocol encoder
   175|- [ ] `hyprcollab-media-preview`: sixel fallback encoder
   176|- [ ] Image preview: PNG/JPG/WebP
   177|- [ ] Code preview: syntect syntax highlight
   178|- [ ] SVG preview: resvg → PNG → kitty protocol
   179|
   180|### Semana 15: TUI Mode + WebSocket
   181|
   182|- [ ] `hyprcollab-tui`: ratatui setup + crossterm backend
   183|- [ ] TUI layout: chat pane + artifact pane
   184|- [ ] Kitty image protocol integration en TUI
   185|- [ ] Vim-like keybinds (j/k, gg/G, i, :commands)
   186|- [ ] WS `/ws/artifacts/:id` para live updates
   187|- [ ] PDF preview (primera página renderizada)
   188|- [ ] Markdown preview en TUI
   189|
   190|**Milestone 3:** Canvas de artifacts con inline render. TUI funcional con kitty image preview.
   191|
   192|---
   193|
   194|## Fase 4 — RAG Pipeline + Browser Engine (Semanas 16-19)
   195|
   196|### Semana 16: Document Parsers
   197|
   198|- [ ] `hyprcollab-rag/parser`: PDF (pdf-rs)
   199|- [ ] `hyprcollab-rag/parser`: Markdown (pulldown-cmark)
   200|- [ ] `hyprcollab-rag/parser`: Code files (tree-sitter chunking)
   201|- [ ] `hyprcollab-rag/parser`: Plain text, CSV, JSON
   202|- [ ] `hyprcollab-rag/parser`: HTML (html5ever)
   203|- [ ] Parser tests con archivos reales
   204|
   205|### Semana 17: Chunking + Embeddings + Store
   206|
   207|- [ ] `hyprcollab-rag/chunker`: RecursiveCharacter con overlap
   208|- [ ] `hyprcollab-rag/embedder`: integración con provider embeddings
   209|- [ ] `hyprcollab-rag/store`: LanceDB conexión, upsert, search
   210|- [ ] Tabla `documents` en SQLite
   211|- [ ] POST `/api/rag/upload` → parse → chunk → embed → store
   212|
   213|### Semana 18: Browser Engine
   214|
   215|- [ ] `hyprcollab-browser`: BrowserPool + headless_chrome/chromiumoxide
   216|- [ ] `web_navigate(url)` → PageSnapshot
   217|- [ ] `web_screenshot()` → PNG bytes
   218|- [ ] `web_extract(selector)` → structured data
   219|- [ ] Cookie jar persistente
   220|- [ ] Browser config (viewport, timeout, user agent)
   221|
   222|### Semana 19: Browser + RAG Integration
   223|
   224|- [ ] `web_interact(action, target)` → click, type, scroll
   225|- [ ] `web_js_execute(code)` → sandboxed JS
   226|- [ ] RAG auto-indexing: páginas visitadas → chunks → LanceDB
   227|- [ ] Query pipeline: embed query → vector search → rerank → inject
   228|- [ ] Citación de sources en respuesta
   229|- [ ] RAG panel en frontend + workspace RAG scope
   230|
   231|**Milestone 4:** RAG funcional. Browser embebido. Auto-indexing de web a RAG.
   232|
   233|---
   234|
   235|## Fase 5 — Memory + Skills + Extra (Semanas 20-24)
   236|
   237|### Semana 20: Working Memory
   238|
   239|- [ ] `hyprcollab-memory/working_memory`: CRUD de facts
   240|- [ ] Auto-extracción de facts post-conversation (LLM call)
   241|- [ ] Categorías: preference, fact, pattern, correction
   242|- [ ] Confidence scoring
   243|- [ ] GET/POST/DELETE `/api/memory/facts`
   244|
   245|### Semana 21: Memory + Search + Themes
   246|
   247|- [ ] Inyección de working memory en system prompt
   248|- [ ] Memory search: FTS5 + semantic search híbrido
   249|- [ ] Memory pruning: cleanup de facts de baja confianza
   250|- [ ] Frontend: panel de memoria
   251|- [ ] Theme engine: CSS variables system
   252|- [ ] Themes: terminal-dark, catppuccin, nord, dracula, tokyo-night
   253|- [ ] Custom theme loading desde ~/.config/hyprcollab/themes/
   254|
   255|### Semana 22: Skills Engine + Auto-Learning
   256|
   257|- [ ] `hyprcollab-skills/loader`: parse YAML skills
   258|- [ ] `hyprcollab-skills/matcher`: trigger pattern matching
   259|- [ ] Skill injection en system prompt cuando match
   260|- [ ] `hyprcollab-skills/learner`: detectar patrones repetidos
   261|- [ ] Auto-generar skill YAML desde conversaciones
   262|- [ ] Usage tracking y success rate
   263|
   264|### Semana 23: Extra Features I
   265|
   266|- [ ] Conversation branching: fork desde cualquier mensaje
   267|- [ ] Tree structure en conversations (parent_id)
   268|- [ ] Bookmarks: marcar mensajes importantes
   269|- [ ] Pinned conversations: fijar en sidebar
   270|- [ ] FTS5 full-text search sobre conversaciones
   271|- [ ] GET `/api/conversations/search?q=`
   272|
   273|### Semana 24: Extra Features II
   274|
   275|- [ ] Token usage tracking: dashboard por modelo, día, workspace
   276|- [ ] Export: Markdown, JSON (PDF/PNG en siguiente fase)
   277|- [ ] Prompt templates: library con variable interpolation
   278|- [ ] Session persistence: reabrir donde se dejó
   279|- [ ] Code block actions: copy, download, open in editor
   280|- [ ] Drag & drop file upload
   281|
   282|**Milestone 5:** Memoria evolutiva + skills auto-aprendidas. Branching, search, export.
   283|
   284|---
   285|
   286|## Fase 6 — Desktop (Tauri) + ACP + Mobile (Semanas 25-28)
   287|
   288|### Semana 25: Tauri Desktop App
   289|
   290|- [ ] `hyprcollab-tauri`: Tauri 2.0 setup (Linux + macOS)
   291|- [ ] PlatformAdapter impl para Tauri
   292|- [ ] System tray icon
   293|- [ ] Global shortcuts (configurables)
   294|- [ ] Native notifications
   295|- [ ] Auto-update mechanism
   296|- [ ] Window state persistence
   297|
   298|### Semana 26: ACP Connector
   299|
   300|- [ ] `hyprcollab-acp/connector`: trait AcpConnector
   301|- [ ] `hyprcollab-acp/claude_code`: `claude -p --output-format stream-json`
   302|- [ ] `hyprcollab-acp/codex`: `codex --acp --stdio`
   303|- [ ] `hyprcollab-acp/agy`: `agy -p --output-format stream-json`
   304|- [ ] Process spawning + PTY handling
   305|- [ ] Output stream parsing (JSON lines)
   306|
   307|### Semana 27: ACP Visualization + Mobile
   308|
   309|- [ ] Frontend: ACP Monitor panel
   310|- [ ] Token stream display (terminal-style)
   311|- [ ] Tool call cards (diff files, run commands)
   312|- [ ] Session list + status
   313|- [ ] `hyprcollab-pwa`: PWA manifest + service worker
   314|- [ ] Touch-optimized UI components
   315|- [ ] Responsive design para tablets
   316|
   317|### Semana 28: Voice + Multi-model Compare
   318|
   319|- [ ] `hyprcollab-voice`: Whisper.cpp bindgen + cpal audio capture
   320|- [ ] Push-to-talk UI component
   321|- [ ] TTS: edge-tts integration
   322|- [ ] Multi-model compare: ParallelProvider → N modelos lado a lado
   323|- [ ] Diff view: comparar respuestas
   324|- [ ] Export PDF/PNG (con headless render)
   325|
   326|**Milestone 6:** Desktop app Tauri. ACP visualization. PWA mobile. Voice mode.
   327|
   328|---
   329|
   330|## Fase 7 — Polish + Release (Semanas 29-34)
   331|
   332|### Semana 29: MCP Client + Integración
   333|
   334|- [ ] `hyprcollab-mcp`: conectar a servidores MCP (stdio transport)
   335|- [ ] Tool discovery dinámico
   336|- [ ] Exponer MCP tools al agent runtime
   337|- [ ] `/.mcp` slash command completo
   338|- [ ] Tests con server MCP de ejemplo
   339|
   340|### Semana 30: Testing Full-Stack
   341|
   342|- [ ] Tests de integración full-stack
   343|- [ ] Tests E2E con headless chrome
   344|- [ ] Property tests (proptest) para agent loop
   345|- [ ] Snapshot tests (insta) para artifact rendering
   346|- [ ] Benchmark tests (criterion) para streaming
   347|- [ ] Fuzz testing para slash command parser
   348|
   349|### Semana 31: Documentation
   350|
   351|- [ ] API documentation (rustdoc + mdbook)
   352|- [ ] User guide: instalación, configuración, uso
   353|- [ ] Theme customization guide
   354|- [ ] Slash commands reference
   355|- [ ] Agent/Skill authoring guide
   356|- [ ] Architecture decision records (ADRs)
   357|
   358|### Semana 32: Packaging
   359|
   360|- [ ] PKGBUILD para AUR (`hyprcollab`, `hyprcollab-bin`, `hyprcollab-git`)
   361|- [ ] Docker image optimizado (multi-stage, < 100MB)
   362|- [ ] Nix flake con dev shell + package
   363|- [ ] AppImage para Linux
   364|- [ ] `.dmg` para macOS
   365|- [ ] GitHub Release workflow con GPG sign
   366|
   367|### Semana 33: Final Polish
   368|
   369|- [ ] Performance profiling + optimization
   370|- [ ] Memory leak detection (valgrind/ASAN)
   371|- [ ] Accessibility audit (keyboard nav, screen readers)
   372|- [ ] Error message review (user-friendly)
   373|- [ ] Final UI polish (animations, transitions)
   374|- [ ] README final con screenshots + demo GIF
   375|
   376|### Semana 34: Release
   377|
   378|- [ ] Tag v0.1.0
   379|- [ ] GitHub Release con notes + binaries
   380|- [ ] AUR publish
   381|- [ ] Docker Hub publish
   382|- [ ] Demo video para README
   383|- [ ] Post en Reddit (r/rust, r/LocalLLaMA, r/selfhosted, r/archlinux)
   384|
   385|**Milestone 7:** v0.1.0 released. Desktop app + web + TUI. AUR + Docker + GitHub.
   386|
   387|---
   388|
   389|## Resumen de Milestones
   390|
   391|| Hito | Semana | Feature Principal | Crates | Líneas Est. |
   392||------|--------|------------------|--------|-------------|
   393|| M0 | 2 | Scaffold + Core types | 3 | ~2,000 |
   394|| M1 | 6 | Chat streaming + UI + slash cmds | 7 | ~7,000 |
   395|| M2 | 11 | Agent + Tools + Approval | 5 | ~7,000 |
   396|| M3 | 15 | Artifacts Canvas + TUI + Kitty | 3 | ~6,000 |
   397|| M4 | 19 | RAG Pipeline + Browser Engine | 2 | ~5,000 |
   398|| M5 | 24 | Memory + Skills + Extras | 3 | ~6,000 |
   399|| M6 | 28 | Tauri + ACP + Voice + Mobile | 3 | ~5,000 |
   400|| M7 | 34 | Polish + Release | 0 | ~3,000 |
   401|| **Total** | **34** | **~26 crates** | **26** | **~41,000** |
   402|
   403|---
   404|
   405|## Estrategia de Agentes
   406|
   407|**Claude Code** para:
   408|- Crates complejos (agent runtime, memory engine, browser engine)
   409|- Code review de cada milestone
   410|- Debugging de issues de compilación Rust
   411|- Escritura de tests
   412|
   413|**Antigravity CLI (agy)** para:
   414|- Frontend Dioxus components
   415|- Tauri desktop integration
   416|- CI/CD pipeline
   417|- AUR/Docker packaging
   418|- Documentation
   419|
   420|### Workflow:
   421|
   422|1. Escribir plan de cada task
   423|2. Delegar a Claude Code via `claude -p` (backend crates)
   424|3. Delegar a agy via `agy -p` (frontend + infra)
   425|4. Review cruzado
   426|5. Integrar + test local
   427|6. Commit + push
   428|
   429|---
   430|
   431|## Riesgos y Mitigaciones por Fase
   432|
   433|- **F1:** Streaming SSE en Dioxus WASM → Usar `gloo-net` + `web-sys` directamente
   434|- **F2:** rig-rs no soporta function calling → Custom agent loop con reqwest
   435|- **F3:** Artifact rendering lento en WASM → SSR hibrido, lazy loading
   436|- **F4:** LanceDB bindings inestables → Fallback a `sqlite-vec` o Qdrant
   437|- **F5:** Headless Chrome pesado → Fallback a reqwest+scraper para scraping simple
   438|- **F6:** Tauri 2.0 mobile inmaduro → PWA como alternativa
   439|- **F7:** Kitty protocol pocas terminales → Sixel fallback, graceful degrade
   440|- **F8:** Scope creep → MVP estricto P0, features P1/P2 en fases posteriores
   441|