use axum::response::sse::{Event, Sse};
use futures::stream::Stream;
use std::convert::Infallible;

use hyprcollab_core::types::{SseEvent, TokenUsage};

/// Wraps a stream of Event results into an Axum Sse response.
pub fn into_sse_stream<S>(stream: S) -> Sse<S>
where
    S: Stream<Item = Result<Event, Infallible>> + Send + 'static,
{
    Sse::new(stream)
}

/// Creates a standard text Event.
pub fn text_event(text: impl Into<String>) -> Event {
    Event::default().data(text.into())
}

/// Creates a JSON-serialized Event.
pub fn json_event<T: serde::Serialize>(data: &T) -> Result<Event, axum::Error> {
    Event::default().json_data(data)
}

/// Convert a `SseEvent` into an axum SSE `Event`.
///
/// A free function rather than a trait impl because `Event` is a foreign type
/// (axum) and `SseEvent` is a foreign type (hyprcollab-core), so the orphan
/// rule prevents `impl From<SseEvent> for Event` here.
pub fn sse_event_to_event(evt: SseEvent) -> Event {
    match evt {
        SseEvent::Token { content } => Event::default()
            .event("token")
            .json_data(serde_json::json!({"content": content}))
            .expect("SseEvent::Token serialization"),

        SseEvent::ToolCall { name, args, id } => Event::default()
            .event("tool_call")
            .json_data(serde_json::json!({"name": name, "args": args, "id": id}))
            .expect("SseEvent::ToolCall serialization"),

        SseEvent::ToolResult { id, output, duration_ms } => Event::default()
            .event("tool_result")
            .json_data(serde_json::json!({"id": id, "output": output, "duration_ms": duration_ms}))
            .expect("SseEvent::ToolResult serialization"),

        SseEvent::Thinking { content } => Event::default()
            .event("thinking")
            .json_data(serde_json::json!({"content": content}))
            .expect("SseEvent::Thinking serialization"),

        SseEvent::ApprovalRequest { tool, args, id } => Event::default()
            .event("approval_request")
            .json_data(serde_json::json!({"tool": tool, "args": args, "id": id}))
            .expect("SseEvent::ApprovalRequest serialization"),

        SseEvent::Done { usage: TokenUsage { prompt_tokens, completion_tokens, .. } } => {
            Event::default()
                .event("done")
                .json_data(serde_json::json!({
                    "usage": {
                        "prompt_tokens": prompt_tokens,
                        "completion_tokens": completion_tokens,
                    }
                }))
                .expect("SseEvent::Done serialization")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hyprcollab_core::types::TokenUsage;

    fn event_debug(event: Event) -> String {
        format!("{event:?}")
    }

    #[test]
    fn token_event_sets_event_type() {
        let evt = SseEvent::Token { content: "hello world".into() };
        let s = event_debug(sse_event_to_event(evt));
        assert!(s.contains("token"), "expected 'token' in {s}");
    }

    #[test]
    fn tool_call_sets_event_type() {
        let evt = SseEvent::ToolCall {
            name: "shell".into(),
            args: serde_json::json!({"cmd": "ls"}),
            id: "call_123".into(),
        };
        let s = event_debug(sse_event_to_event(evt));
        assert!(s.contains("tool_call"), "expected 'tool_call' in {s}");
    }

    #[test]
    fn tool_result_sets_event_type() {
        let evt =
            SseEvent::ToolResult { id: "call_123".into(), output: "file1\nfile2".into(), duration_ms: 150 };
        let s = event_debug(sse_event_to_event(evt));
        assert!(s.contains("tool_result"), "expected 'tool_result' in {s}");
    }

    #[test]
    fn thinking_sets_event_type() {
        let evt = SseEvent::Thinking { content: "Let me analyze...".into() };
        let s = event_debug(sse_event_to_event(evt));
        assert!(s.contains("thinking"), "expected 'thinking' in {s}");
    }

    #[test]
    fn approval_request_sets_event_type() {
        let evt = SseEvent::ApprovalRequest {
            tool: "shell".into(),
            args: serde_json::json!({"cmd": "rm -rf /"}),
            id: "req_456".into(),
        };
        let s = event_debug(sse_event_to_event(evt));
        assert!(s.contains("approval_request"), "expected 'approval_request' in {s}");
    }

    #[test]
    fn done_sets_event_type() {
        let evt =
            SseEvent::Done { usage: TokenUsage { prompt_tokens: 10, completion_tokens: 50, total_tokens: 60 } };
        let s = event_debug(sse_event_to_event(evt));
        assert!(s.contains("done"), "expected 'done' in {s}");
    }
}
