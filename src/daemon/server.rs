use hyprcollab::agents::openai::OpenAIConnector;
use hyprcollab::agents::{AgentConnector, AgentEvent, ChatMessage};
use hyprcollab::ipc::*;
use hyprcollab::rag::embedder::Embedder;
use hyprcollab::state::{Message, State};
use hyprcollab::storage::Config;
use hyprcollab::waybar;
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixListener;
use tokio::sync::mpsc;
use tracing::{error, info, warn};

pub async fn run_ipc_server() -> anyhow::Result<()> {
    let socket_path = crate::utils::paths::socket_path();
    let _ = tokio::fs::remove_file(&socket_path).await;
    let listener = UnixListener::bind(&socket_path)?;
    info!("IPC server listening on {}", socket_path.display());

    let state = Arc::new(Mutex::new(
        State::load().unwrap_or_else(|_| State::new()),
    ));
    let config = Arc::new(Config::load().unwrap_or_default());

    // Publish initial idle status
    if let Err(e) = waybar::publish_from_state(&state.lock().unwrap()) {
        warn!("waybar initial publish: {}", e);
    }

    loop {
        let (stream, addr) = listener.accept().await?;
        info!("Client connected: {:?}", addr);
        let state = state.clone();
        let config = config.clone();
        tokio::spawn(async move {
            if let Err(e) = handle_client(stream, state, config).await {
                error!("Client error: {}", e);
            }
        });
    }
}

async fn handle_client(
    stream: tokio::net::UnixStream,
    state: Arc<Mutex<State>>,
    config: Arc<Config>,
) -> anyhow::Result<()> {
    let (reader, mut writer) = stream.into_split();
    let mut reader = BufReader::new(reader);
    let mut line = String::new();

    loop {
        line.clear();
        match reader.read_line(&mut line).await {
            Ok(0) => break,
            Ok(_) => {
                let req = match decode_request(&line) {
                    Some(r) => r,
                    None => {
                        let err = encode(&Response::Error {
                            msg: format!("Invalid request: {}", line.trim()),
                        });
                        let _ = writer.write_all(err.as_bytes()).await;
                        continue;
                    }
                };

                match req {
                    Request::SendMessage { content } => {
                        if let Err(e) =
                            handle_send_message(content, &state, &config, &mut writer).await
                        {
                            warn!("SendMessage error: {}", e);
                            let out = encode(&Response::Error { msg: e.to_string() });
                            let _ = writer.write_all(out.as_bytes()).await;
                        }
                    }
                    other => {
                        let resp = {
                            let mut s = state.lock().unwrap();
                            s.handle(other)
                        };
                        let out = encode(&resp);
                        if writer.write_all(out.as_bytes()).await.is_err() {
                            break;
                        }
                    }
                }
            }
            Err(e) => {
                error!("Read error: {}", e);
                break;
            }
        }
    }
    info!("Client disconnected");
    Ok(())
}

async fn handle_send_message(
    content: String,
    state: &Arc<Mutex<State>>,
    config: &Config,
    writer: &mut (impl tokio::io::AsyncWrite + Unpin),
) -> anyhow::Result<()> {
    // ── 1. Add user message to state, collect LLM context ────────────────────

    // Quick guard (separate lock so no guard crosses an await)
    if state.lock().unwrap().active_chat().is_none() {
        let out = encode(&Response::Error {
            msg: "No active chat — create one first (Ctrl+N)".into(),
        });
        writer.write_all(out.as_bytes()).await?;
        return Ok(());
    }

    // Check for slash commands before hitting the LLM
    if let Some(cmd) = hyprcollab::commands::parse(&content) {
        // Handle /clear specially so we can pass the folder_id
        let output = match &cmd {
            hyprcollab::commands::SlashCommand::Clear => {
                let folder_id = state.lock().unwrap().active_folder_id();
                match folder_id {
                    Some(fid) => match hyprcollab::rag::clear_index(&fid) {
                        Ok(n) => format!("RAG index cleared. {} chunks removed.", n),
                        Err(e) => format!("Failed to clear RAG index: {}", e),
                    },
                    None => "No active folder.".into(),
                }
            }
            _ => hyprcollab::commands::execute(cmd).await,
        };
        let now = now_secs();
        let resp_tokens = (output.len() / 4) as u32;
        let assistant_msg = Message {
            role: "assistant".into(),
            content: output.clone(),
            timestamp: now,
            tokens: resp_tokens,
        };
        let snapshot = {
            let mut s = state.lock().unwrap();
            let user_tokens = (content.len() / 4) as u32;
            let user_msg = Message { role: "user".into(), content: content.clone(), timestamp: now, tokens: user_tokens };
            if let Some(chat) = s.active_chat_mut() {
                chat.messages.push(user_msg.clone());
                chat.messages.push(assistant_msg.clone());
                chat.tokens_used += user_tokens as u64 + resp_tokens as u64;
            }
            let folder_id = s.active_folder_id();
            let chat_id = s.active_chat_id();
            if let (Some(ref fid), Some(ref cid)) = (&folder_id, &chat_id) {
                let _ = hyprcollab::storage::messages::append_message(fid, cid, &user_msg);
                let _ = hyprcollab::storage::messages::append_message(fid, cid, &assistant_msg);
            }
            s.snapshot()
        };
        let out = encode(&Response::State { data: snapshot });
        writer.write_all(out.as_bytes()).await?;
        return Ok(());
    }

    let (folder_id, chat_id, history, model, base_url, api_key, chat_system_prompt) = {
        let mut s = state.lock().unwrap();

        let model = {
            // Prefer model from agent config; fall back to active model in UI
            let active = s
                .models
                .get(s.active_model_idx)
                .map(|m| m.name.clone())
                .unwrap_or_else(|| "openai/gpt-4o-mini".into());
            agent_cfg(config)
                .and_then(|a| a.model.clone())
                .unwrap_or(active)
        };

        let (base_url, api_key) = credentials(config);
        let folder_id = s.active_folder_id();
        let chat_id = s.active_chat_id();
        let chat_system_prompt = s.active_chat().and_then(|c| c.system_prompt.clone());

        let now = now_secs();
        let user_tokens = (content.len() / 4) as u32;
        let user_msg = Message {
            role: "user".into(),
            content: content.clone(),
            timestamp: now,
            tokens: user_tokens,
        };

        if let Some(chat) = s.active_chat_mut() {
            chat.messages.push(user_msg.clone());
            chat.tokens_used += user_tokens as u64;
        }

        if let (Some(ref fid), Some(ref cid)) = (&folder_id, &chat_id) {
            if let Err(e) = hyprcollab::storage::messages::append_message(fid, cid, &user_msg) {
                warn!("persist user msg: {}", e);
            }
        }

        // Mark busy before spawning LLM task
        s.working = true;
        if let Err(e) = waybar::publish_from_state(&s) {
            warn!("waybar publish: {}", e);
        }

        let history: Vec<ChatMessage> = s
            .active_chat()
            .map(|c| {
                c.messages
                    .iter()
                    .map(|m| ChatMessage { role: m.role.clone(), content: m.content.clone() })
                    .collect()
            })
            .unwrap_or_default();

        (folder_id, chat_id, history, model, base_url, api_key, chat_system_prompt)
    };

    // ── 1.5. RAG: search for context relevant to this query ───────────────────
    let base_prompt = chat_system_prompt
        .as_deref()
        .unwrap_or("You are a helpful assistant.");

    let rag_system_prompt = if config.rag.chunk_size > 0 {
        let (embed_base_url, embed_api_key) = credentials(config);
        let embedder = Embedder::new(&embed_base_url, &embed_api_key, &config.rag.embedder);
        if let Some(ref fid) = folder_id {
            match hyprcollab::rag::search_context(fid, &content, &embedder, &config.rag).await {
                Ok(ctx) if !ctx.is_empty() => format!("{}\n\n{}", base_prompt, ctx),
                Ok(_) => base_prompt.to_string(),
                Err(e) => {
                    warn!("RAG search error (non-fatal): {}", e);
                    base_prompt.to_string()
                }
            }
        } else {
            base_prompt.to_string()
        }
    } else {
        base_prompt.to_string()
    };

    // Prepend system message to history
    let mut history_with_system = vec![
        ChatMessage { role: "system".into(), content: rag_system_prompt },
    ];
    history_with_system.extend(history);

    // ── 2. Spawn LLM stream ───────────────────────────────────────────────────
    let (tx, mut rx) = mpsc::channel::<AgentEvent>(64);

    tokio::spawn(async move {
        let connector = OpenAIConnector::new(base_url, api_key);
        if let Err(e) = connector.stream(history_with_system, model, tx.clone()).await {
            let _ = tx.send(AgentEvent::Error(e.to_string())).await;
        }
    });

    // ── 3. Forward tokens to client ───────────────────────────────────────────
    let mut full_content = String::new();
    let mut tokens_used = 0u32;

    while let Some(event) = rx.recv().await {
        match event {
            AgentEvent::Token(text) => {
                full_content.push_str(&text);
                let out = encode(&Response::Token { text });
                writer.write_all(out.as_bytes()).await?;
            }
            AgentEvent::Complete { tokens_used: tu } => {
                tokens_used = tu;
                break;
            }
            AgentEvent::Error(msg) => {
                {
                    let mut s = state.lock().unwrap();
                    s.working = false;
                    if let Err(e) = waybar::publish_from_state(&s) {
                        warn!("waybar publish: {}", e);
                    }
                }
                let out = encode(&Response::Error { msg });
                writer.write_all(out.as_bytes()).await?;
                return Ok(());
            }
        }
    }

    // ── 4. Persist assistant message + send final state ───────────────────────
    let resp_tokens = if tokens_used > 0 {
        tokens_used
    } else {
        (full_content.len() / 4) as u32
    };

    let assistant_msg = Message {
        role: "assistant".into(),
        content: full_content.clone(),
        timestamp: now_secs(),
        tokens: resp_tokens,
    };

    let snapshot = {
        let mut s = state.lock().unwrap();

        if let Some(chat) = s.active_chat_mut() {
            chat.messages.push(assistant_msg.clone());
            chat.tokens_used += resp_tokens as u64;
        }

        if let (Some(ref fid), Some(ref cid)) = (&folder_id, &chat_id) {
            if let Err(e) = hyprcollab::storage::messages::append_message(fid, cid, &assistant_msg) {
                warn!("persist assistant msg: {}", e);
            }
        }

        s.working = false;
        if let Err(e) = waybar::publish_from_state(&s) {
            warn!("waybar publish: {}", e);
        }

        s.snapshot()
    };

    // ── 4.5. RAG: index the assistant response (non-fatal) ───────────────────
    if config.rag.chunk_size > 0 {
        let (embed_base_url, embed_api_key) = credentials(config);
        let embedder = Embedder::new(&embed_base_url, &embed_api_key, &config.rag.embedder);
        if let Some(ref fid) = folder_id {
            if let Some(ref cid) = chat_id {
                if let Err(e) = hyprcollab::rag::index_message(fid, cid, &assistant_msg, &embedder, &config.rag).await {
                    warn!("RAG index error (non-fatal): {}", e);
                }
            }
        }
    }

    let out = encode(&Response::State { data: snapshot });
    writer.write_all(out.as_bytes()).await?;
    Ok(())
}

fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

// ── Config helpers ────────────────────────────────────────────────────────────

fn agent_cfg(config: &Config) -> Option<&hyprcollab::storage::AgentConfig> {
    config.agents.iter().find(|a| a.agent_type == "openai")
}

fn credentials(config: &Config) -> (String, String) {
    let cfg = agent_cfg(config);
    let base_url = cfg
        .and_then(|a| a.base_url.as_deref())
        .map(String::from)
        .or_else(|| std::env::var("OPENAI_BASE_URL").ok())
        .unwrap_or_else(|| "https://openrouter.ai/api/v1".into());

    let api_key = cfg
        .and_then(|a| a.api_key.as_deref())
        .map(String::from)
        .or_else(|| std::env::var("OPENAI_API_KEY").ok())
        .unwrap_or_default();

    (base_url, api_key)
}
