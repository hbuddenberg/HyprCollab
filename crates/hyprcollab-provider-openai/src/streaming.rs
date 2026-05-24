//! SSE stream parser for OpenAI streaming chat completions.

use futures::stream::Stream;
use futures::StreamExt;
use reqwest::Client;
use tokio_stream::wrappers::ReceiverStream;

use hyprcollab_core::errors::{CoreError, Result};
use hyprcollab_core::types::TokenChunk;

use crate::types::OpenAiRequest;
use crate::types::OpenAiStreamChunk;

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

    while let Some(event) = event_stream.next().await {
        match event {
            Ok(event) => {
                let data = event.data.trim();

                // End of stream signal
                if data == "[DONE]" {
                    break;
                }

                // Skip empty lines
                if data.is_empty() {
                    continue;
                }

                // Parse the JSON chunk
                match serde_json::from_str::<OpenAiStreamChunk>(data) {
                    Ok(chunk) => {
                        if let Some(token_chunk) = chunk.into_token_chunk() {
                            if tx.send(Ok(token_chunk)).await.is_err() {
                                // Consumer dropped, stop.
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
                    .send(Err(CoreError::Llm(format!(
                        "SSE parse error: {e}"
                    ))))
                    .await;
                break;
            }
        }
    }

    Ok(())
}
