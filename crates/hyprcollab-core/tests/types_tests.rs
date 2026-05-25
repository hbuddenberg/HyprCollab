use hyprcollab_core::errors::CoreError;
use hyprcollab_core::types::*;

#[test]
fn id_newtypes_generate_unique() {
    let a = ChatId::new();
    let b = ChatId::new();
    assert_ne!(a, b);
}

#[test]
fn id_newtypes_display() {
    let id = MessageId::new();
    assert!(!id.to_string().is_empty());
}

#[test]
fn id_newtypes_from_uuid_roundtrip() {
    let uuid = uuid::Uuid::new_v4();
    let chat_id = ChatId::from(uuid);
    let back: uuid::Uuid = chat_id.into();
    assert_eq!(uuid, back);
}

#[test]
fn message_role_serialization() {
    let role = MessageRole::Assistant;
    let json = serde_json::to_string(&role).unwrap();
    assert_eq!(json, "\"assistant\"");
    let parsed: MessageRole = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed, role);
}

#[test]
fn approval_mode_variants() {
    let modes = vec![
        ApprovalMode::Auto,
        ApprovalMode::Normal,
        ApprovalMode::Strict,
    ];
    for mode in modes {
        let json = serde_json::to_string(&mode).unwrap();
        let parsed: ApprovalMode = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, mode);
    }
}

#[test]
fn artifact_types() {
    let types = vec![
        ArtifactType::Code,
        ArtifactType::Markdown,
        ArtifactType::Html,
        ArtifactType::Image,
        ArtifactType::Mermaid,
        ArtifactType::File,
    ];
    for at in types {
        let json = serde_json::to_string(&at).unwrap();
        let parsed: ArtifactType = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, at);
    }
}

#[test]
fn message_construction() {
    let msg = Message {
        id: MessageId::new(),
        chat_id: ChatId::new(),
        role: MessageRole::User,
        content: "Hello HyprCollab!".into(),
        tool_calls: vec![],
        artifacts: vec![],
        timestamp: chrono::Utc::now(),
        metadata: serde_json::Value::Null,
        parent_id: None,
    };
    assert_eq!(msg.role, MessageRole::User);
    assert_eq!(msg.content, "Hello HyprCollab!");
    assert!(msg.tool_calls.is_empty());
}

#[test]
fn message_with_tool_calls() {
    let tc = ToolCall {
        id: "call_123".into(),
        name: "web_scrape".into(),
        arguments: serde_json::json!({"url": "https://example.com"}),
    };
    let msg = Message {
        id: MessageId::new(),
        chat_id: ChatId::new(),
        role: MessageRole::Assistant,
        content: String::new(),
        tool_calls: vec![tc],
        artifacts: vec![],
        timestamp: chrono::Utc::now(),
        metadata: serde_json::Value::Null,
        parent_id: None,
    };
    assert_eq!(msg.tool_calls.len(), 1);
    assert_eq!(msg.tool_calls[0].name, "web_scrape");
}

#[test]
fn chat_request_serialization_roundtrip() {
    let req = ChatRequest {
        model: "claude-sonnet-4".into(),
        messages: vec![],
        tools: vec![],
        temperature: Some(0.7),
        max_tokens: Some(4096),
        stream: true,
        persona: None,
        agent_role: None,
    };
    let json = serde_json::to_string(&req).unwrap();
    let parsed: ChatRequest = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.model, "claude-sonnet-4");
    assert_eq!(parsed.temperature, Some(0.7));
    assert!(parsed.stream);
}

#[test]
fn model_info_fields() {
    let info = ModelInfo {
        id: "anthropic/claude-sonnet-4".into(),
        name: "Claude Sonnet 4".into(),
        provider: "anthropic".into(),
        context_length: 200_000,
        supports_streaming: true,
        supports_tools: true,
        supports_vision: true,
    };
    assert_eq!(info.provider, "anthropic");
    assert_eq!(info.context_length, 200_000);
}

#[test]
fn core_error_variants() {
    let err = CoreError::Llm("timeout".into());
    assert!(err.to_string().contains("LLM provider error"));

    let err = CoreError::Persona("not found".into());
    assert!(err.to_string().contains("Persona error"));
}

#[test]
fn token_usage_calculates_total() {
    let usage = TokenUsage {
        prompt_tokens: 100,
        completion_tokens: 50,
        total_tokens: 150,
    };
    assert_eq!(
        usage.total_tokens,
        usage.prompt_tokens + usage.completion_tokens
    );
}

#[test]
fn finish_reason_serialization() {
    let reasons = vec![
        FinishReason::Stop,
        FinishReason::ToolCalls,
        FinishReason::Length,
        FinishReason::ContentFilter,
    ];
    for reason in reasons {
        let json = serde_json::to_string(&reason).unwrap();
        let parsed: FinishReason = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, reason);
    }
}

#[test]
fn artifact_with_language() {
    let art = Artifact {
        id: "art_1".into(),
        artifact_type: ArtifactType::Code,
        title: "main.rs".into(),
        content: "fn main() {}".into(),
        language: Some("rust".into()),
    };
    let json = serde_json::to_string(&art).unwrap();
    assert!(json.contains("rust"));
    let parsed: Artifact = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.language, Some("rust".into()));
}

#[test]
fn tool_result_error_flag() {
    let ok = ToolResult {
        tool_call_id: "c1".into(),
        content: "done".into(),
        is_error: false,
    };
    let err = ToolResult {
        tool_call_id: "c2".into(),
        content: "fail".into(),
        is_error: true,
    };
    assert!(!ok.is_error);
    assert!(err.is_error);
}
