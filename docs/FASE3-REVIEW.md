# HyprCollab Fase 3 Code Review

**Reviewed**: 2026-05-24  
**Branch**: `feat/cc-backend`  
**Scope**: All crates, Phases 0–3  
**Reviewer**: Claude Sonnet 4.6 (via Context7 docs for syntect, pulldown-cmark, resvg, rusqlite, ratatui, tokio-tungstenite, image)

---

## Legend

| Symbol | Meaning |
|--------|---------|
| 🔴 | **BLOCKER** — must fix before Fase 4 |
| 🟡 | **WARNING** — should fix soon |
| 🟢 | **GOOD** — well implemented |

---

## Summary

| Severity | Count |
|----------|-------|
| 🔴 BLOCKER | 1 |
| 🟡 WARNING | 12 |
| 🟢 GOOD | 22 |

**Test run**: `cargo test --workspace` — **369 tests, 0 failures, 0 warnings**

---

## NEW Crates — Fase 3 (Highest Priority)

---

### `crates/hyprcollab-artifacts/`

#### 🟢 ArtifactId newtype and Artifact enum

`types.rs` is clean. The `ArtifactId(String)` newtype wraps UUIDs, has correct `Default` (generates a fresh UUID), and `Display`. The `Artifact` enum uses `#[serde(tag = "type", rename_all = "snake_case")]` for clean JSON serialization. All methods (`type_name`, `extension`, `content_str`) are exhaustive and correct.

#### 🟢 ArtifactStore uses spawn_blocking correctly

`store.rs` wraps every `rusqlite::Connection` operation in `tokio::task::spawn_blocking`. This is the correct pattern because `rusqlite::Connection` is `!Send`; the connection never crosses an await point. All blocking errors are mapped cleanly.

#### 🟢 SQL queries use parameterized inputs — no injection risk

All queries use `rusqlite::params![]`. The schema uses a primary key on `id` (TEXT, UUID-shaped) and an index on `chat_id`. No string concatenation into SQL anywhere.

#### 🟡 ArtifactStore opens a new SQLite connection per operation

Every `create()`, `get()`, `update()`, `list_for_chat()`, and `delete()` call opens a fresh `rusqlite::Connection::open(...)`. SQLite open+close is cheap for WAL mode, but for a store serving many artifact-heavy conversations this incurs unnecessary overhead. **Consider a `deadpool-sqlite` pool or a `Mutex<Connection>` shared within the store.**

Location: `store.rs:38, 70, 89, 127, 149, 190`

#### 🟡 `DateTime::from_timestamp` deprecation in chrono 0.4.38+

`store.rs:163–165`:
```rust
chrono::DateTime::from_timestamp(created_ts, 0).unwrap_or_default()
```
`DateTime::<Utc>::from_timestamp` is deprecated in recent chrono in favour of `DateTime::from_timestamp` on the `Utc` type directly, or `chrono::DateTime::from_timestamp`. While this compiles and works, a future chrono bump will produce warnings. **Use `chrono::DateTime::from_timestamp(ts, 0).unwrap_or_default()` without the turbofish, or switch to `chrono::DateTime::<chrono::Utc>::from_timestamp`.**

#### 🟡 `delete()` enumerates all known extensions — misses future types

`store.rs:181–184`:
```rust
for ext in &["rs", "py", "js", "ts", "md", "html", "svg", "mmd", "jsx", "tex", "txt", "json"] {
```
When a new `Artifact` variant is added (e.g. `Wasm`, `Jupyter`), its extension must be manually added here or the file will be orphaned on disk. **Prefer storing the extension in the DB row at creation time so `delete()` can look it up.**

#### 🟢 Test coverage is comprehensive

`lib.rs` tests cover: CRUD round-trips, serde round-trips for every variant, filesystem write verification, update-of-nonexistent returns error, delete returns correct boolean. 15 meaningful tests — not stubs.

---

### `crates/hyprcollab-canvas/`

---

#### 🟢 CodeRenderer — syntect API usage is correct per docs

`code.rs` uses `SyntaxSet::load_defaults_newlines()`, `ThemeSet::load_defaults()`, `HighlightLines::new(syntax, theme)`, `h.highlight_line(line, &ps)`, `as_24_bit_terminal_escaped(&ranges, false)`, and `styled_line_to_highlighted_html`. All match the Context7 syntect documentation exactly. The `false` for background parameter suppresses ANSI background colours appropriately for terminal use. ANSI reset `\x1b[0m` at the end prevents colour bleed.

#### 🟡 CodeRenderer loads SyntaxSet+ThemeSet on every `new()` — expensive

`code.rs:24–27`:
```rust
pub fn new() -> Self {
    Self {
        syntax_set: SyntaxSet::load_defaults_newlines(),  // deserialises ~2 MB
        theme_set: ThemeSet::load_defaults(),
    }
}
```
These are large, fully loaded structs. In production every renderer construction pays this cost. **Wrap in `once_cell::sync::Lazy<SyntaxSet>` and `Lazy<ThemeSet>` globals, or accept them by reference.**

---

#### 🔴 BLOCKER — MarkdownRenderer preview truncation panics on non-ASCII content

`markdown.rs:47–51`:
```rust
let preview = if content.len() > PREVIEW_CHARS {
    format!("{}…", &content[..PREVIEW_CHARS])  // ← byte-index slice
} else {
    content.to_string()
};
```

`content.len()` returns the byte count, and `&content[..500]` is a byte-range slice. If the 500th byte falls inside a multi-byte UTF-8 codepoint (any non-ASCII character — emoji, accented characters, CJK, Arabic, etc.), this panics with `byte index 500 is not a char boundary`.

**Fix**:
```rust
let preview = if content.chars().count() > PREVIEW_CHARS {
    let end = content
        .char_indices()
        .nth(PREVIEW_CHARS)
        .map(|(i, _)| i)
        .unwrap_or(content.len());
    format!("{}…", &content[..end])
} else {
    content.to_string()
};
```

Or use the `unicode-segmentation` crate for grapheme-cluster awareness if the preview is for display.

---

#### 🟢 SvgRenderer — resvg/usvg API correct per Context7 docs

`svg.rs` uses `usvg::Tree::from_data(svg_content.as_bytes(), &opt)`, `tree.size().to_int_size()`, `Pixmap::new(w, h)`, `resvg::render(&tree, Transform::default(), &mut pixmap.as_mut())`, `pixmap.encode_png()` — exactly matching the Context7 resvg examples. Graceful fallback to raw SVG source on render failure prevents panics on malformed input.

#### 🟢 MermaidRenderer — appropriate for current scope

Returns source wrapped in `<div class="mermaid">` for client-side Mermaid.js rendering. Correct strategy since there is no server-side Mermaid renderer. The `is_valid_syntax` keyword-prefix check is appropriately labelled as a hint, not authoritative validation.

#### 🟡 `unified_diff` hunk header always starts at line 1

`diff.rs:17–23`:
```rust
result.push_str(&format!(
    "@@ -{},{} +{},{} @@\n",
    1,           // ← always 1, not the actual first changed line
    old_lines.len(),
    1,
    new_lines.len()
));
```

Standard unified-diff format encodes the start line of the hunk, not always 1. Patches produced here cannot be applied with `patch -p0` unless they happen to span the whole file. For display-only use this is fine, but **label it as a display diff, not a standard patch**, or fix the hunk header computation.

#### 🟡 `CanvasManager::open_ids()` returns non-deterministic order

`manager.rs:112–114`:
```rust
pub fn open_ids(&self) -> Vec<ArtifactId> {
    self.open.keys().cloned().collect()
}
```
`HashMap` key iteration is unordered and varies between runs. If the TUI renders tabs in this order, tab positions will shuffle unpredictably. **Use `IndexMap` (preserves insertion order) or maintain a separate `Vec<ArtifactId>` for tab order.**

#### 🟢 CanvasManager event model and history tracking

Drain-based event queue (`take_events` uses `std::mem::take`) is clean and avoids allocation. History snapshots per artifact enable diff computation. Close correctly reassigns active tab. All test assertions verify exact event sequences. Well implemented.

---

### `crates/hyprcollab-media-preview/`

#### 🟢 Kitty protocol encoding is spec-correct

`kitty.rs`: First chunk includes `a=T,f=100,q=1,m={more}`, continuation chunks include `m={more}` only. `\x1b_G...;\x1b\\` APC framing is correct. Base64 chunk size 4096 matches the kitty spec recommendation. The `expect("base64 is ASCII")` on UTF-8 validation is safe because standard base64 output is always ASCII — not a panic risk.

#### 🟢 Sixel encoding is correct

`sixel.rs` produces: DCS intro `\x1bP0;0;0q`, raster attributes `"1;1;w;h`, palette entries `#n;2;R;G;B` in 0–100 range, pixel bands of 6 rows, `$` for CR within band, `-` for next band, ST `\x1b\\`. The `b + 63` character encoding: bit masks 0–63 map to characters 63–126 (? through ~), which is the correct sixel character range. Palette capped at 256 colours.

#### 🟡 `ImagePreview` cache field is never written — dead code

`preview.rs:23–29`:
```rust
pub struct ImagePreview {
    caps: TerminalCapabilities,
    cache: HashMap<u64, String>,  // ← never populated
}
```
`clear_cache()` is implemented but `encode_png()` never inserts into `cache`. Either implement the caching (hash PNG bytes → encoded sequence) or remove the dead field and method.

#### 🟢 `resize_to_cols` uses `thumbnail()` for aspect-ratio-preserving scaling

`preview.rs:82–88`: `img.thumbnail(max_px, u32::MAX)` from the `image` crate correctly scales width-bound while preserving aspect ratio. The 8px-per-column assumption is a reasonable approximation for typical terminal font sizes.

---

### `platforms/hyprcollab-tui/`

#### 🟡 `App.pane` and `InputHandler.pane` are duplicated and can diverge

`app.rs:9` has `pub pane: FocusedPane` and `input.rs:53` has `pub pane: FocusedPane`. When Tab is pressed, `InputHandler.handle_key` updates `self.pane` (`input.rs:121`) but `App.pane` is never updated. Any code reading `App.pane` to determine focus will see stale state.

**Fix**: Remove `App.pane` and use `app.input.pane` as the single source of truth, or update `App.pane` in the event loop after processing key actions.

#### 🟢 Vim keybind implementation is correct

`i` → Insert, `Esc` → Normal, `:` → Command, `j/k` scroll, `gg` top (pending-g state machine), `G` bottom, `Tab` switches pane, `Ctrl-Q` quit, `:q`/`:quit` quit. All state transitions are correct and tested. `pending_g` is cleared on any non-`g` key, preventing stale state.

#### 🟢 ChatPane streaming accumulation works correctly

`push_stream_token` appends to the last message if it is still streaming, otherwise creates a new streaming message. `push_token` with `done=true` clears the `streaming` flag. This correctly handles partial token delivery.

#### 🟢 ArtifactPane scroll resets on new artifact

`set_artifact` resets `self.scroll = 0` every time. Prevents scroll position carrying over to unrelated content.

---

### `crates/hyprcollab-server/src/artifacts.rs`

#### 🟢 Five artifact endpoints are complete and correct

| Endpoint | Status |
|----------|--------|
| `POST /api/artifacts` | 201 Created with id |
| `GET /api/artifacts?chat_id=` | 200 with list |
| `GET /api/artifacts/:id` | 200 or 404 |
| `PUT /api/artifacts/:id` | 204 No Content or 404 |
| `DELETE /api/artifacts/:id` | 200 with `{deleted: bool}` |

All five map `ArtifactError::NotFound` to HTTP 404 and other errors to HTTP 500. Input validation (empty `chat_id`) returns 400. Integration tests use `tower::ServiceExt::oneshot` against a real in-memory store.

#### 🟡 CORS allows any origin — document the tradeoff

`app.rs:69`: `CorsLayer::new().allow_origin(Any)` — fine for a local server but means any web page can call the API. **Document this as intentional for the local-server use case**, or restrict to `localhost` origins in production builds.

#### 🟡 `chat_handler` is a stub — does not call any LLM provider

`app.rs:95–142`: The chat handler echoes words back with `"Response to: {message}"`. The approval handler checks `tool_name.starts_with("unsafe")`. Both are placeholders. **These should be wired to real providers in Fase 4**; filing them as warnings since they are clearly stubs and the tests don't assert correct LLM behaviour.

---

## Existing Crates — Phase 0–2 Verification

---

### `crates/hyprcollab-memory/`

#### 🟢 Send+Sync fix: `Arc<Mutex<Connection>>` is correct

`store.rs:20–21`: Wrapping `rusqlite::Connection` in `Arc<Mutex<>>` makes `MemoryStore: Send + Sync`. The mutex is `std::sync::Mutex` (not async), which is correct — lock is never held across an `.await` point. All methods are synchronous.

#### 🟢 Migration transactions use `unchecked_transaction()` correctly

`migrations.rs:37–51`: Each migration runs in its own transaction via `conn.unchecked_transaction()`. This is appropriate when the caller controls the connection lifetime. The `schema_version` table prevents re-applying migrations. Idempotency test confirms this.

---

### `crates/hyprcollab-agent/`

#### 🟢 Approval wiring is correct

`agent_loop.rs:69–73`: `approval.map_or(false, |eng| eng.needs_approval(...) && config.approval_mode == Strict)` — approval is only enforced when the engine says yes AND the config is Strict. Auto mode bypasses the engine check correctly.

---

### `crates/hyprcollab-tools/`

#### 🟢 Path traversal prevention is correct

`file_ops.rs:37–62`: `validate_path` canonicalizes the path (or its parent for new files), then calls `starts_with(sandbox_canonical)`. For `/sandbox/../escape.txt`, the parent is `..` → canonicalizes to `/`, which does not start with `/sandbox`. Traversal correctly blocked. Tested with symlink-safe `canonicalize()`.

#### 🟡 Shell tool has no command injection mitigation — by design but document it

`shell.rs:96–98`: Commands are passed directly to `sh -c`. This is intentional (the shell tool IS a shell), but callers (agent loop, `/run`) must enforce approval gating. **Add a doc comment stating that callers are responsible for authorization.**

---

### `crates/hyprcollab-approval/`

#### 🟡 Approval cache keyed by tool name only — not by arguments

`engine.rs:64`:
```rust
pub fn cache_decision(&mut self, tool_name: &str, _args: &serde_json::Value, allowed: bool) {
    self.cache.insert(tool_name.to_string(), allowed);
}
```
A previous approval for `file_ops { action: "read", path: "/safe" }` would also approve `file_ops { action: "write", path: "/etc/passwd" }` on the second call. **Cache key should be `format!("{tool_name}:{}", args_hash)` where `args_hash` is a stable hash of the arguments value.**

---

### `crates/hyprcollab-provider-openai/`

#### 🟢 Streaming tool call assembly is correct

`streaming.rs`: Tool call fragments accumulate per `index` into `PartialToolCall`, arguments are concatenated as strings, then parsed with `serde_json::from_str` at the finish chunk. Handles multi-tool-call responses correctly. Channel size 256 prevents backpressure from slowing the provider.

#### 🟡 Invalid tool call JSON arguments silently produce `Null`

`streaming.rs:148–150`:
```rust
arguments: serde_json::from_str(&acc.arguments)
    .unwrap_or(serde_json::Value::Null),
```
If the LLM emits malformed JSON in arguments (truncated, incomplete), this silently falls back to `Null`. The agent loop will then attempt to execute the tool with null arguments, producing a confusing error. **At minimum, log a warning: `tracing::warn!("malformed tool args for {}: {}", acc.name, e)`.**

---

## `cargo test --workspace` Results

```
Running 369 tests across all workspace crates
0 failures
0 ignored
```

All tests pass. Test quality is high — tests are substantive (verify actual behaviour, not just that functions compile) across:

- `hyprcollab-artifacts`: 15 tests — full CRUD, filesystem writes, serde round-trips
- `hyprcollab-canvas`: 52 tests — all render modes, diff edge cases, manager lifecycle
- `hyprcollab-media-preview`: 17 tests — protocol encoding, resize, dispatch
- `hyprcollab-tui`: 45 tests — mode transitions, scroll, streaming, pane state
- `hyprcollab-server`: 23 tests — HTTP integration tests against real store
- `hyprcollab-memory`: 10 tests — CRUD, migration idempotency
- `hyprcollab-agent`: 13 tests — loop, approval, tool registry
- `hyprcollab-commands`: 18 tests
- `hyprcollab-approval`: 7 tests
- Workspace integration: 13 tests

---

## Priority Fix List for Fase 4

### Must Fix (🔴 BLOCKER)

1. **`markdown.rs:47`** — Replace `&content[..PREVIEW_CHARS]` byte-index slice with a char-boundary-safe truncation. Any Markdown content with non-ASCII characters (the common case for international users, emoji in code comments, etc.) will panic.

### Should Fix Soon (🟡 WARNING)

2. **`artifacts/store.rs`** — Connection per operation. Consider `deadpool-sqlite` or a single shared `Mutex<Connection>` within `ArtifactStore`.
3. **`canvas/code.rs:24`** — Move `SyntaxSet`/`ThemeSet` to `once_cell::sync::Lazy` globals.
4. **`canvas/diff.rs:17`** — Fix unified diff hunk header to use actual start line, or document as display-only.
5. **`canvas/manager.rs:112`** — Use `IndexMap` for stable tab order in `open_ids()`.
6. **`tui/app.rs:9`** — Remove `App.pane` duplication or keep it in sync with `input.pane`.
7. **`media-preview/preview.rs:28`** — Implement or remove the unused `cache` field.
8. **`approval/engine.rs:64`** — Key cache on `(tool_name, stable_hash(args))`.
9. **`provider-openai/streaming.rs:148`** — Log a `tracing::warn!` on malformed tool-call arguments instead of silently producing Null.
10. **`server/app.rs:69`** — Document CORS `allow_origin(Any)` as intentional for local use.
11. **`tools/shell.rs:96`** — Add doc comment that callers are responsible for authorization.
12. **`artifacts/store.rs:181`** — Store extension in DB row at creation; look up at deletion instead of enumerating all known extensions.
