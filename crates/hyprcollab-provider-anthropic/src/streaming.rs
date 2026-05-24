//! SSE stream parser for Anthropic streaming Messages API.

use futures::stream::Stream;
use futures::StreamExt;
use reqwest::Client;
use tokio_stream::wrappers::ReceiverStream;

use hyprcollab_core::errors::{CoreError, Result};
use hyprcollab_core::types::{FinishReason, TokenChunk, TokenUsage};

use crate::types::*;

/// Creates a boxed stream of `TokenChunk` from a streaming Anthropic request.
pub fn create_stream(
    http: Client,
    url: String,
    api_key: String,
    api_version: String,
    request: AnthropicRequest,
) -> impl Stream<Item = Result<TokenChunk>> + Send {
    let (tx, rx) = tokio::sync::mpsc::channel::<Result<TokenChunk>>(256);

    tokio::spawn(async move {
        let send_result = run_stream(http, url, api_key, api_version, request, &tx).await;
        if let Err(e) = send_result {
            let _ = tx.send(Err(e)).await;
        }
    });

    ReceiverStream::new(rx)
}

async fn run_stream(
    http: Client,
    url: String,
    api_key: String,
    api_version: String,
    request: AnthropicRequest,
    tx: &tokio::sync::mpsc::Sender<Result<TokenChunk>>,
) -> Result<()> {
    use eventsource_stream::Eventsource;

    let resp = http
        .post(&url)
        .header("x-api-key", &api_key)
        .header("anthropic-version", &api_version)
        .header("Content-Type", "application/json")
        .json(&request)
        .send()
        .await
        .map_err(|e| CoreError::Llm(format!("Anthropic stream request failed: {e}")))?;

    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(CoreError::Llm(format!(
            "Anthropic streaming error ({}): {}",
            status, body
        )));
    }

    let mut event_stream = resp.bytes_stream().eventsource();

    while let Some(event) = event_stream.next().await {
        match event {
            Ok(sse_event) => {
                let event_type = sse_event.event;
                let data = sse_event.data.trim();

                // Skip empty data
                if data.is_empty() {
                    continue;
                }

                match event_type.as_str() {
                    "ping" => {
                        // Heartbeat, ignore
                        continue;
                    }
                    "message_start" => {
                        // Could extract initial usage info; for now skip
                        continue;
                    }
                    "content_block_start" => {
                        // Beginning of a content block, no text delta yet
                        continue;
                    }
                    "content_block_stop" => {
                        // End of a content block
                        continue;
                    }
                    "content_block_delta" => {
                        match serde_json::from_str::<AnthropicStreamContentBlockDelta>(data) {
                            Ok(delta_event) => {
                                let text = delta_event.delta.text.unwrap_or_default();
                                let partial_json = delta_event.delta.partial_json;

                                // If it's a text delta, emit it
                                if !text.is_empty() {
                                    let chunk = TokenChunk {
                                        delta: text,
                                        finish_reason: None,
                                        usage: None,
                                    tool_calls: vec![],
                                    };
                                    if tx.send(Ok(chunk)).await.is_err() {
                                        break;
                                    }
                                } else if partial_json.is_some() {
                                    // Tool input streaming — emit partial JSON as delta
                                    let chunk = TokenChunk {
                                        delta: partial_json.unwrap(),
                                        finish_reason: None,
                                        usage: None,
                                    tool_calls: vec![],
                                    };
                                    if tx.send(Ok(chunk)).await.is_err() {
                                        break;
                                    }
                                }
                            }
                            Err(e) => {
                                let _ = tx
                                    .send(Err(CoreError::Llm(format!(
                                        "Failed to parse content_block_delta: {e}"
                                    ))))
                                    .await;
                                break;
                            }
                        }
                    }
                    "message_delta" => {
                        match serde_json::from_str::<AnthropicStreamMessageDelta>(data) {
                            Ok(msg_delta) => {
                                let finish_reason = parse_stop_reason(&msg_delta.delta.stop_reason);
                                let usage = msg_delta.usage.map(|u| TokenUsage {
                                    prompt_tokens: 0, // input tokens were in message_start
                                    completion_tokens: u.output_tokens,
                                    total_tokens: u.output_tokens,
                                });

                                let chunk = TokenChunk {
                                    delta: String::new(),
                                    finish_reason: Some(finish_reason),
                                    usage,
                                    tool_calls: vec![],
                                };
                                if tx.send(Ok(chunk)).await.is_err() {
                                    break;
                                }
                            }
                            Err(e) => {
                                let _ = tx
                                    .send(Err(CoreError::Llm(format!(
                                        "Failed to parse message_delta: {e}"
                                    ))))
                                    .await;
                                break;
                            }
                        }
                    }
                    "message_stop" => {
                        // Stream is done
                        break;
                    }
                    "error" => {
                        let _ = tx
                            .send(Err(CoreError::Llm(format!(
                                "Anthropic stream error: {data}"
                            ))))
                            .await;
                        break;
                    }
                    _ => {
                        // Unknown event type, skip
                        continue;
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

fn parse_stop_reason(s: &Option<String>) -> FinishReason {
    match s.as_deref() {
        Some("end_turn") | Some("stop") => FinishReason::Stop,
        Some("tool_use") => FinishReason::ToolCalls,
        Some("max_tokens") => FinishReason::Length,
        _ => FinishReason::Stop,
    }
}
