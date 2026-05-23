use super::{AgentConnector, AgentEvent, ChatMessage, StreamFut};
use futures_util::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tracing::warn;

pub struct OpenAIConnector {
    base_url: String,
    api_key: String,
    client: Client,
}

impl OpenAIConnector {
    pub fn new(base_url: impl Into<String>, api_key: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            api_key: api_key.into(),
            client: Client::new(),
        }
    }
}

// ── API request types ────────────────────────────────────────────────────────

#[derive(Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ApiMsg>,
    stream: bool,
    stream_options: StreamOpts,
}

#[derive(Serialize)]
struct ApiMsg {
    role: String,
    content: String,
}

#[derive(Serialize)]
struct StreamOpts {
    include_usage: bool,
}

// ── SSE response types ────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct StreamChunk {
    choices: Vec<StreamChoice>,
    usage: Option<Usage>,
}

#[derive(Deserialize)]
struct StreamChoice {
    delta: Delta,
}

#[derive(Deserialize)]
struct Delta {
    /// Main response content
    content: Option<String>,
    /// Internal reasoning / chain-of-thought (Z.AI glm models)
    reasoning_content: Option<String>,
}

#[derive(Deserialize)]
struct Usage {
    completion_tokens: u32,
}

// ── Implementation ────────────────────────────────────────────────────────────

impl AgentConnector for OpenAIConnector {
    fn stream(&self, messages: Vec<ChatMessage>, model: String, tx: mpsc::Sender<AgentEvent>) -> StreamFut {
        let base_url = self.base_url.clone();
        let api_key = self.api_key.clone();
        let client = self.client.clone();

        Box::pin(async move {
            let api_messages: Vec<ApiMsg> = messages
                .into_iter()
                .map(|m| ApiMsg { role: m.role, content: m.content })
                .collect();

            let body = ChatRequest {
                model,
                messages: api_messages,
                stream: true,
                stream_options: StreamOpts { include_usage: true },
            };

            let response = match client
                .post(format!("{}/chat/completions", base_url))
                .header("Authorization", format!("Bearer {}", api_key))
                .json(&body)
                .send()
                .await
            {
                Ok(r) => r,
                Err(e) => {
                    let _ = tx.send(AgentEvent::Error(e.to_string())).await;
                    return Ok(());
                }
            };

            if !response.status().is_success() {
                let status = response.status();
                let body = response.text().await.unwrap_or_default();
                let _ = tx.send(AgentEvent::Error(format!("API {}: {}", status, body))).await;
                return Ok(());
            }

            let mut byte_stream = response.bytes_stream();
            let mut buf = String::new();
            let mut tokens_used = 0u32;
            // Track section transitions to emit visual headers once
            let mut emitted_reasoning_header = false;
            let mut emitted_content_header = false;

            while let Some(chunk) = byte_stream.next().await {
                let chunk = match chunk {
                    Ok(c) => c,
                    Err(e) => {
                        warn!("stream chunk error: {}", e);
                        break;
                    }
                };
                buf.push_str(&String::from_utf8_lossy(&chunk));

                // Drain complete SSE lines
                loop {
                    let Some(nl) = buf.find('\n') else { break };
                    let line = buf[..nl].trim().to_owned();
                    buf = buf[nl + 1..].to_owned();

                    if line.is_empty() || line.starts_with(':') {
                        continue;
                    }
                    let Some(data) = line.strip_prefix("data: ") else { continue };

                    if data == "[DONE]" {
                        let _ = tx.send(AgentEvent::Complete { tokens_used }).await;
                        return Ok(());
                    }

                    match serde_json::from_str::<StreamChunk>(data) {
                        Ok(ev) => {
                            if let Some(u) = ev.usage {
                                tokens_used = u.completion_tokens;
                            }
                            if let Some(ch) = ev.choices.first() {
                                // ── reasoning_content (chain-of-thought) ──────────────
                                if let Some(ref rc) = ch.delta.reasoning_content {
                                    if !rc.is_empty() {
                                        if !emitted_reasoning_header {
                                            emitted_reasoning_header = true;
                                            let _ = tx.send(AgentEvent::Token("🤔 ".into())).await;
                                        }
                                        let _ = tx.send(AgentEvent::Token(rc.clone())).await;
                                    }
                                }
                                // ── content (final answer) ────────────────────────────
                                if let Some(ref c) = ch.delta.content {
                                    if !c.is_empty() {
                                        if !emitted_content_header {
                                            emitted_content_header = true;
                                            // Separator only if reasoning was shown
                                            if emitted_reasoning_header {
                                                let _ = tx.send(AgentEvent::Token("\n\n─────\n".into())).await;
                                            }
                                        }
                                        let _ = tx.send(AgentEvent::Token(c.clone())).await;
                                    }
                                }
                            }
                        }
                        Err(e) => warn!("SSE parse error: {} | data: {}", e, data),
                    }
                }
            }

            let _ = tx.send(AgentEvent::Complete { tokens_used }).await;
            Ok(())
        })
    }
}
