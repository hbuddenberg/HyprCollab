//! Tests for the agent runtime core.

use std::collections::VecDeque;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use hyprcollab_core::errors::Result;
use hyprcollab_core::traits::{LlmProvider, Tool};
use hyprcollab_core::types::*;

use crate::session::AgentSession;
use crate::tool_registry::ToolRegistry;
use crate::types::AgentConfig;

// ── Mock Tool ────────────────────────────────────────────────────────

/// A mock tool that returns a fixed string and records its invocations.
struct MockTool {
    name: String,
    description: String,
    return_value: String,
    calls: Arc<Mutex<Vec<serde_json::Value>>>,
}

impl MockTool {
    fn new(name: &str, return_value: &str) -> Self {
        Self {
            name: name.to_string(),
            description: format!("Mock tool: {name}"),
            return_value: return_value.to_string(),
            calls: Arc::new(Mutex::new(Vec::new())),
        }
    }

        #[allow(dead_code)]
        pub fn call_count(&self) -> usize {
        self.calls.lock().unwrap().len()
    }
}

#[async_trait]
impl Tool for MockTool {
    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "input": { "type": "string" }
            }
        })
    }

    async fn execute(&self, args: serde_json::Value) -> Result<String> {
        self.calls.lock().unwrap().push(args);
        Ok(self.return_value.clone())
    }
}

// ── Mock Provider ────────────────────────────────────────────────────

/// A mock LLM provider that returns a sequence of pre-programmed responses.
struct MockProvider {
    responses: Mutex<VecDeque<ChatResponse>>,
}

impl MockProvider {
    fn new(responses: Vec<ChatResponse>) -> Self {
        Self {
            responses: Mutex::new(VecDeque::from(responses)),
        }
    }

    /// Build a response that finishes with Stop and contains `content`.
    fn stop_response(content: &str) -> ChatResponse {
        ChatResponse {
            id: uuid::Uuid::new_v4().to_string(),
            message: Message {
                id: MessageId::new(),
                chat_id: ChatId::new(),
                role: MessageRole::Assistant,
                content: content.to_string(),
                tool_calls: Vec::new(),
                artifacts: Vec::new(),
                timestamp: chrono::Utc::now(),
                metadata: serde_json::Value::Null,
                parent_id: None,
            },
            usage: TokenUsage {
                prompt_tokens: 10,
                completion_tokens: 20,
                total_tokens: 30,
            },
            model: "mock/model".to_string(),
            finish_reason: FinishReason::Stop,
        }
    }

    /// Build a response that requests one tool call.
    fn tool_call_response(tool_name: &str, tool_args: serde_json::Value) -> ChatResponse {
        ChatResponse {
            id: uuid::Uuid::new_v4().to_string(),
            message: Message {
                id: MessageId::new(),
                chat_id: ChatId::new(),
                role: MessageRole::Assistant,
                content: String::new(),
                tool_calls: vec![ToolCall {
                    id: uuid::Uuid::new_v4().to_string(),
                    name: tool_name.to_string(),
                    arguments: tool_args,
                }],
                artifacts: Vec::new(),
                timestamp: chrono::Utc::now(),
                metadata: serde_json::Value::Null,
                parent_id: None,
            },
            usage: TokenUsage {
                prompt_tokens: 10,
                completion_tokens: 5,
                total_tokens: 15,
            },
            model: "mock/model".to_string(),
            finish_reason: FinishReason::ToolCalls,
        }
    }
}

#[async_trait]
impl LlmProvider for MockProvider {
    async fn chat_completion(&self, _request: ChatRequest) -> Result<ChatResponse> {
        let mut queue = self.responses.lock().unwrap();
        queue
            .pop_front()
            .ok_or_else(|| hyprcollab_core::errors::CoreError::Llm("No more mock responses".into()))
    }

    fn chat_stream(
        &self,
        _request: ChatRequest,
    ) -> Pin<Box<dyn futures::Stream<Item = Result<TokenChunk>> + Send>> {
        Box::pin(futures::stream::empty())
    }

    async fn embeddings(&self, _input: &str) -> Result<Vec<f32>> {
        Ok(vec![0.0; 128])
    }

    async fn models(&self) -> Result<Vec<ModelInfo>> {
        Ok(vec![])
    }

    fn name(&self) -> &str {
        "mock"
    }

    fn supports_streaming(&self) -> bool {
        false
    }

    fn supports_tools(&self) -> bool {
        true
    }
}

// ── Tests ────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_agent_session_new() {
    let provider = Arc::new(MockProvider::new(vec![]));
    let session = AgentSession::new(provider);
    assert!(!session.session_id.is_empty());
    assert!(session.messages().is_empty());
    assert!(session.registry().is_empty());
}

#[tokio::test]
async fn test_agent_session_with_system_prompt() {
    let provider = Arc::new(MockProvider::new(vec![]));
    let session = AgentSession::new(provider).with_system_prompt("You are a helpful assistant.");
    assert_eq!(session.config.system_prompt, "You are a helpful assistant.");
}

#[tokio::test]
async fn test_agent_session_with_model() {
    let provider = Arc::new(MockProvider::new(vec![]));
    let session = AgentSession::new(provider).with_model("anthropic/claude-sonnet-4");
    assert_eq!(session.config.model, "anthropic/claude-sonnet-4");
}

#[tokio::test]
async fn test_agent_session_register_tool() {
    let provider = Arc::new(MockProvider::new(vec![]));
    let mut session = AgentSession::new(provider);
    let tool = MockTool::new("calculator", "42");
    session.register_tool(Box::new(tool));
    assert_eq!(session.registry().len(), 1);
}

#[test]
fn test_tool_registry_register_and_get() {
    let mut registry = ToolRegistry::new();
    let tool = MockTool::new("read_file", "file contents");
    registry.register(Box::new(tool));
    assert!(registry.get("read_file").is_some());
    assert!(registry.get("nonexistent").is_none());
}

#[test]
fn test_tool_registry_list() {
    let mut registry = ToolRegistry::new();
    registry.register(Box::new(MockTool::new("tool_a", "a")));
    registry.register(Box::new(MockTool::new("tool_b", "b")));
    let list = registry.list();
    assert_eq!(list.len(), 2);
}

#[test]
fn test_tool_registry_definitions() {
    let mut registry = ToolRegistry::new();
    registry.register(Box::new(MockTool::new("shell", "output")));
    let defs = registry.tool_definitions();
    assert_eq!(defs.len(), 1);
    assert_eq!(defs[0].name, "shell");
    assert!(!defs[0].description.is_empty());
}

#[tokio::test]
async fn test_agent_loop_stop_immediately() {
    let provider = Arc::new(MockProvider::new(vec![
        MockProvider::stop_response("Hello! How can I help?"),
    ]));
    let mut session = AgentSession::new(provider);
    let output = session.run("Hi there").await.unwrap();
    assert_eq!(output.final_response.message.content, "Hello! How can I help?");
    assert_eq!(output.turns_used, 1);
    assert!(output.tool_calls.is_empty());
}

#[tokio::test]
async fn test_agent_loop_tool_call_then_stop() {
    let provider = Arc::new(MockProvider::new(vec![
        MockProvider::tool_call_response("calculator", serde_json::json!({"input": "2+2"})),
        MockProvider::stop_response("The answer is 42."),
    ]));
    let mut session = AgentSession::new(provider);
    let tool = MockTool::new("calculator", "42");
    session.register_tool(Box::new(tool));
    let output = session.run("What is 2+2?").await.unwrap();
    assert_eq!(output.turns_used, 2);
    assert_eq!(output.tool_calls.len(), 1);
    assert_eq!(output.tool_calls[0].tool_name, "calculator");
    assert_eq!(output.tool_calls[0].result, "42");
}

#[tokio::test]
async fn test_agent_loop_multi_tool_calls() {
    // First call: 2 tool calls at once, then stop.
    let tc1 = ToolCall {
        id: "tc-1".to_string(),
        name: "read_file".to_string(),
        arguments: serde_json::json!({"path": "/tmp/a.txt"}),
    };
    let tc2 = ToolCall {
        id: "tc-2".to_string(),
        name: "shell".to_string(),
        arguments: serde_json::json!({"cmd": "ls"}),
    };
    let multi_response = ChatResponse {
        id: "multi-1".to_string(),
        message: Message {
            id: MessageId::new(),
            chat_id: ChatId::new(),
            role: MessageRole::Assistant,
            content: String::new(),
            tool_calls: vec![tc1, tc2],
            artifacts: Vec::new(),
            timestamp: chrono::Utc::now(),
            metadata: serde_json::Value::Null,
            parent_id: None,
        },
        usage: TokenUsage {
            prompt_tokens: 10,
            completion_tokens: 10,
            total_tokens: 20,
        },
        model: "mock/model".to_string(),
        finish_reason: FinishReason::ToolCalls,
    };

    let provider = Arc::new(MockProvider::new(vec![
        multi_response,
        MockProvider::stop_response("Here are the results."),
    ]));
    let mut session = AgentSession::new(provider);
    session.register_tool(Box::new(MockTool::new("read_file", "file content")));
    session.register_tool(Box::new(MockTool::new("shell", "file1\nfile2")));
    let output = session.run("Show me files and their contents").await.unwrap();
    assert_eq!(output.turns_used, 2);
    assert_eq!(output.tool_calls.len(), 2);
    assert_eq!(output.tool_calls[0].tool_name, "read_file");
    assert_eq!(output.tool_calls[1].tool_name, "shell");
}

#[tokio::test]
async fn test_agent_loop_max_turns_exceeded() {
    // Provider always returns tool calls — will exhaust the turn budget.
    let make_tc_response = || {
        MockProvider::tool_call_response("echo", serde_json::json!({"input": "loop"}))
    };
    let infinite: Vec<ChatResponse> = (0..20).map(|_| make_tc_response()).collect();
    let provider = Arc::new(MockProvider::new(infinite));
    let mut session = AgentSession::new(provider).with_max_turns(5);
    session.register_tool(Box::new(MockTool::new("echo", "echo!")));
    let result = session.run("Keep going").await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.to_string().contains("maximum turns"));
}

#[test]
fn test_agent_config_default() {
    let config = AgentConfig::default();
    assert_eq!(config.model, "openai/gpt-4o");
    assert!(config.system_prompt.is_empty());
    assert_eq!(config.max_turns, 10);
    assert!(config.temperature.is_none());
    assert_eq!(config.approval_mode, ApprovalMode::Auto);
}

#[test]
fn test_tool_registry_default() {
    let registry = ToolRegistry::default();
    assert!(registry.is_empty());
    assert_eq!(registry.len(), 0);
}
