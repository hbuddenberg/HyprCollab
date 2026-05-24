use dioxus::prelude::*;
use serde::{Deserialize, Serialize};

// ── SSE event types ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SseEvent {
    Token { text: String },
    ToolCall { name: String, args: serde_json::Value },
    ToolResult { output: String },
    Thinking { text: String },
    ApprovalRequest {
        tool_name: String,
        args: serde_json::Value,
        risk: String,
    },
    Done,
}

// ── Hook ─────────────────────────────────────────────────────────────────────

/// Connects to an SSE endpoint and returns a reactive list of events.
///
/// On WASM the hook spawns a future that reads from the EventSource and pushes
/// parsed `SseEvent` values into the signal.  On non-WASM targets the hook
/// compiles to a no-op that returns an empty signal.
pub fn use_sse_stream(url: String) -> Signal<Vec<SseEvent>> {
    let events: Signal<Vec<SseEvent>> = use_signal(Vec::new);

    #[cfg(target_arch = "wasm32")]
    {
        let events_clone = events;
        use_future(move || {
            let url = url.clone();
            async move {
                sse_connect(url, events_clone).await;
            }
        });
    }

    // Prevent unused-variable warning on non-WASM targets
    #[cfg(not(target_arch = "wasm32"))]
    drop(url);

    events
}

// ── WASM implementation ───────────────────────────────────────────────────────

#[cfg(target_arch = "wasm32")]
async fn sse_connect(url: String, mut events: Signal<Vec<SseEvent>>) {
    use futures::StreamExt;
    use gloo_net::eventsource::futures::EventSource;

    let event_types = [
        "token",
        "tool_call",
        "tool_result",
        "thinking",
        "approval_request",
        "done",
    ];

    loop {
        let Ok(mut source) = EventSource::new(&url) else {
            break;
        };

        let streams: Vec<_> = event_types
            .iter()
            .filter_map(|t| source.subscribe(t).ok())
            .collect();

        if streams.is_empty() {
            break;
        }

        let mut merged = futures::stream::select_all(streams);

        while let Some(Ok((_event_type, msg))) = merged.next().await {
            let data = msg.data().as_string().unwrap_or_default();
            if let Ok(event) = serde_json::from_str::<SseEvent>(&data) {
                events.write().push(event);
            }
        }

        // Stream ended — reconnect (browser SSE spec: retry after ~3 s).
        // We drop and re-create the EventSource in the next loop iteration.
    }
}
