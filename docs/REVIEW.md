# HyprCollab — Code Review: Fase 0, 1, 2

> Reviewed: 2026-05-24 | Branch: `feat/cc-backend` | Reviewer: Claude Sonnet 4.6

---

## Executive Summary

The project has a solid **Fase 0** foundation and a well-implemented **Fase 1** LLM layer. The **Fase 2** components are architecturally sound but critically incomplete: the Axum server is disconnected from all core services (no real LLM calls, no memory, no approval wiring), and every slash command is a stateless stub. Eleven issues are Fase-3 blockers. The code compiles and tests pass, but the system cannot actually serve AI responses.

---

## Per-Crate Assessment

### `hyprcollab-core` ✅ Good

**What's good:**
- `id_newtype!` macro is elegant and covers UUID-based IDs correctly.
- `CoreError` is well-structured with `thiserror` derive.
- `LlmProvider` trait is correct: `chat_stream` properly returns `Pin<Box<dyn Stream<...> + Send>>` rather than an `async fn`.
- `ToolDefinition` as a JSON Schema struct matches what OpenAI/Anthropic APIs expect.

**Issues:**

1. **Trait diverges from TRD spec** (`traits.rs:107–121`):
   - TRD `SlashCommand` requires `fn aliases(&self) -> Vec<&str>` and `fn completions(&self, partial: &str) -> Vec<String>`. Neither is in the trait. All builtin commands are uncompleted without these.
   - TRD `AcpConnector` requires `fn stream_output(&self, session: &AcpSession) -> Pin<Box<dyn Stream<Item = AcpEvent>>>`. Current trait has `send_request` (JSON-RPC style), which is a completely different abstraction.
   - TRD `PlatformAdapter` requires `fn supports_media_preview`, `fn supports_browser`, `fn open_external`, `fn show_approval_dialog`. Current trait has `send_message`, `show_notification`, `clipboard_write` — a different surface.

2. **`CoreError` missing `reqwest` conversion** (`errors.rs`): Every provider manually wraps `reqwest::Error` into `CoreError::Llm(format!(...))`. A `From<reqwest::Error>` impl would remove ~20 lines of boilerplate.

3. **`CoreError` missing `anyhow` bridge**: `hyprcollab-config` returns `anyhow::Result` but the rest of the stack uses `CoreError`. There is no automatic conversion, meaning config errors cannot propagate cleanly through server handlers.

4. **No `CommandContext` type**: The TRD defines `CommandContext { session, workspace, platform, renderer }` that should be passed to every command `execute`. Without it, builtin commands can never access sessions or workspace state.

---

### `hyprcollab-memory` ✅ Solid but has critical bugs

**What's good:**
- WAL mode + foreign keys pragma setup is correct.
- Migration system with `schema_version` tracking is idiomatic.
- `get_recent` correctly reverses the descending order. 
- `message_from_row` properly handles all JSON deserialization edge cases.
- Tests cover CRUD, cascade, and upsert.

**Critical bugs:**

5. **Transaction bug in `migrations.rs:38-51`** — the `tx` object is created from `conn.unchecked_transaction()` but the actual SQL is executed on `conn` directly, not through `tx`:
   ```rust
   let tx = conn.unchecked_transaction()...;
   conn.execute_batch(sql)...;  // runs outside the tx!
   // ...
   tx.commit()...;  // commits nothing from the migration
   ```
   The `commit()` is effectively a no-op; if the `INSERT INTO schema_version` succeeds but `execute_batch(sql)` was already committed (autocommit), a crash between them leaves a corrupt state. Fix: use `tx.execute_batch(sql)` and `tx.execute(...)` throughout.

6. **`unwrap()` on UUID parse in `ChatRecord::from_row`** (`store.rs:313–316`):
   ```rust
   workspace_id: ws.as_deref()
       .map(|s| WorkspaceId(uuid::Uuid::parse_str(s).unwrap()))
   ```
   A malformed UUID in the DB will **panic** the thread. Same pattern for `persona_id` and `agent_role_id`. Replace with `?`-propagation.

7. **`MemoryStore` is `!Send`** (`store.rs:16`): `rusqlite::Connection` is `!Send`. `MemoryStore` cannot be shared across Axum handlers or passed to `tokio::spawn`. For the server to use `MemoryStore`, it must be wrapped in `Arc<tokio::sync::Mutex<MemoryStore>>` or use connection pooling (`r2d2-sqlite`/`sqlx`).

**Missing:**
- No `update_chat_title` method.
- No `full_text_search` on messages (FTS5 tables are not created).
- `approval_log` table referenced in PRD approval flow doesn't exist.

---

### `hyprcollab-config` ⚠️ Functional but inconsistent

**What's good:**
- The 4-layer priority chain (`global < folder < agent < chat`) is correctly implemented.
- `interpolate_env_vars` is a clean hand-rolled scanner with proper unterminated-brace handling.
- Tests cover all priority layer combinations.

**Issues:**

8. **Error type inconsistency** (`global.rs:5`): Uses `anyhow::Result` instead of `CoreError`. This means all config errors must be manually converted before reaching any API handler. The entire crate should use `CoreError::Config(...)`.

9. **`interpolate_env_vars` has dead `_input` parameter** (`global.rs:227`): The function signature is `fn interpolate_env_vars(input: &str) -> String` but internally calls `re.find_all(&input, &mut result)` where `find_all` takes `_input: &str` and ignores it (uses `output.clone()` instead). This is misleading dead code.

10. **`FolderConfig::detect()` is not implemented** (`folder.rs`): PLAN.md Fase 1 Week 4 calls for walking up from CWD to find `.hyprcollab/config.yaml`. No auto-detection logic exists. `ConfigResolver::load()` only loads global config.

---

### `hyprcollab-approval` ✅ Good design, not wired up

**What's good:**
- Rule matching order (first match wins) with `Allow`/`Deny`/`Ask` actions is correct.
- Regex patterns compile on each call (acceptable for now; memoize for Fase 3).
- Decision caching is keyed by `tool_name` only (not args) — appropriate simplification for now.
- Test coverage is complete for all rule types.

**Issues:**

11. **`ApprovalEngine` not connected to `agent_loop.rs`** (`engine.rs`): `AgentConfig.approval_mode` is stored but never consulted in the loop. Tools are called unconditionally. This is the core functionality gap for Fase 2.

12. **`ApprovalAction::Deny` is misleadingly named** (`rules.rs:22`): The docstring says "Always ask the user for confirmation" but the name is `Deny`. This should be `Require` or `Ask` (which already exists). `Deny` implies blocking without user interaction.

---

### `hyprcollab-router` ⚠️ Routing logic has dead code

**What's good:**
- `provider/model` format splitting is correct.
- Alias resolution is clean.
- `LlmProvider` impl on `LlmRouter` itself is a good abstraction layer.
- Tests cover all routing scenarios.

**Critical issue:**

13. **Dead loop in `resolve()` does nothing** (`router.rs:66–73`):
   ```rust
   for (_name, provider) in &self.providers {
       let provider_prefix = provider.name();
       if resolved.starts_with(provider_prefix) {
           continue; // ← This skips, but never returns anything
       }
   }
   ```
   This loop was probably meant to find a provider by name-prefix match but only has a `continue` branch — it never returns. The intended fallback to default provider below works, so routing is functional, but this loop is dead code that adds confusion.

14. **`chat_stream` in router ignores errors on provider-not-found** (`router.rs:140–155`): When `resolve()` fails, a single `Err(e)` is emitted via `stream::once`. The consumer will receive one error chunk and the stream ends. This is acceptable but asymmetric with `chat_completion` which returns `Result<ChatResponse>`.

---

### `hyprcollab-provider-openai` ✅ Well-implemented

**What's good:**
- Type conversions `from_core`/`into_core` are correct and complete.
- Tool call serialization matches OpenAI's `{type: "function", function: {...}}` format.
- SSE stream uses `eventsource-stream` crate — correct approach.
- `[DONE]` terminator handling is correct.
- mockito tests verify auth headers and error codes.

**Issues:**

15. **`OpenAiResponse::into_core` panics on empty choices** (`types.rs:188`):
   ```rust
   let choice = self.choices.into_iter().next()
       .expect("OpenAI response must have at least one choice");
   ```
   The OpenAI API can return empty `choices` for moderated content. This should return `Err(CoreError::Llm(...))`.

16. **`context_length` hardcoded to `128_000` for all models** (`client.rs:173`): Models like `gpt-3.5-turbo` have 16K context. This will mislead the router if context-length-based routing is ever added.

17. **No streaming tool-call reassembly** (`streaming.rs`): OpenAI streams tool calls as incremental JSON fragments across chunks. The current streaming implementation only emits `delta.content` text and discards `delta.tool_calls`. Tool calls will be missing from streamed responses.

---

### `hyprcollab-provider-anthropic` ✅ Well-implemented

**What's good:**
- System message extraction to top-level `system` field is correct.
- `AnthropicMessage::from_core` correctly handles tool-result messages as `user`-role content blocks.
- Stream event handling covers all Anthropic event types (`message_start`, `content_block_delta`, `message_delta`, `message_stop`).
- Input token count is 0 in `message_delta` usage (correct — input tokens come in `message_start`).

**Issues:**

18. **`message_start` event drops input token count** (`streaming.rs:81–84`): Input tokens are available in `message_start.message.usage.input_tokens` but the handler `continue`s without emitting them. The final `TokenUsage` will have `prompt_tokens: 0`. Fix: parse `message_start` and store input tokens for the final `message_delta` chunk.

19. **`AnthropicContentBlock::ToolResult` in `into_core` is silently ignored** (`types.rs:226`):
   ```rust
   _ => {}
   ```
   If a response contains `ToolResult` blocks, they're dropped. This is unlikely in practice (responses don't contain tool results) but is an unhandled match arm.

---

### `hyprcollab-provider-ollama` ⚠️ Functional but limited

**What's good:**
- OpenAI-compatible `/v1/chat/completions` endpoint is correct.
- Embeddings uses Ollama's native `/api/embeddings` endpoint.
- Model discovery via `/api/tags` is correct.
- mockito tests cover success and error paths.

**Issues:**

20. **No tool call support in `chat_completion`** (`client.rs:63`): The request body omits `tools` even when `request.tools` is non-empty. This silently drops tool definitions — the model won't know about tools.

21. **`null` temperature is serialized** (`client.rs:68–72`): `"temperature": request.temperature` serializes `null` when `None`, which some Ollama versions interpret as 0.0. Use `skip_serializing_if = "Option::is_none"`.

22. **SSE streaming doesn't handle Ollama's `done: true` in non-OpenAI mode** (`client.rs:184`): The `[DONE]` check is for OpenAI-compatible format. Ollama's native streaming sends `{"done":true}` in the JSON body. If Ollama's `/v1/` endpoints are not fully OpenAI-compatible, streaming will hang.

---

### `hyprcollab-agent` ✅ Best-implemented crate

**What's good:**
- `AgentSession::run()` correctly prepends system prompt, carries conversation history, and persists updated history.
- `agent_loop.rs` handles `FinishReason::ToolCalls`, multiple tool calls per turn, and max-turn budget correctly.
- `TurnToolCall` records timing (`duration_ms`) for every tool invocation.
- Mock-based tests are thorough: single call, tool→stop, multi-tool, max-turns exceeded.

**Issues:**

23. **`ApprovalMode` in `AgentConfig` is never consulted** (`agent_loop.rs:17–130`): Tools are executed unconditionally. The `config.approval_mode` field is there but never read. Before tool execution, the loop should call `ApprovalEngine::needs_approval(tc.name, tc.arguments)`.

24. **`AgentSession::run` prepends system prompt on every call** (`session.rs:80–94`): The system message is re-inserted each time `run()` is called. If the session is run multiple times (multi-turn conversation), the system prompt will appear multiple times in `self.messages`. Fix: only add if `self.messages` is empty, or filter existing system messages.

25. **No workspace / CWD tracking in session** (`session.rs`): TRD `AgentSession` spec includes `current_cwd: PathBuf` and `env_vars: HashMap<String, String>`. These are missing. `EnhancedShellTool` has its own CWD tracking, but it's disconnected from the session.

---

### `hyprcollab-tools` ✅ Good tools, some security issues

**What's good:**
- `ShellTool` uses `tokio::process::Command` with timeout correctly.
- `EnhancedShellTool` background process management with separate stdout/stderr draining is solid.
- `FileOpsTool` sandbox via `canonicalize()` is a reasonable approach.
- `WebFetchTool.strip_html` strips `<script>` blocks before tag removal (correct order).
- `WebSearchTool` Brave vs SearXNG fallback logic is correct.

**Issues:**

26. **`FileOpsTool::validate_path` breaks for `Write` to new files** (`file_ops.rs:36–50`): `canonicalize()` requires the path to exist. Writing to a new file will fail the sandbox check with `"Invalid path: No such file or directory"` before any data is written. Fix: for write operations, canonicalize only the parent directory and check that instead.

27. **`FileOpsTool::validate_path` breaks for `Exists`** (`file_ops.rs`): Same issue — `validate_path` for `Exists` check will error for non-existent paths, even though checking existence of non-existent files is a valid use case within the sandbox.

28. **`WebFetchTool::strip_html` corrupts multi-byte UTF-8** (`web_fetch.rs:89–90`):
   ```rust
   result.push(bytes[i] as char);
   ```
   `bytes[i]` is a single byte. For any non-ASCII character (é, ñ, 中, etc.) this produces garbage or panics. The function should work on `chars()`, not raw bytes, or use a proper HTML parsing library.

29. **`MemoryTools::FactStore` ID generation is not unique under concurrency** (`memory_tools.rs:78`):
   ```rust
   let id = format!("fact-{}", facts.len() + 1);
   ```
   Two concurrent `store()` calls on the same `FactStore` could produce the same ID. Use `uuid::Uuid::new_v4()`.

30. **`EnhancedShellTool` holds `bg_processes` async-lock while acquiring sync-lock** (`shell_enhanced.rs:391`): In `Output` action, `bg_processes.lock().await` is held while calling `bp.stdout_buffer.lock().unwrap()`. If the stdout-draining tokio task holds `stdout_buffer` and is waiting on the async lock, this deadlocks.

---

### `hyprcollab-commands` ⚠️ Parser is good, builtins are all stubs

**What's good:**
- `ParsedCommand::parse` handles quoted strings, long flags (`--key=value`), and short flags correctly.
- `CommandRegistry::execute` has correct parse → look up → dispatch flow.
- Tests for the parser and registry are comprehensive.

**Issues:**

31. **Every builtin command is a stateless stub** (`builtins/`): None of the 10 builtins actually do anything:
    - `/agent load <name>` returns a string but doesn't read a `.md` file or update any session.
    - `/run <cmd>` returns `"✅ Approved and executed (placeholder)"` without executing anything.
    - `/temperature 0.7` returns `"Temperature set to 0.7"` but stores nothing.
    - `/approval strict` returns a string but doesn't update `ApprovalEngine`.
    - `/browse <url>` returns `"Browser session opened for <url>"` but opens nothing.
    
    These are acceptable as scaffolding for Fase 2 completion, but the whole commands system needs real `CommandContext` wiring before Fase 3.

32. **`SlashCommand` trait has no `CommandContext` parameter in `execute`** (`traits.rs:118`): The current signature is `async fn execute(&self, args: serde_json::Value) -> Result<String>`. To do anything real (load agent, modify session, toggle approval), commands need access to shared state. This is a core trait design gap.

---

### `hyprcollab-server` 🔴 Critically incomplete

**What's good:**
- `AppError` → `IntoResponse` with structured JSON errors is correct axum usage.
- `CorsLayer` configuration (`allow_origin(Any)`, `allow_methods`, `allow_headers(Any)`) is correct.
- Tests use `tower::ServiceExt::oneshot` properly.
- SSE response via `Sse::new(stream::iter(...))` is correct axum pattern.

**Critical issues:**

33. **Server is completely disconnected from all core services** (`state.rs`, `app.rs`): `AppState` contains only `version: String` and `approvals: Arc<Mutex<Vec<String>>>`. There is no `LlmRouter`, no `MemoryStore`, no `AgentSession` pool. The `chat_handler` returns `format!("Response to: {}", payload.message)` — a hardcoded string, not an LLM response. This is the #1 blocker for Fase 3.

34. **Server `ChatRequest` duplicates and contradicts core `ChatRequest`** (`app.rs:16–22`): The server defines its own `ChatRequest { message: String, model: String, ... }` while `hyprcollab-core` has `ChatRequest { messages: Vec<Message>, model: String, tools: ..., stream: bool, ... }`. The server needs to convert incoming HTTP payloads into core types. No such conversion exists.

35. **Streaming SSE collects all chunks eagerly** (`app.rs:105–122`): The "streaming" response builds a `Vec<Event>` synchronously before returning. This is not true streaming — when real LLM integration is added, this must use a channel-backed stream (same pattern as provider streaming).

36. **Approval endpoint logic is a toy** (`app.rs:151`):
   ```rust
   let approved = !payload.tool_name.starts_with("unsafe");
   ```
   Not connected to `ApprovalEngine`. The `approvals` state records only tool names, losing args, context, and timestamps.

37. **`std::sync::Mutex` in async handler** (`state.rs:9`): `approvals: Arc<Mutex<Vec<String>>>` uses `std::sync::Mutex`. The lock is acquired in `approve_handler` which is an async function. While the current code is technically safe (lock is released before any `.await`), a panic in the handler would permanently poison the mutex, making all future requests fail with `Internal("Lock poisoned")`. Use `tokio::sync::Mutex` or wrap in `parking_lot::Mutex`.

38. **No conversation CRUD endpoints**: PLAN.md Fase 1 Week 5 requires `GET /api/conversations`, `DELETE /api/conversations`, `GET /api/conversations/:id`. None exist.

39. **No authentication / authorization**: PLAN.md specifies JWT + OAuth2 for Fase 7, but for a localhost server, at minimum there should be a note about this being intentionally deferred.

---

## Critical Issues (Fase 3 Blockers)

| # | Issue | File | Severity |
|---|-------|------|----------|
| B1 | Server returns hardcoded response, not LLM output | `server/src/app.rs:96` | 🔴 BLOCKER |
| B2 | `AppState` has no `LlmRouter`, `MemoryStore`, `AgentSession` | `server/src/state.rs` | 🔴 BLOCKER |
| B3 | `ApprovalEngine` not wired into `agent_loop.rs` | `agent/src/agent_loop.rs` | 🔴 BLOCKER |
| B4 | Transaction bug: SQL runs outside transaction in migrations | `memory/src/migrations.rs:40` | 🔴 BLOCKER |
| B5 | `MemoryStore` is `!Send`, unusable in async server | `memory/src/store.rs:16` | 🔴 BLOCKER |
| B6 | `validate_path` panics on `canonicalize` for new files | `tools/src/file_ops.rs:36` | 🔴 BLOCKER |
| B7 | Slash commands have no `CommandContext`, all are stubs | `commands/src/builtins/` | 🔴 BLOCKER |
| B8 | `unwrap()` on UUID parse can panic server | `memory/src/store.rs:313` | 🔴 BLOCKER |
| B9 | `strip_html` corrupts multi-byte characters | `tools/src/web_fetch.rs:89` | 🔴 BLOCKER |
| B10 | OpenAI streaming drops all tool calls | `provider-openai/src/streaming.rs` | 🔴 BLOCKER |
| B11 | `AgentSession::run` double-inserts system prompt on multi-turn | `agent/src/session.rs:80` | 🟠 HIGH |

---

## API Misuse (Context7-verified)

1. **axum `Sse` is used correctly** — `Sse::new(stream)` with `Stream<Item = Result<Event, Infallible>>` matches the axum v0.8 API.

2. **`tower_http::cors::CorsLayer`** — `allow_origin(Any)` with `allow_headers(Any)` disables the `Access-Control-Allow-Credentials` restriction (can't use `credentials: true` on the client with `Any` origin). This is acceptable for a local dev server but **must not be deployed to production** without narrowing `allow_origin`.

3. **`tokio::process::Command`** is used correctly in `ShellTool` and `EnhancedShellTool`. The `stdout(Stdio::piped())` pattern before `output()` is correct.

4. **`tokio::sync::Mutex` vs `std::sync::Mutex`**: `EnhancedShellTool` correctly uses `tokio::sync::Mutex` for `bg_processes` (held across `.await`) and `std::sync::Mutex` for `cwd`/`env_vars` (never held across `.await`). However, `AppState` uses `std::sync::Mutex` in an async handler — borderline but a footgun.

5. **`axum::State`** extractors are correct: `State(state): State<AppState>` is the v0.8 syntax.

6. **`serde` derive macros** — `#[serde(tag = "action", rename_all = "snake_case")]` on `ShellAction` and `FileParams` is the correct adjacently-tagged enum approach.

---

## Architecture Gaps Before Fase 3

### 1. Dependency Injection into AppState
```rust
// What AppState NEEDS:
pub struct AppState {
    pub version: String,
    pub router: Arc<LlmRouter>,           // for real LLM calls
    pub db: Arc<Mutex<MemoryStore>>,      // for conversation persistence  
    pub approval: Arc<Mutex<ApprovalEngine>>, // for tool approval
    pub sessions: Arc<Mutex<HashMap<String, AgentSession>>>, // per-chat sessions
}
```

### 2. CommandContext for Builtin Commands
```rust
pub struct CommandContext {
    pub session: Arc<Mutex<AgentSession>>,
    pub workspace_id: WorkspaceId,
    pub db: Arc<Mutex<MemoryStore>>,
    pub approval: Arc<Mutex<ApprovalEngine>>,
}
```
All `SlashCommand::execute` signatures must accept `&CommandContext`.

### 3. CoreError Unified Error Strategy
- `hyprcollab-config` must switch from `anyhow` to `CoreError::Config`.
- Add `From<reqwest::Error>`, `From<serde_yaml::Error>` to `CoreError`.

### 4. `MemoryStore` Must Be Connection-Pooled
Either use `r2d2` + `r2d2-sqlite` or migrate to `sqlx` with a `SqlitePool`. The current single-connection design blocks concurrent writes.

### 5. Anthropic Streaming Input Token Fix
Store `input_tokens` from `message_start` and emit them in the final `TokenUsage` on `message_delta`.

---

## Recommendations (Priority Order)

### Immediate (must fix before any Fase 3 work)

1. **Fix migration transaction bug** (`migrations.rs:40`): Change `conn.execute_batch(sql)` → `tx.execute_batch(sql)` and `conn.execute(...)` → `tx.execute(...)`.

2. **Replace `MemoryStore` single connection** with `r2d2-sqlite` or `rusqlite::Connection` behind `Arc<Mutex<>>` that is `Send`. This unblocks server state sharing.

3. **Wire `AppState` to real services**: Add `LlmRouter`, `MemoryStore`, `ApprovalEngine` to `AppState`. Make `create_app` accept these as parameters.

4. **Connect `chat_handler` to `AgentSession`**: Create or retrieve an `AgentSession` per `chat_id`, call `session.run(message)`, stream the response as SSE.

5. **Fix `file_ops.rs::validate_path` for writes**: For `Write` action, canonicalize `Path::new(path).parent()` instead of the full path.

6. **Fix `strip_html` multi-byte handling**: Process `chars()` instead of raw bytes, or add `scraper` crate.

7. **Replace UUID `unwrap()` in `ChatRecord::from_row`**: Propagate parse error as `CoreError::Memory`.

### Short-term (Fase 2 completion)

8. **Wire `ApprovalEngine` into `agent_loop`**: Before `tool.execute(...)`, call `approval_engine.needs_approval(tc.name, &tc.arguments)`. If approval is needed, return an error with `CoreError::Approval(...)` or pause the loop (channel-based approval flow).

9. **Fix `AgentSession::run` system prompt duplication**: Only prepend system message if `self.messages` doesn't already contain one.

10. **Implement `CommandContext` and wire builtins**: Pass shared state to commands. Make `/agent load` read `~/config/hyprcollab/agents/<name>.md` and update the session's system prompt.

11. **Fix OpenAI streaming tool call reassembly**: Buffer `delta.tool_calls` fragments across chunks and emit a `FinishReason::ToolCalls` chunk when complete.

12. **Add conversation CRUD endpoints**: `GET /api/conversations`, `GET /api/conversations/:id`, `DELETE /api/conversations/:id`.

### Medium-term (quality)

13. **Switch config crate from `anyhow` to `CoreError`**: Consistent error strategy across the workspace.

14. **Add `From<reqwest::Error>` to `CoreError`**: Remove 20+ manual error wrappers.

15. **Add `FTS5` tables to migration**: The `messages` table needs `messages_fts` virtual table for `/api/conversations/search`.

16. **Fix `MemoryTools::FactStore` ID generation**: Use `uuid::Uuid::new_v4().to_string()`.

17. **Add test coverage for `EnhancedShellTool`** and `MemoryTools` (zero coverage currently).

18. **Fix Anthropic `message_start` input token tracking** in streaming.

19. **Remove dead loop in `LlmRouter::resolve`** (`router.rs:66–73`).

20. **Replace `approval` endpoint logic** with real `ApprovalEngine` call.

---

## Test Coverage Summary

| Crate | Coverage Quality | Notable Gaps |
|-------|-----------------|--------------|
| `hyprcollab-core` | ✅ Complete | — |
| `hyprcollab-memory` | ✅ Good | FTS5, approval_log |
| `hyprcollab-config` | ✅ Good | FolderConfig::detect() |
| `hyprcollab-approval` | ✅ Complete | — |
| `hyprcollab-router` | ✅ Good | — |
| `hyprcollab-provider-openai` | ✅ Good | Tool call streaming reassembly |
| `hyprcollab-provider-anthropic` | ✅ Good | Input tokens in streaming |
| `hyprcollab-provider-ollama` | ✅ Good | — |
| `hyprcollab-agent` | ✅ Excellent | Approval integration |
| `hyprcollab-tools` | ⚠️ Partial | `EnhancedShellTool`, `MemoryTools` (zero coverage) |
| `hyprcollab-commands` | ⚠️ Partial | All builtin execute paths are stubs |
| `hyprcollab-server` | 🔴 Tests test stubs | All tests verify fake hardcoded responses |

---

*End of review. 39 issues identified: 11 Fase-3 blockers, 11 high-priority, 17 medium/low.*
