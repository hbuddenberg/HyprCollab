//! Workspace-level integration tests for HyprCollab.
//!
//! Covers the full pipeline: Router → Agent → Tools → Commands → Approval.
//!
//! All LLM calls use in-process mocks — no real network traffic.

use std::collections::VecDeque;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use hyprcollab_core::errors::{CoreError, Result};
use hyprcollab_core::traits::{LlmProvider, SlashCommand, Tool};
use hyprcollab_core::types::*;

// ── Shared helpers ────────────────────────────────────────────────────────────

fn user_message(content: &str) -> Message {
    Message {
        id: MessageId::new(),
        chat_id: ChatId::new(),
        role: MessageRole::User,
        content: content.to_string(),
        tool_calls: vec![],
        artifacts: vec![],
        timestamp: chrono::Utc::now(),
        metadata: serde_json::Value::Null,
    }
}

fn chat_request(model: &str) -> ChatRequest {
    ChatRequest {
        model: model.to_string(),
        messages: vec![user_message("test")],
        tools: vec![],
        temperature: None,
        max_tokens: None,
        stream: false,
        persona: None,
        agent_role: None,
    }
}

// ── MockProvider ──────────────────────────────────────────────────────────────

struct MockProvider {
    provider_name: String,
    responses: Mutex<VecDeque<ChatResponse>>,
}

impl MockProvider {
    fn new(name: &str, responses: Vec<ChatResponse>) -> Self {
        Self {
            provider_name: name.to_string(),
            responses: Mutex::new(VecDeque::from(responses)),
        }
    }

    /// Build a terminal (Stop) response stamped with `provider_name` in the ID.
    fn stop_response(provider_name: &str, content: &str) -> ChatResponse {
        ChatResponse {
            id: format!("{provider_name}-resp"),
            message: Message {
                id: MessageId::new(),
                chat_id: ChatId::new(),
                role: MessageRole::Assistant,
                content: content.to_string(),
                tool_calls: vec![],
                artifacts: vec![],
                timestamp: chrono::Utc::now(),
                metadata: serde_json::Value::Null,
            },
            usage: TokenUsage { prompt_tokens: 5, completion_tokens: 10, total_tokens: 15 },
            model: "mock/model".to_string(),
            finish_reason: FinishReason::Stop,
        }
    }

    /// Build a response that requests one tool call.
    fn tool_call_response(tool_name: &str, args: serde_json::Value) -> ChatResponse {
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
                    arguments: args,
                }],
                artifacts: vec![],
                timestamp: chrono::Utc::now(),
                metadata: serde_json::Value::Null,
            },
            usage: TokenUsage { prompt_tokens: 5, completion_tokens: 5, total_tokens: 10 },
            model: "mock/model".to_string(),
            finish_reason: FinishReason::ToolCalls,
        }
    }
}

#[async_trait]
impl LlmProvider for MockProvider {
    async fn chat_completion(&self, _req: ChatRequest) -> Result<ChatResponse> {
        self.responses
            .lock()
            .unwrap()
            .pop_front()
            .ok_or_else(|| CoreError::Llm("MockProvider: response queue exhausted".into()))
    }

    fn chat_stream(
        &self,
        _req: ChatRequest,
    ) -> Pin<Box<dyn futures::Stream<Item = Result<TokenChunk>> + Send>> {
        Box::pin(futures::stream::empty())
    }

    async fn embeddings(&self, _input: &str) -> Result<Vec<f32>> {
        Ok(vec![])
    }

    async fn models(&self) -> Result<Vec<ModelInfo>> {
        Ok(vec![ModelInfo {
            id: "mock-model".to_string(),
            name: "Mock Model".to_string(),
            provider: self.provider_name.clone(),
            context_length: 4096,
            supports_streaming: false,
            supports_tools: true,
            supports_vision: false,
        }])
    }

    fn name(&self) -> &str {
        &self.provider_name
    }

    fn supports_streaming(&self) -> bool {
        false
    }

    fn supports_tools(&self) -> bool {
        true
    }
}

// ── MockTool ──────────────────────────────────────────────────────────────────

struct MockTool {
    tool_name: String,
    fixed_result: String,
}

impl MockTool {
    fn new(name: &str, result: &str) -> Self {
        Self { tool_name: name.to_string(), fixed_result: result.to_string() }
    }
}

#[async_trait]
impl Tool for MockTool {
    fn name(&self) -> &str {
        &self.tool_name
    }

    fn description(&self) -> &str {
        "A mock tool for integration testing"
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({"type": "object", "properties": {}})
    }

    async fn execute(&self, _args: serde_json::Value) -> Result<String> {
        Ok(self.fixed_result.clone())
    }
}

// ── MockSlashCommand ──────────────────────────────────────────────────────────

struct MockSlashCommand {
    cmd_name: String,
}

#[async_trait]
impl SlashCommand for MockSlashCommand {
    fn name(&self) -> &str {
        &self.cmd_name
    }

    fn description(&self) -> &str {
        "A mock slash command for integration testing"
    }

    async fn execute(&self, args: serde_json::Value, _ctx: &mut hyprcollab_core::types::CommandContext) -> Result<String> {
        let raw = args["raw"].as_str().unwrap_or("");
        Ok(format!("{}: {raw}", self.cmd_name.trim_start_matches('/')))
    }
}

// ── Test 1: Router resolves "openai/gpt-4o" to the openai provider ───────────

#[tokio::test]
async fn router_resolves_provider_model_format() {
    use hyprcollab_router::LlmRouter;

    let mut router = LlmRouter::new();
    router.register(
        "openai",
        Box::new(MockProvider::new(
            "openai",
            vec![MockProvider::stop_response("openai", "gpt-4o response")],
        )),
    );

    let resp = router.chat_completion(chat_request("openai/gpt-4o")).await.unwrap();

    assert_eq!(resp.id, "openai-resp", "openai provider should handle the request");
    assert!(resp.message.content.contains("gpt-4o"));
}

// ── Test 2: Router falls back to the default provider ────────────────────────

#[tokio::test]
async fn router_falls_back_to_default_provider() {
    use hyprcollab_router::LlmRouter;

    let mut router = LlmRouter::new();
    router.register(
        "anthropic",
        Box::new(MockProvider::new(
            "anthropic",
            vec![MockProvider::stop_response("anthropic", "fallback response")],
        )),
    );
    router.set_default("anthropic");

    // "claude-3" has no provider prefix — must fall back to anthropic
    let resp = router.chat_completion(chat_request("claude-3")).await.unwrap();

    assert_eq!(resp.id, "anthropic-resp");
    assert!(resp.message.content.contains("fallback"));
}

// ── Test 3: Agent session starts and completes (no tools) ─────────────────────

#[tokio::test]
async fn agent_session_completes_with_stop_response() {
    use hyprcollab_agent::AgentSession;

    let provider = Arc::new(MockProvider::new(
        "mock",
        vec![MockProvider::stop_response("mock", "Task done")],
    ));

    let mut session = AgentSession::new(provider)
        .with_model("mock/model")
        .with_system_prompt("You are a helpful assistant.");

    let output = session.run("Do something").await.unwrap();

    assert_eq!(output.turns_used, 1);
    assert!(output.tool_calls.is_empty());
    assert_eq!(output.final_response.message.content, "Task done");
    assert!(!session.messages().is_empty(), "messages should be recorded");
}

// ── Test 4: Agent calls a tool and returns result ─────────────────────────────

#[tokio::test]
async fn agent_calls_tool_and_returns_result() {
    use hyprcollab_agent::AgentSession;

    let provider = Arc::new(MockProvider::new(
        "mock",
        vec![
            MockProvider::tool_call_response("echo", serde_json::json!({"input": "ping"})),
            MockProvider::stop_response("mock", "Echo replied: pong"),
        ],
    ));

    let mut session = AgentSession::new(provider).with_model("mock/model");
    session.register_tool(Box::new(MockTool::new("echo", "pong")));

    let output = session.run("Echo ping").await.unwrap();

    assert_eq!(output.turns_used, 2);
    assert_eq!(output.tool_calls.len(), 1);
    assert_eq!(output.tool_calls[0].tool_name, "echo");
    assert_eq!(output.tool_calls[0].result, "pong");
    assert!(output.final_response.message.content.contains("pong"));
}

// ── Test 5: Parser handles /help, /model sonnet, /agent coder ────────────────

#[test]
fn parser_handles_help_command() {
    use hyprcollab_commands::ParsedCommand;

    let cmd = ParsedCommand::parse("/help").unwrap().unwrap();
    assert_eq!(cmd.name, "help");
    assert!(cmd.args.is_empty());
    assert!(cmd.flags.is_empty());
    assert_eq!(cmd.raw, "/help");
}

#[test]
fn parser_handles_model_with_arg() {
    use hyprcollab_commands::ParsedCommand;

    let cmd = ParsedCommand::parse("/model sonnet").unwrap().unwrap();
    assert_eq!(cmd.name, "model");
    assert_eq!(cmd.first_arg(), Some("sonnet"));
    assert_eq!(cmd.args_joined(), "sonnet");
}

#[test]
fn parser_handles_agent_with_arg() {
    use hyprcollab_commands::ParsedCommand;

    let cmd = ParsedCommand::parse("/agent coder").unwrap().unwrap();
    assert_eq!(cmd.name, "agent");
    assert_eq!(cmd.first_arg(), Some("coder"));
}

// ── Test 6: CommandRegistry registers and retrieves commands ──────────────────

#[test]
fn command_registry_registers_and_retrieves() {
    use hyprcollab_commands::CommandRegistry;

    let mut registry = CommandRegistry::new();
    registry.register(Box::new(MockSlashCommand { cmd_name: "/ping".to_string() }));

    // registry strips the leading "/" when inserting
    assert!(registry.get("ping").is_some(), "get by bare name should succeed");
    assert!(registry.get("/ping").is_none(), "keys don't include the slash");
    assert!(registry.get("missing").is_none());
    assert_eq!(registry.len(), 1);
    assert!(!registry.is_empty());
}

// ── Test 7: register_all() registers exactly 10 built-in commands ────────────

#[test]
fn register_all_registers_ten_builtin_commands() {
    use hyprcollab_commands::{builtins, CommandRegistry};

    let mut registry = CommandRegistry::new();
    builtins::register_all(&mut registry);

    assert_eq!(registry.len(), 12, "register_all should register exactly 12 commands");

    for name in ["help", "agent", "approval", "run", "skill", "config"] {
        assert!(registry.get(name).is_some(), "/{name} should be registered");
    }
}

// ── Test 8: ApprovalEngine — Auto mode approves everything ───────────────────

#[test]
fn approval_auto_mode_approves_all_tools() {
    use hyprcollab_approval::ApprovalEngine;

    let engine = ApprovalEngine::new(ApprovalMode::Auto);

    assert!(!engine.needs_approval("rm_rf", &serde_json::json!({"path": "/"})));
    assert!(!engine.needs_approval("web_fetch", &serde_json::json!({"url": "https://example.com"})));
    assert!(!engine.needs_approval("shell", &serde_json::json!({"command": "curl ..."})));
}

// ── Test 9: ApprovalEngine — Strict mode blocks everything ───────────────────

#[test]
fn approval_strict_mode_blocks_all_tools() {
    use hyprcollab_approval::ApprovalEngine;

    let engine = ApprovalEngine::new(ApprovalMode::Strict);

    assert!(engine.needs_approval("read_file", &serde_json::json!({})));
    assert!(engine.needs_approval("safe_noop", &serde_json::json!({})));
    assert!(engine.needs_approval("shell", &serde_json::json!({"command": "ls"})));
}

// ── Test 10: ShellTool — name, description, and schema are correct ────────────

#[test]
fn shell_tool_has_correct_name_and_description() {
    use hyprcollab_core::traits::Tool;
    use hyprcollab_tools::ShellTool;

    let tool = ShellTool::new();

    assert_eq!(tool.name(), "shell");
    assert!(!tool.description().is_empty());

    let params = tool.parameters();
    assert_eq!(params["type"].as_str(), Some("object"), "schema root must be an object");
    assert!(
        params["properties"]["command"].is_object(),
        "schema must declare a 'command' property"
    );
}

// ── Test 11: Full pipeline — parse command → configure session → run tool ─────

#[tokio::test]
async fn full_pipeline_parse_command_configure_agent_run_tool() {
    use hyprcollab_agent::AgentSession;
    use hyprcollab_commands::ParsedCommand;

    // Step 1 — extract model name from a slash command
    let cmd = ParsedCommand::parse("/model anthropic/claude-sonnet-4").unwrap().unwrap();
    assert_eq!(cmd.name, "model");
    let target_model = cmd.first_arg().unwrap_or("openai/gpt-4o").to_string();

    // Step 2 — build a provider that will request one tool call then stop
    let provider = Arc::new(MockProvider::new(
        "mock",
        vec![
            MockProvider::tool_call_response("greet", serde_json::json!({"name": "world"})),
            MockProvider::stop_response("mock", "Greeted the world successfully"),
        ],
    ));

    // Step 3 — configure the session using the model selected by the command
    let mut session = AgentSession::new(provider)
        .with_model(&target_model)
        .with_system_prompt("You are a greeter agent.")
        .with_max_turns(5);

    session.register_tool(Box::new(MockTool::new("greet", "Hello, world!")));

    // Step 4 — run and verify the full loop
    let output = session.run("Greet the world").await.unwrap();

    assert_eq!(output.turns_used, 2, "one tool-call turn + one final answer turn");
    assert_eq!(output.tool_calls.len(), 1);
    assert_eq!(output.tool_calls[0].tool_name, "greet");
    assert_eq!(output.tool_calls[0].result, "Hello, world!");
    assert_eq!(output.final_response.message.content, "Greeted the world successfully");
    assert_eq!(
        session.config.model, target_model,
        "session model should match what the command selected"
    );
}
