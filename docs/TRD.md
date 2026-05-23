     1|# HyprCollab — Technical Requirements Document
     2|
     3|> **OpenSpec SSD v2.0** · Agentic AI Chat Platform · Rust-Native · Multi-Platform
     4|
     5|---
     6|
     7|## 1. Diagramas de Arquitectura
     8|
     9|### 1.1 Arquitectura Multi-Plataforma
    10|
    11|```mermaid
    12|graph TB
    13|    subgraph "Plataformas de Entrada"
    14|        TAURI["🖥️ Tauri 2.0 Desktop<br/>Linux · macOS · Windows"]
    15|        WEB["🌐 Web Server<br/>Axum + Dioxus WASM"]
    16|        TUI["⌨️ TUI Terminal<br/>ratatui + crossterm"]
    17|        MOBILE["📱 Mobile<br/>Tauri 2.0 / PWA"]
    18|    end
    19|
    20|    subgraph "Platform Abstraction Layer"
    21|        PA["PlatformAdapter trait"]
    22|        MEDIA["MediaPreview<br/>kitty · sixel · off"]
    23|        RENDER["RenderBackend<br/>WebView · Terminal · HTML"]
    24|    end
    25|
    26|    subgraph "API Gateway (Axum)"
    27|        REST["REST endpoints"]
    28|        SSE["SSE streaming"]
    29|        WS["WebSocket"]
    30|    end
    31|
    32|    subgraph "Core Services"
    33|        AGENT["Agent Runtime<br/>rig-rs"]
    34|        LLM["LLM Router<br/>multi-provider"]
    35|        MEM["Memory Engine<br/>SQLite + FTS5"]
    36|        RAG["RAG Pipeline<br/>LanceDB"]
    37|        TOOLS["Tool System"]
    38|        SKILLS["Skills Engine"]
    39|        ACP["ACP Connector"]
    40|        CMDS["Slash Commands"]
    41|        APPROVE["Approval Engine"]
    42|        BROWSER["Browser Engine<br/>headless_chrome"]
    43|        MEDIA_P["Media Preview<br/>kitty + sixel"]
    44|    end
    45|
    46|    subgraph "Storage"
    47|        SQLITE["SQLite + FTS5"]
    48|        LANCE["LanceDB vectors"]
    49|        FS["Filesystem"]
    50|    end
    51|
    52|    TAURI --> PA
    53|    WEB --> PA
    54|    TUI --> PA
    55|    MOBILE --> PA
    56|    PA --> MEDIA
    57|    PA --> RENDER
    58|    RENDER --> REST
    59|    RENDER --> SSE
    60|    RENDER --> WS
    61|    REST --> AGENT
    62|    SSE --> AGENT
    63|    WS --> AGENT
    64|    AGENT --> LLM
    65|    AGENT --> MEM
    66|    AGENT --> RAG
    67|    AGENT --> TOOLS
    68|    AGENT --> SKILLS
    69|    AGENT --> CMDS
    70|    TOOLS --> APPROVE
    71|    TOOLS --> BROWSER
    72|    TOOLS --> MEDIA_P
    73|    AGENT --> ACP
    74|    MEM --> SQLITE
    75|    RAG --> LANCE
    76|    TOOLS --> FS
    77|```
    78|
    79|### 1.2 Flujo de Slash Commands
    80|
    81|```mermaid
    82|flowchart TD
    83|    INPUT["User input: /command args"] --> PARSER["SlashCommandParser"]
    84|    PARSER --> |"match"| DISPATCH["CommandDispatcher"]
    85|    
    86|    DISPATCH --> |"/.agent"| AGENT_CMD["Load ~/.config/hyprcollab/agents/<name>.md"]
    87|    DISPATCH --> |"/.skill"| SKILL_CMD["Load ~/.config/hyprcollab/skills/<name>.md"]
    88|    DISPATCH --> |"/.mcp"| MCP_CMD["Connect/Disconnect MCP server"]
    89|    DISPATCH --> |"/.acp"| ACP_CMD["Start ACP session"]
    90|    DISPATCH --> |"/.design"| DESIGN_CMD["Load design.md → Canvas mode"]
    91|    DISPATCH --> |"/.review"| REVIEW_CMD["Load review.md → Review mode"]
    92|    DISPATCH --> |"/.run"| RUN_CMD["System command → Approval check"]
    93|    DISPATCH --> |"/.browse"| BROWSE_CMD["Open browser session"]
    94|    DISPATCH --> |"/.model"| MODEL_CMD["Switch LLM provider/model"]
    95|    DISPATCH --> |"/.workspace"| WS_CMD["Switch workspace"]
    96|    
    97|    AGENT_CMD --> INJECT["Inject into AgentSession"]
    98|    SKILL_CMD --> INJECT
    99|    DESIGN_CMD --> INJECT
   100|    REVIEW_CMD --> INJECT
   101|    MCP_CMD --> TOOLS_REG["Register MCP tools"]
   102|    ACP_CMD --> ACP_SESS["Start ACP session"]
   103|    RUN_CMD --> APPROVAL{"Approval Engine"}
   104|    APPROVAL --> |"auto"| EXEC["Execute"]
   105|    APPROVAL --> |"manual"| UI_APPROVE["UI: Approve/Deny/Modify"]
   106|    UI_APPROVE --> |"approve"| EXEC
   107|    BROWSE_CMD --> BROWSER_SESS["BrowserSession"]
   108|    MODEL_CMD --> ROUTER["Update LLM Router"]
   109|    WS_CMD --> WS_SWITCH["Change workspace context"]
   110|```
   111|
   112|### 1.3 Browser Engine Architecture
   113|
   114|```mermaid
   115|flowchart LR
   116|    subgraph "Agent Tools"
   117|        NAV["web_navigate(url)"]
   118|        SS["web_screenshot()"]
   119|        EXT["web_extract(selector)"]
   120|        INTER["web_interact(action)"]
   121|        JS["web_js_execute(code)"]
   122|    end
   123|
   124|    subgraph "Browser Engine (hyprcollab-browser)"
   125|        POOL["BrowserPool<br/>Chrome instances"]
   126|        PAGE["Page Handle"]
   127|        COOKIES["CookieJar<br/>persistent"]
   128|        SCRIPT["JS Sandbox"]
   129|    end
   130|
   131|    subgraph "Output"
   132|        SNAP["PageSnapshot<br/>html + text + links"]
   133|        IMG["Screenshot<br/>PNG bytes"]
   134|        DATA["Extracted Data<br/>structured"]
   135|    end
   136|
   137|    NAV --> POOL --> PAGE --> SNAP
   138|    SS --> PAGE --> IMG
   139|    EXT --> PAGE --> DATA
   140|    INTER --> PAGE
   141|    JS --> SCRIPT --> PAGE
   142|    PAGE --> COOKIES
   143|    SNAP --> RAG["Auto-index to RAG"]
   144|```
   145|
   146|### 1.4 Kitty Image Protocol Flow
   147|
   148|```mermaid
   149|sequenceDiagram
   150|    participant TUI as TUI (ratatui)
   151|    participant Detect as TerminalDetector
   152|    participant Preview as MediaPreview
   153|    participant Kitty as Kitty Protocol
   154|    participant Sixel as Sixel Fallback
   155|    
   156|    TUI->>Detect: Check terminal capabilities
   157|    Detect->>Detect: Query kitty graphics protocol
   158|    Detect->>Detect: Query sixel support
   159|    Detect-->>TUI: capabilities report
   160|    
   161|    Note over TUI: Agent generates image artifact
   162|    
   163|    TUI->>Preview: preview_image(path, area)
   164|    Preview->>Preview: Detect format (PNG/JPG/SVG/PDF)
   165|    
   166|    alt Kitty protocol available
   167|        Preview->>Kitty: Encode image as kitty escape sequence
   168|        Kitty-->>TUI: Render inline in terminal
   169|    else Sixel available
   170|        Preview->>Sixel: Convert to sixel format
   171|        Sixel-->>TUI: Render inline
   172|    else No protocol
   173|        Preview-->>TUI: Show file path + dimensions as text
   174|    end
   175|```
   176|
   177|---
   178|
   179|## 2. Componentes
   180|
   181|### 2.1 LLM Router
   182|
   183|Gateway multi-provider. Unifica OpenAI, Anthropic, Ollama, OpenRouter bajo un solo trait.
   184|
   185|```rust
   186|#[async_trait]
   187|pub trait LlmProvider: Send + Sync {
   188|    async fn chat_completion(&self, req: ChatRequest) -> Result<ChatResponse>;
   189|    async fn chat_stream(&self, req: ChatRequest) -> Result<Pin<Box<dyn Stream<Item = Result<TokenChunk>> + Send>>>;
   190|    async fn embeddings(&self, text: &str) -> Result<Vec<f32>>;
   191|    fn models(&self) -> Vec<ModelInfo>;
   192|    fn name(&self) -> &str;
   193|    fn supports_streaming(&self) -> bool;
   194|    fn supports_tools(&self) -> bool;
   195|}
   196|
   197|pub struct LlmRouter {
   198|    providers: HashMap<String, Box<dyn LlmProvider>>,
   199|    default: String,
   200|    fallback: Vec<String>,
   201|}
   202|```
   203|
   204|Providers como crates separados:
   205|- `hyprcollab-provider-openai` — OpenAI + OpenAI-compatible
   206|- `hyprcollab-provider-anthropic` — Claude directo
   207|- `hyprcollab-provider-ollama` — Modelos locales
   208|- `hyprcollab-provider-openrouter` — Aggregator
   209|
   210|### 2.2 Agent Runtime (rig-rs)
   211|
   212|```rust
   213|pub struct AgentSession {
   214|    agent: Agent<LlmProvider>,
   215|    tools: ToolRegistry,
   216|    memory: SessionMemory,
   217|    skills: SkillMatcher,
   218|    slash_commands: CommandDispatcher,
   219|    max_turns: u32,
   220|    workspace: WorkspaceId,
   221|    approval_engine: ApprovalEngine,
   222|    current_cwd: PathBuf,
   223|    env_vars: HashMap<String, String>,
   224|}
   225|```
   226|
   227|**Memoria evolutiva (inspirado en Hermes):**
   228|- `ShortTermMemory` — Context window de la conversación actual
   229|- `WorkingMemory` — Hechos extraídos entre sesiones
   230|- `SkillMemory` — Skills aprendidas de patrones de uso
   231|- `PreferenceMemory` — Preferencias inferidas del usuario
   232|
   233|### 2.3 Platform Adapter
   234|
   235|Trait que abstrae la plataforma de ejecución:
   236|
   237|```rust
   238|pub trait PlatformAdapter: Send + Sync {
   239|    fn platform_name(&self) -> &str;
   240|    fn supports_media_preview(&self) -> bool;
   241|    fn supports_browser(&self) -> bool;
   242|    fn supports_notifications(&self) -> bool;
   243|    fn open_external(&self, url: &str) -> Result<()>;
   244|    fn show_approval_dialog(&self, request: &ApprovalRequest) -> ApprovalFuture;
   245|}
   246|
   247|// Implementaciones:
   248|pub struct TauriAdapter;      // Desktop (Tauri 2.0)
   249|pub struct WebAdapter;         // Web browser (WASM)
   250|pub struct TuiAdapter;         // Terminal (ratatui)
   251|pub struct MobileAdapter;      // iOS/Android (Tauri mobile)
   252|```
   253|
   254|### 2.4 RAG Engine
   255|
   256|```
   257|Upload → Parse → Chunk → Embed → Store (LanceDB)
   258|Query  → Embed → Vector Search → Rerank → Inject Context
   259|```
   260|
   261|**Parsers soportados:**
   262|- PDF (pdf-rs)
   263|- Markdown (pulldown-cmark)
   264|- Code files (tree-sitter para AST-aware chunking)
   265|- Plain text / CSV / JSON
   266|- HTML (html5ever)
   267|- Web pages (via Browser Engine)
   268|
   269|### 2.5 Artifacts Canvas
   270|
   271|**Tipos soportados:**
   272|- **Code:** Syntax highlight via tree-sitter + CSS classes
   273|- **Markdown:** Rich render via pulldown-cmark → HTML
   274|- **SVG:** Inline render via resvg
   275|- **React/JSX:** Sandboxed preview en iframe
   276|- **Mermaid:** Diagram render via mermaid.js WASM
   277|- **LaTeX:** Math render via katex WASM
   278|- **CSV/Table:** Sortable table component
   279|- **HTML:** Sandboxed iframe preview
   280|- **Images:** Direct render (PNG, JPG, WebP)
   281|- **PDF:** Embedded viewer
   282|
   283|```rust
   284|pub enum Artifact {
   285|    Code { lang: String, content: String },
   286|    Markdown(String),
   287|    Svg(String),
   288|    React { code: String, dependencies: HashMap<String, String> },
   289|    Mermaid(String),
   290|    Html(String),
   291|    Image { path: PathBuf, data: Vec<u8> },
   292|    Pdf { path: PathBuf },
   293|    Table { headers: Vec<String>, rows: Vec<Vec<String>> },
   294|}
   295|```
   296|
   297|### 2.6 ACP Connector
   298|
   299|```rust
   300|#[async_trait]
   301|pub trait AcpConnector: Send + Sync {
   302|    fn name(&self) -> &str;
   303|    async fn start_session(&self, config: AcpConfig) -> Result<AcpSession>;
   304|    async fn send_prompt(&self, session: &AcpSession, prompt: &str) -> Result<()>;
   305|    fn stream_output(&self, session: &AcpSession) -> Pin<Box<dyn Stream<Item = AcpEvent> + Send>>;
   306|}
   307|
   308|pub enum AcpEvent {
   309|    Token(String),
   310|    ToolCall { name: String, input: Value, output: Value },
   311|    FileChange { path: String, diff: String },
   312|    Thinking(String),
   313|    Error(String),
   314|    Done,
   315|}
   316|```
   317|
   318|**Connectores:** claude-code, codex, agy
   319|
   320|### 2.7 Slash Commands Engine
   321|
   322|Sistema extensible de comandos con parser y dispatcher:
   323|
   324|```rust
   325|#[async_trait]
   326|pub trait SlashCommand: Send + Sync {
   327|    fn name(&self) -> &str;
   328|    fn aliases(&self) -> Vec<&str>;
   329|    fn description(&self) -> &str;
   330|    fn completions(&self, partial: &str) -> Vec<String>;
   331|    async fn execute(&self, args: &str, ctx: &CommandContext) -> Result<CommandResult>;
   332|}
   333|
   334|pub struct CommandDispatcher {
   335|    commands: HashMap<String, Box<dyn SlashCommand>>,
   336|    agents_dir: PathBuf,    // ~/.config/hyprcollab/agents/
   337|    skills_dir: PathBuf,    // ~/.config/hyprcollab/skills/
   338|    designs_dir: PathBuf,   // ~/.config/hyprcollab/designs/
   339|    reviews_dir: PathBuf,   // ~/.config/hyprcollab/reviews/
   340|}
   341|
   342|pub struct CommandContext {
   343|    pub session: Arc<Mutex<AgentSession>>,
   344|    pub workspace: WorkspaceId,
   345|    pub platform: Arc<dyn PlatformAdapter>,
   346|    pub renderer: Arc<dyn RenderBackend>,
   347|}
   348|```
   349|
   350|**Archivo .md de agente (ejemplo):**
   351|```markdown
   352|---
   353|name: security-auditor
   354|model: claude-sonnet-4
   355|tools: [file_read, web_search, shell]
   356|approval: strict
   357|max_turns: 50
   358|system_prompt: |
   359|  You are a senior security auditor...
   360|---
   361|
   362|# Security Auditor Agent
   363|
   364|This agent specializes in code security review...
   365|```
   366|
   367|### 2.8 Browser Engine
   368|
   369|```rust
   370|pub struct BrowserEngine {
   371|    pool: BrowserPool,
   372|    cookie_jar: CookieJar,
   373|    config: BrowserConfig,
   374|}
   375|
   376|pub struct PageSnapshot {
   377|    pub url: String,
   378|    pub title: String,
   379|    pub html: String,
   380|    pub text: String,
   381|    pub links: Vec<LinkInfo>,
   382|    pub screenshot: Option<Vec<u8>>,
   383|}
   384|
   385|pub struct BrowserConfig {
   386|    pub headless: bool,
   387|    pub viewport: (u32, u32),
   388|    pub user_agent: Option<String>,
   389|    pub timeout: Duration,
   390|    pub auto_index: bool,  // Auto-index to RAG
   391|}
   392|
   393|impl BrowserEngine {
   394|    pub async fn navigate(&self, url: &str) -> Result<PageSnapshot>;
   395|    pub async fn screenshot(&self) -> Result<Vec<u8>>;
   396|    pub async fn extract(&self, selector: &str) -> Result<Vec<String>>;
   397|    pub async fn interact(&self, action: Interaction) -> Result<()>;
   398|    pub async fn execute_js(&self, code: &str) -> Result<Value>;
   399|}
   400|```
   401|
   402|### 2.9 Media Preview (TUI Kitty Protocol)
   403|
   404|```rust
   405|pub struct MediaPreview {
   406|    protocol: PreviewProtocol,
   407|    terminal_size: (u16, u16),
   408|}
   409|
   410|pub enum PreviewProtocol {
   411|    Kitty(KittyGraphics),
   412|    Sixel(SixelEncoder),
   413|    None,
   414|}
   415|
   416|impl MediaPreview {
   417|    pub fn detect() -> Self;
   418|    pub fn preview_image(&self, path: &Path, area: Rect) -> Result<()>;
   419|    pub fn preview_code(&self, code: &str, lang: &str, area: Rect) -> Result<()>;
   420|    pub fn preview_pdf_page(&self, path: &Path, page: usize, area: Rect) -> Result<()>;
   421|    pub fn preview_svg(&self, svg: &str, area: Rect) -> Result<()>;
   422|    pub fn preview_markdown(&self, md: &str, area: Rect) -> Result<()>;
   423|    pub fn clear(&self, area: Rect) -> Result<()>;
   424|}
   425|```
   426|
   427|### 2.10 Approval Engine
   428|
   429|```rust
   430|pub struct ApprovalEngine {
   431|    rules: Vec<ApprovalRule>,
   432|    log: Arc<ApprovalLog>,
   433|    timeout: Duration,
   434|}
   435|
   436|pub struct ApprovalRule {
   437|    pub tool_name: Option<String>,
   438|    pub pattern: Option<Regex>,
   439|    pub auto_approve: bool,
   440|    pub workspace: Option<String>,
   441|}
   442|
   443|pub enum ApprovalVerdict {
   444|    Approved,
   445|    Denied { reason: String },
   446|    Modified { new_command: String },
   447|}
   448|
   449|pub struct ApprovalRequest {
   450|    pub id: Uuid,
   451|    pub tool: String,
   452|    pub action: String,
   453|    pub args: Value,
   454|    pub risk_level: RiskLevel,
   455|    pub context: String,
   456|    pub timestamp: DateTime<Utc>,
   457|}
   458|
   459|pub enum RiskLevel {
   460|    Low,      // read-only operations
   461|    Medium,   // network requests, web interactions
   462|    High,     // file writes, shell commands
   463|    Critical, // destructive commands (rm, format, etc.)
   464|}
   465|```
   466|
   467|### 2.11 Shell Tool (Enhanced)
   468|
   469|```rust
   470|pub struct ShellTool {
   471|    cwd: Arc<Mutex<PathBuf>>,
   472|    env: Arc<Mutex<HashMap<String, String>>>,
   473|    background_processes: Arc<Mutex<HashMap<Uuid, BackgroundProcess>>>,
   474|    pty_alloc: Option<PtyAllocator>,
   475|    approval: Arc<ApprovalEngine>,
   476|}
   477|
   478|pub struct BackgroundProcess {
   479|    id: Uuid,
   480|    command: String,
   481|    pid: u32,
   482|    started_at: DateTime<Utc>,
   483|    status: ProcessStatus,
   484|}
   485|
   486|pub enum ProcessStatus {
   487|    Running,
   488|    Stopped,
   489|    Exited(i32),
   490|}
   491|```
   492|
   493|---
   494|
   495|## 3. Schema SQL
   496|
   497|```sql
   498|-- Conversaciones (tree structure para branching)
   499|CREATE TABLE conversations (
   500|    id TEXT PRIMARY KEY,
   501|