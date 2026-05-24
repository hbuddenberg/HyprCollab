//! SSE stream parser for OpenAI streaming chat completions.

use futures::stream::Stream;
use futures::StreamExt;
use reqwest::Client;
use tokio_stream::wrappers::ReceiverStream;

use hyprcollab_core::errors::{CoreError, Result};
use hyprcollab_core::types::{FinishReason, TokenChunk, TokenUsage, ToolCall};

use crate::types::{OpenAiRequest, OpenAiStreamChunk};

/// Accumulator for a single tool call being assembled across streaming chunks.
#[derive(Default)]
struct PartialToolCall {
    id: String,
    name: String,
    arguments: String,
}

/// Creates a boxed stream of `TokenChunk` from a streaming OpenAI request.
pub fn create_stream(
    http: Client,
    url: String,
    auth: String,
    request: OpenAiRequest,
) -> impl Stream<Item = Result<TokenChunk>> + Send {
    let (tx, rx) = tokio::sync::mpsc::channel::<Result<TokenChunk>>(256);

    tokio::spawn(async move {
        let send_result = run_stream(http, url, auth, request, &tx).await;
        if let Err(e) = send_result {
            // If the channel is closed, that's fine (consumer dropped).
            let _ = tx.send(Err(e)).await;
        }
    });

    ReceiverStream::new(rx)
}

async fn run_stream(
    http: Client,
    url: String,
    auth: String,
    request: OpenAiRequest,
    tx: &tokio::sync::mpsc::Sender<Result<TokenChunk>>,
) -> Result<()> {
    use eventsource_stream::Eventsource;

    let resp = http
        .post(&url)
        .header("Authorization", auth)
        .header("Content-Type", "application/json")
        .json(&request)
        .send()
        .await
        .map_err(|e| CoreError::Llm(format!("Stream request failed: {e}")))?;

    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(CoreError::Llm(format!(
            "OpenAI streaming error ({}): {}",
            status, body
        )));
    }

    let mut event_stream = resp.bytes_stream().eventsource();
    // Buffer for assembling incremental tool call fragments across SSE chunks.
    let mut partial_tool_calls: Vec<PartialToolCall> = Vec::new();

    while let Some(event) = event_stream.next().await {
        match event {
            Ok(event) => {
                let data = event.data.trim();

                if data == "[DONE]" {
                    break;
                }
                if data.is_empty() {
                    continue;
                }

                match serde_json::from_str::<OpenAiStreamChunk>(data) {
                    Ok(chunk) => {
                        let usage = chunk.usage.map(|u| TokenUsage {
                            prompt_tokens: u.prompt_tokens,
                            completion_tokens: u.completion_tokens,
                            total_tokens: u.total_tokens,
                        });

                        let choice = match chunk.choices.into_iter().next() {
                            Some(c) => c,
                            None => continue,
                        };

                        // Accumulate tool call fragments by index.
                        for tc_delta in choice.delta.tool_calls {
                            let idx = tc_delta.index;
                            while partial_tool_calls.len() <= idx {
                                partial_tool_calls.push(PartialToolCall::default());
                            }
                            let acc = &mut partial_tool_calls[idx];
                            if let Some(id) = tc_delta.id {
                                acc.id = id;
                            }
                            if let Some(func) = tc_delta.function {
                                if let Some(name) = func.name {
                                    acc.name = name;
                                }
                                if let Some(args) = func.arguments {
                                    acc.arguments.push_str(&args);
                                }
                            }
                        }

                        // Emit content text deltas as they arrive.
                        if let Some(text) = choice.delta.content
                            && !text.is_empty()
                        {
                            let tc = TokenChunk {
                                delta: text,
                                finish_reason: None,
                                usage: None,
                                tool_calls: vec![],
                            };
                            if tx.send(Ok(tc)).await.is_err() {
                                break;
                            }
                        }

                        // On finish, emit the terminal chunk with assembled tool calls.
                        if choice.finish_reason.is_some() {
                            let finish_reason = match choice.finish_reason.as_deref() {
                                Some("stop") => FinishReason::Stop,
                                Some("tool_calls") => FinishReason::ToolCalls,
                                Some("length") => FinishReason::Length,
                                Some("content_filter") => FinishReason::ContentFilter,
                                _ => FinishReason::Stop,
                            };

                            let assembled: Vec<ToolCall> = if finish_reason == FinishReason::ToolCalls {
                                partial_tool_calls
                                    .drain(..)
                                    .map(|acc| {
                                        let arguments = serde_json::from_str(&acc.arguments)
                                            .unwrap_or_else(|e| {
                                                tracing::warn!(
                                                    "malformed tool call arguments for {}: {}",
                                                    acc.name,
                                                    e
                                                );
                                                serde_json::Value::Null
                                            });
                                        ToolCall {
                                            id: acc.id,
                                            name: acc.name,
                                            arguments,
                                        }
                                    })
                                    .collect()
                            } else {
                                vec![]
                            };

                            let tc = TokenChunk {
                                delta: String::new(),
                                finish_reason: Some(finish_reason),
                                usage,
                                tool_calls: assembled,
                            };
                            if tx.send(Ok(tc)).await.is_err() {
                                break;
                            }
                        }
                    }
                    Err(e) => {
                        let _ = tx
                            .send(Err(CoreError::Llm(format!(
                                "Failed to parse stream chunk: {e}"
                            ))))
                            .await;
                        break;
                    }
                }
            }
            Err(e) => {
                let _ = tx
                    .send(Err(CoreError::Llm(format!("SSE parse error: {e}"))))
                    .await;
                break;
            }
        }
    }

    Ok(())
}
