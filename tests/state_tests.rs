use hyprcollab::state::*;
use hyprcollab::ipc::*;
use hyprcollab::storage::messages as msg_store;
use uuid::Uuid;

// Serialise tests that mutate global env vars (XDG_DATA_HOME / XDG_CACHE_HOME).
// unwrap_or_else(|e| e.into_inner()) recovers from a poisoned mutex so one failing
// test doesn't cascade-kill all subsequent ones.
static ENV_LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
fn env_lock() -> std::sync::MutexGuard<'static, ()> {
    ENV_LOCK
        .get_or_init(|| std::sync::Mutex::new(()))
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}

#[test]
fn default_state_has_folders() {
    let state = State::new();
    assert!(!state.folders.is_empty());
    assert_eq!(state.folders.len(), 4); // Uncategorized, Dev, System, Research
}

#[test]
fn default_folders_have_names() {
    let state = State::new();
    assert_eq!(state.folders[0].name, "Uncategorized");
    assert_eq!(state.folders[1].name, "Dev");
    assert_eq!(state.folders[2].name, "System");
    assert_eq!(state.folders[3].name, "Research");
}

#[test]
fn default_folders_have_chats() {
    let state = State::new();
    assert!(!state.folders[0].chats.is_empty()); // Uncategorized has "Quick question"
    assert!(!state.folders[1].chats.is_empty()); // Dev has 3 chats
}

#[test]
fn dev_chats_have_titles() {
    let state = State::new();
    assert_eq!(state.folders[1].chats[0].title, "Fix API bug");
    assert_eq!(state.folders[1].chats[1].title, "Refactor database");
    assert_eq!(state.folders[1].chats[2].title, "Deploy script");
}

#[test]
fn first_chat_has_messages() {
    let state = State::new();
    assert!(!state.folders[1].chats[0].messages.is_empty());
    assert_eq!(state.folders[1].chats[0].messages.len(), 2);
}

#[test]
fn messages_have_correct_roles() {
    let state = State::new();
    assert_eq!(state.folders[1].chats[0].messages[0].role, "user");
    assert_eq!(state.folders[1].chats[0].messages[1].role, "assistant");
}

#[test]
fn messages_have_content() {
    let state = State::new();
    let msg = &state.folders[1].chats[0].messages[0];
    assert!(msg.content.contains("500"));
}

#[test]
fn messages_have_tokens() {
    let state = State::new();
    for msg in &state.folders[1].chats[0].messages {
        assert!(msg.tokens > 0);
    }
}

#[test]
fn chat_tokens_used() {
    let state = State::new();
    assert_eq!(state.folders[1].chats[0].tokens_used, 111);
}

#[test]
fn active_chat_defaults() {
    let state = State::new();
    assert_eq!(state.active_folder_idx, Some(0)); // Uncategorized by default
    assert_eq!(state.active_chat_idx, Some(0));
}

#[test]
fn active_chat_method() {
    let state = State::new();
    let chat = state.active_chat().expect("Should have active chat");
    assert_eq!(chat.title, "Quick question"); // Uncategorized's first chat
}

#[test]
fn active_folder_method() {
    let state = State::new();
    let folder = state.active_folder().expect("Should have active folder");
    assert_eq!(folder.name, "Uncategorized");
}

#[test]
fn default_agents() {
    let state = State::new();
    assert!(!state.agents.is_empty());
    assert_eq!(state.agents.len(), 3);
    assert_eq!(state.agents[0].name, "openai");
    assert_eq!(state.agents[1].name, "claude-code");
    assert_eq!(state.agents[2].name, "opencode");
}

#[test]
fn default_models() {
    let state = State::new();
    assert!(!state.models.is_empty());
    assert!(state.models.iter().any(|m| m.name == "gpt-4o"));
    assert!(state.models.iter().any(|m| m.name == "claude-sonnet-4"));
}

#[test]
fn git_branch_some() {
    let state = State::new();
    assert_eq!(state.folders[1].git_branch, Some("feature/api-fix".to_string())); // Dev
}

#[test]
fn git_branch_none() {
    let state = State::new();
    assert!(state.folders[0].git_branch.is_none()); // Uncategorized
    assert!(state.folders[2].git_branch.is_none()); // System
}

#[test]
fn sidebar_visible_default() {
    let state = State::new();
    assert!(state.sidebar_visible);
}

#[test]
fn folder_workdir() {
    let state = State::new();
    assert!(state.folders[0].workdir.is_none()); // Uncategorized
    assert!(state.folders[1].workdir.is_some()); // Dev
}

#[test]
fn uncategorized_has_chat() {
    let state = State::new();
    assert_eq!(state.folders[0].name, "Uncategorized");
    assert_eq!(state.folders[0].chats.len(), 1);
    assert_eq!(state.folders[0].chats[0].title, "Quick question");
}

// ── IPC Handle Tests ──

#[test]
fn handle_ping() {
    let mut state = State::new();
    let resp = state.handle(Request::Ping);
    assert!(matches!(resp, Response::Pong));
}

#[test]
fn handle_get_state() {
    let mut state = State::new();
    let resp = state.handle(Request::GetState);
    match resp {
        Response::State { data } => {
            assert!(!data.folders.is_empty());
            assert_eq!(data.folders.len(), 4);
        }
        _ => panic!("Expected State response"),
    }
}

#[test]
fn handle_send_message() {
    let mut state = State::new();
    // Select Dev/Fix API bug first
    state.handle(Request::SelectChat { folder: 1, chat: 0 });
    let resp = state.handle(Request::SendMessage {
        content: "Test message".into(),
    });
    match resp {
        Response::State { data } => {
            assert_eq!(data.active_messages.len(), 4); // 2 original + user + assistant
            assert!(data.active_chat_tokens > 111);
        }
        _ => panic!("Expected State response"),
    }
}

#[test]
fn handle_select_chat() {
    let mut state = State::new();
    let resp = state.handle(Request::SelectChat { folder: 1, chat: 1 }); // Dev, Refactor database
    match resp {
        Response::State { data } => {
            assert_eq!(data.active_folder, Some(1));
            assert_eq!(data.active_chat, Some(1));
            assert_eq!(data.active_chat_title, "Refactor database");
        }
        _ => panic!("Expected State response"),
    }
}

#[test]
fn handle_select_invalid_chat() {
    let mut state = State::new();
    let resp = state.handle(Request::SelectChat { folder: 0, chat: 99 });
    assert!(matches!(resp, Response::Error { .. }));
}

#[test]
fn handle_new_folder() {
    let mut state = State::new();
    let resp = state.handle(Request::NewFolder { name: "TestFolder".into() });
    match resp {
        Response::State { data } => {
            assert_eq!(data.folders.len(), 5); // 4 + 1 new
            assert_eq!(data.active_folder, Some(4));
        }
        _ => panic!("Expected State response"),
    }
}

#[test]
fn handle_new_chat() {
    let mut state = State::new();
    // Select Dev first
    state.handle(Request::SelectChat { folder: 1, chat: 0 });
    let resp = state.handle(Request::NewChat { title: "New test chat".into() });
    match resp {
        Response::State { data } => {
            assert_eq!(data.active_chat, Some(3)); // 4th chat in Dev
            assert_eq!(data.active_chat_title, "New test chat");
        }
        _ => panic!("Expected State response"),
    }
}

#[test]
fn handle_toggle_sidebar() {
    let mut state = State::new();
    assert!(state.sidebar_visible);
    state.handle(Request::ToggleSidebar);
    assert!(!state.sidebar_visible);
    state.handle(Request::ToggleSidebar);
    assert!(state.sidebar_visible);
}

#[test]
fn handle_set_sidebar() {
    let mut state = State::new();
    state.handle(Request::SetSidebar { visible: false });
    assert!(!state.sidebar_visible);
}

#[test]
fn handle_set_agent() {
    let mut state = State::new();
    let resp = state.handle(Request::SetAgent { name: "claude-code".into() });
    assert!(matches!(resp, Response::Ack { .. }));
    assert_eq!(state.active_agent_idx, 1);
}

#[test]
fn handle_set_invalid_agent() {
    let mut state = State::new();
    let resp = state.handle(Request::SetAgent { name: "nonexistent".into() });
    assert!(matches!(resp, Response::Error { .. }));
}

#[test]
fn handle_set_model() {
    let mut state = State::new();
    let resp = state.handle(Request::SetModel { name: "claude-sonnet-4".into() });
    assert!(matches!(resp, Response::Ack { .. }));
    assert_eq!(state.active_model_idx, 2);
}

#[test]
fn snapshot_has_all_fields() {
    let state = State::new();
    let snap = state.snapshot();
    assert!(!snap.folders.is_empty());
    assert!(!snap.agents.is_empty());
    assert!(!snap.models.is_empty());
    assert!(snap.sidebar_visible);
    assert_eq!(snap.active_chat_title, "Quick question"); // Default active
}

#[test]
fn total_tokens_after_messages() {
    let mut state = State::new();
    state.handle(Request::SelectChat { folder: 1, chat: 0 }); // Dev/Fix API bug
    let initial = state.active_chat().unwrap().tokens_used;
    state.handle(Request::SendMessage { content: "test".into() });
    let after = state.active_chat().unwrap().tokens_used;
    assert!(after > initial);
}

#[test]
fn select_uncategorized_chat() {
    let mut state = State::new();
    let resp = state.handle(Request::SelectChat { folder: 0, chat: 0 });
    match resp {
        Response::State { data } => {
            assert_eq!(data.active_chat_title, "Quick question");
        }
        _ => panic!("Expected State response"),
    }
}

// ── Persistence Tests ─────────────────────────────────────────────────────────

/// # test_message_append_and_load
///
/// Verifica que `append_message` escribe en JSONL y `load_messages` recupera
/// exactamente los mensajes guardados, en el mismo orden.
///
/// ## Precondiciones
/// - IDs únicos (UUID) para evitar colisión entre tests paralelos
///
/// ## Estado esperado al final
/// ```text
/// ~/.local/share/hypr-collab/data/folders/__test_{uuid}/chats/__test_{uuid}/
/// └── messages.jsonl  (2 líneas JSON)
///
/// load_messages(folder_id, chat_id) → Vec[
///   Message { role: "user", content: "hello world", tokens: 5 },
///   Message { role: "user", content: "hello world", tokens: 5 },
/// ]
/// ```
#[test]
fn test_message_append_and_load() {
    let tmp = tempfile::tempdir().unwrap();
    let _lock = env_lock();
    std::env::set_var("XDG_DATA_HOME", tmp.path().to_str().unwrap());
    let folder_id = format!("test_{}", Uuid::new_v4().simple());
    let chat_id   = format!("__test_{}", Uuid::new_v4().simple());

    let msg = Message {
        role: "user".into(),
        content: "hello world".into(),
        timestamp: 1000,
        tokens: 5,
    };

    // Append twice → JSONL with 2 lines
    msg_store::append_message(&folder_id, &chat_id, &msg).unwrap();
    msg_store::append_message(&folder_id, &chat_id, &msg).unwrap();

    let loaded = msg_store::load_messages(&folder_id, &chat_id).unwrap();
    assert_eq!(loaded.len(), 2, "debe haber exactamente 2 mensajes");
    assert_eq!(loaded[0].content, "hello world");
    assert_eq!(loaded[1].role, "user");

    // Verify the JSONL file exists and has exactly 2 non-empty lines
    let path = msg_store::messages_path(&folder_id, &chat_id);
    assert!(path.exists(), "messages.jsonl debe existir en disco");
    let raw = std::fs::read_to_string(&path).unwrap();
    assert_eq!(raw.lines().count(), 2, "JSONL debe tener 2 líneas");
    // tmpdir cleans up automatically on drop
}

/// # test_load_messages_nonexistent_returns_empty
///
/// Verifica que cargar mensajes de un chat inexistente devuelve `Ok(vec![])`
/// sin pánico — comportamiento correcto en first-run o chat vacío.
///
/// ## Precondiciones
/// - IDs únicos que garantizan que el directorio no existe
///
/// ## Estado esperado al final
/// ```text
/// ~/.local/share/hypr-collab/data/folders/__test_{uuid}/   ← NO existe
///
/// load_messages(folder_id, chat_id) → Ok([])
/// ```
#[test]
fn test_load_messages_nonexistent_returns_empty() {
    let tmp = tempfile::tempdir().unwrap();
    let _lock = env_lock();
    std::env::set_var("XDG_DATA_HOME", tmp.path().to_str().unwrap());
    let folder_id = format!("test_{}", Uuid::new_v4().simple());
    let chat_id   = format!("test_{}", Uuid::new_v4().simple());

    let result = msg_store::load_messages(&folder_id, &chat_id);
    assert!(result.is_ok(), "no debe devolver error para chat inexistente");
    assert!(result.unwrap().is_empty(), "debe devolver vec vacío");
}

/// # test_append_preserves_all_fields
///
/// Verifica que el round-trip serialización → JSONL → deserialización conserva
/// todos los campos de Message sin pérdida, incluyendo UTF-8 y emojis.
///
/// ## Estado esperado al final
/// ```text
/// Mensaje original:
///   role: "assistant"
///   content: "respuesta con ñ y emojis 🦀"
///   timestamp: 9_999_999_999
///   tokens: 42
///
/// Después de append → load:
///   role: "assistant"  ✓
///   content: "respuesta con ñ y emojis 🦀"  ✓  (UTF-8 intacto)
///   timestamp: 9_999_999_999  ✓
///   tokens: 42  ✓
/// ```
#[test]
fn test_append_preserves_all_fields() {
    let tmp = tempfile::tempdir().unwrap();
    let _lock = env_lock();
    std::env::set_var("XDG_DATA_HOME", tmp.path().to_str().unwrap());
    let folder_id = format!("test_{}", Uuid::new_v4().simple());
    let chat_id   = format!("test_{}", Uuid::new_v4().simple());

    let original = Message {
        role: "assistant".into(),
        content: "respuesta con ñ y emojis 🦀".into(),
        timestamp: 9_999_999_999,
        tokens: 42,
    };

    msg_store::append_message(&folder_id, &chat_id, &original).unwrap();
    let loaded = msg_store::load_messages(&folder_id, &chat_id).unwrap();

    assert_eq!(loaded.len(), 1);
    let m = &loaded[0];
    assert_eq!(m.role, original.role);
    assert_eq!(m.content, original.content, "UTF-8 content must survive round-trip");
    assert_eq!(m.timestamp, original.timestamp);
    assert_eq!(m.tokens, original.tokens);
    // tmpdir cleans up automatically on drop
}

// ── Fase C: Waybar ────────────────────────────────────────────────────────────

#[test]
fn waybar_publish_idle_creates_file() {
    let tmp = tempfile::tempdir().unwrap();
    let _lock = env_lock();
    std::env::set_var("XDG_CACHE_HOME", tmp.path().to_str().unwrap());

    let state = State::new();
    hyprcollab::waybar::publish_from_state(&state).unwrap();

    let path = hyprcollab::utils::paths::waybar_status_path();
    assert!(path.exists(), "waybar.json should be created");
    let content = std::fs::read_to_string(&path).unwrap();
    let v: serde_json::Value = serde_json::from_str(&content).unwrap();
    assert_eq!(v["alt"], "idle");
    assert_eq!(v["class"], "idle");
}

#[test]
fn waybar_publish_working_shows_processing() {
    let tmp = tempfile::tempdir().unwrap();
    let _lock = env_lock();
    std::env::set_var("XDG_CACHE_HOME", tmp.path().to_str().unwrap());

    let mut state = State::new();
    state.working = true;
    hyprcollab::waybar::publish_from_state(&state).unwrap();

    let path = hyprcollab::utils::paths::waybar_status_path();
    let content = std::fs::read_to_string(&path).unwrap();
    let v: serde_json::Value = serde_json::from_str(&content).unwrap();
    assert_eq!(v["alt"], "processing");
    assert_eq!(v["class"], "processing");
}

// ── Fase D: Slash Commands ────────────────────────────────────────────────────

#[test]
fn parse_returns_none_for_plain_text() {
    assert!(hyprcollab::commands::parse("hello world").is_none());
    assert!(hyprcollab::commands::parse("").is_none());
}

#[test]
fn parse_help_command() {
    match hyprcollab::commands::parse("/help").unwrap() {
        hyprcollab::commands::SlashCommand::Help => {}
        other => panic!("expected Help, got {:?}", std::mem::discriminant(&other)),
    }
}

#[test]
fn parse_run_command() {
    match hyprcollab::commands::parse("/run ls -la").unwrap() {
        hyprcollab::commands::SlashCommand::Run { command } => {
            assert_eq!(command, "ls -la");
        }
        _ => panic!("expected Run"),
    }
}

#[test]
fn parse_clear_command() {
    match hyprcollab::commands::parse("/clear").unwrap() {
        hyprcollab::commands::SlashCommand::Clear => {}
        _ => panic!("expected Clear"),
    }
}

#[test]
fn parse_unknown_command() {
    match hyprcollab::commands::parse("/foobar").unwrap() {
        hyprcollab::commands::SlashCommand::Unknown { raw } => {
            assert!(raw.contains("foobar"));
        }
        _ => panic!("expected Unknown"),
    }
}

#[tokio::test]
async fn execute_help_returns_text() {
    let cmd = hyprcollab::commands::SlashCommand::Help;
    let output = hyprcollab::commands::execute(cmd).await;
    assert!(output.contains("/run"));
    assert!(output.contains("/read"));
}

#[tokio::test]
async fn execute_run_echo() {
    let cmd = hyprcollab::commands::SlashCommand::Run { command: "echo hello".into() };
    let output = hyprcollab::commands::execute(cmd).await;
    assert!(output.contains("hello"), "output: {}", output);
}

#[tokio::test]
async fn execute_run_bad_command_no_panic() {
    let cmd = hyprcollab::commands::SlashCommand::Run { command: "nonexistent-command-xyz-abc".into() };
    let output = hyprcollab::commands::execute(cmd).await;
    // Should return an error message, not panic
    assert!(!output.is_empty());
}

#[tokio::test]
async fn execute_read_existing_file() {
    // Read Cargo.toml which always exists
    let cmd = hyprcollab::commands::SlashCommand::Read {
        path: "Cargo.toml".into(),
        start: None,
        end: None,
    };
    let output = hyprcollab::commands::execute(cmd).await;
    assert!(output.contains("hyprcollab") || output.contains("Cannot read"), "output: {}", output);
}

// ── Fase E: RAG ───────────────────────────────────────────────────────────────

#[test]
fn chunker_empty_input() {
    let chunks = hyprcollab::rag::chunker::chunk("", 100, 10);
    assert!(chunks.is_empty());
}

#[test]
fn chunker_short_text_single_chunk() {
    let chunks = hyprcollab::rag::chunker::chunk("hello world", 100, 10);
    assert_eq!(chunks.len(), 1);
    assert_eq!(chunks[0].text, "hello world");
    assert_eq!(chunks[0].index, 0);
}

#[test]
fn chunker_long_text_multiple_chunks() {
    let words: Vec<String> = (0..200).map(|i| format!("word{}", i)).collect();
    let text = words.join(" ");
    let chunks = hyprcollab::rag::chunker::chunk(&text, 50, 10);
    assert!(chunks.len() > 1, "should produce multiple chunks");
    // All chunks should have content
    for c in &chunks {
        assert!(!c.text.is_empty());
    }
}

#[test]
fn cosine_similarity_identical_vectors() {
    use hyprcollab::rag::embedder::cosine_similarity;
    let v = vec![1.0f32, 0.0, 0.0];
    assert!((cosine_similarity(&v, &v) - 1.0).abs() < 1e-6);
}

#[test]
fn cosine_similarity_orthogonal_vectors() {
    use hyprcollab::rag::embedder::cosine_similarity;
    let a = vec![1.0f32, 0.0];
    let b = vec![0.0f32, 1.0];
    assert!((cosine_similarity(&a, &b)).abs() < 1e-6);
}

#[test]
fn rag_index_insert_and_search() {
    let tmp = tempfile::tempdir().unwrap();
    let _lock = env_lock();
    std::env::set_var("XDG_DATA_HOME", tmp.path().to_str().unwrap());

    let folder_id = "test-folder-rag";
    let entry = hyprcollab::rag::index::RagEntry {
        id: "e1".into(),
        folder_id: folder_id.into(),
        chat_id: "c1".into(),
        chunk_index: 0,
        text: "the bug was on line 42".into(),
        message_id: "m1".into(),
        created_at: 0,
        vector: vec![1.0, 0.0, 0.0],
    };

    hyprcollab::rag::index::insert(folder_id, &entry).unwrap();

    // Search with the exact same vector → score = 1.0 ≥ 0.7
    let results = hyprcollab::rag::index::search(folder_id, &[1.0, 0.0, 0.0], 5, 0.7).unwrap();
    assert_eq!(results.len(), 1);
    assert!(results[0].text.contains("line 42"));

    // Search with orthogonal vector → score = 0.0 < 0.7
    let results = hyprcollab::rag::index::search(folder_id, &[0.0, 1.0, 0.0], 5, 0.7).unwrap();
    assert_eq!(results.len(), 0);
}

#[test]
fn rag_index_clear() {
    let tmp = tempfile::tempdir().unwrap();
    let _lock = env_lock();
    std::env::set_var("XDG_DATA_HOME", tmp.path().to_str().unwrap());

    let folder_id = "test-folder-clear";
    for i in 0..3 {
        let entry = hyprcollab::rag::index::RagEntry {
            id: format!("e{}", i),
            folder_id: folder_id.into(),
            chat_id: "c1".into(),
            chunk_index: i,
            text: format!("chunk {}", i),
            message_id: "m1".into(),
            created_at: 0,
            vector: vec![1.0, 0.0, 0.0],
        };
        hyprcollab::rag::index::insert(folder_id, &entry).unwrap();
    }

    let removed = hyprcollab::rag::index::clear(folder_id).unwrap();
    assert_eq!(removed, 3);

    // After clear, search returns nothing
    let results = hyprcollab::rag::index::search(folder_id, &[1.0, 0.0, 0.0], 5, 0.0).unwrap();
    assert_eq!(results.len(), 0);
}

// ── RAG scope: global search ──────────────────────────────────────────────────

#[test]
fn rag_search_all_folders_merges_results() {
    let tmp = tempfile::tempdir().unwrap();
    let _lock = env_lock();
    std::env::set_var("XDG_DATA_HOME", tmp.path().to_str().unwrap());

    // Two folders with one chunk each
    let entry_a = hyprcollab::rag::index::RagEntry {
        id: "a1".into(), folder_id: "folder-a".into(), chat_id: "c1".into(),
        chunk_index: 0, text: "rust ownership model".into(), message_id: "m1".into(),
        created_at: 0, vector: vec![1.0, 0.0, 0.0],
    };
    let entry_b = hyprcollab::rag::index::RagEntry {
        id: "b1".into(), folder_id: "folder-b".into(), chat_id: "c2".into(),
        chunk_index: 0, text: "python async await".into(), message_id: "m2".into(),
        created_at: 0, vector: vec![0.0, 1.0, 0.0],
    };

    hyprcollab::rag::index::insert("folder-a", &entry_a).unwrap();
    hyprcollab::rag::index::insert("folder-b", &entry_b).unwrap();

    // Search with folder-a's vector — should find entry_a with score 1.0
    // and entry_b with score 0.0 (filtered out by min_score 0.7)
    let results = hyprcollab::rag::index::search_all_folders(
        Some("folder-a"), &[1.0, 0.0, 0.0], 5, 0.7, 0.9,
    ).unwrap();

    assert_eq!(results.len(), 1);
    assert!(results[0].text.contains("rust ownership"));
}

#[test]
fn rag_search_all_folders_penalty_applied() {
    let tmp = tempfile::tempdir().unwrap();
    let _lock = env_lock();
    std::env::set_var("XDG_DATA_HOME", tmp.path().to_str().unwrap());

    // Both folders with similar vectors; active folder should score higher
    let entry_active = hyprcollab::rag::index::RagEntry {
        id: "x1".into(), folder_id: "folder-x".into(), chat_id: "c1".into(),
        chunk_index: 0, text: "active folder chunk".into(), message_id: "m1".into(),
        created_at: 0, vector: vec![1.0, 0.0],
    };
    let entry_other = hyprcollab::rag::index::RagEntry {
        id: "y1".into(), folder_id: "folder-y".into(), chat_id: "c2".into(),
        chunk_index: 0, text: "other folder chunk".into(), message_id: "m2".into(),
        created_at: 0, vector: vec![1.0, 0.0],
    };

    hyprcollab::rag::index::insert("folder-x", &entry_active).unwrap();
    hyprcollab::rag::index::insert("folder-y", &entry_other).unwrap();

    // penalty=0.9 means other folder scores 0.9, active scores 1.0
    let results = hyprcollab::rag::index::search_all_folders(
        Some("folder-x"), &[1.0, 0.0], 5, 0.0, 0.9,
    ).unwrap();

    assert_eq!(results.len(), 2);
    // Active folder result should be first (higher score)
    assert_eq!(results[0].text, "active folder chunk");
    assert_eq!(results[1].text, "other folder chunk");
}

#[test]
fn rag_config_scope_defaults_to_both() {
    use hyprcollab::storage::RagConfig;
    let cfg = RagConfig::default();
    assert!(cfg.is_both());
    assert!(!cfg.is_folder());
    assert!(!cfg.is_global());
}

#[test]
fn rag_config_scope_folder() {
    use hyprcollab::storage::RagConfig;
    let cfg = RagConfig { scope: "folder".into(), ..RagConfig::default() };
    assert!(cfg.is_folder());
    assert!(!cfg.is_both());
}
