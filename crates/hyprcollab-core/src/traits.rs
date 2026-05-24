use std::pin::Pin;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::errors::Result;
use crate::types::*;

/// A JSON-schema description of a tool's parameters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

// ── LlmProvider ──────────────────────────────────────────────────────

#[async_trait]
pub trait LlmProvider: Send + Sync {
    /// Send a non-streaming chat completion request.
    async fn chat_completion(&self, request: ChatRequest) -> Result<ChatResponse>;

    /// Send a streaming chat completion request.
    /// Returns a boxed stream of `TokenChunk` items.
    fn chat_stream(
        &self,
        request: ChatRequest,
    ) -> Pin<Box<dyn futures::Stream<Item = Result<TokenChunk>> + Send>>;

    /// Get embeddings for the given input text.
    async fn embeddings(&self, input: &str) -> Result<Vec<f32>>;

    /// List available models.
    async fn models(&self) -> Result<Vec<ModelInfo>>;

    /// Provider display name.
    fn name(&self) -> &str;

    /// Whether this provider supports streaming.
    fn supports_streaming(&self) -> bool;

    /// Whether this provider supports tool/function calling.
    fn supports_tools(&self) -> bool;
}

// ── Tool ─────────────────────────────────────────────────────────────

/// A tool that can be invoked by an LLM agent.
#[async_trait]
pub trait Tool: Send + Sync {
    /// Tool name (used by the LLM to invoke it).
    fn name(&self) -> &str;

    /// Human-readable description of what the tool does.
    fn description(&self) -> &str;

    /// JSON Schema describing the tool's expected parameters.
    fn parameters(&self) -> serde_json::Value;

    /// Execute the tool with the given arguments and return its output.
    async fn execute(&self, args: serde_json::Value) -> Result<String>;
}

// ── AcpConnector ─────────────────────────────────────────────────────

/// Connector for the Agent Communication Protocol (JSON-RPC over stdio).
#[async_trait]
pub trait AcpConnector: Send + Sync {
    /// Establish a connection to the agent process.
    async fn connect(&mut self) -> Result<()>;

    /// Disconnect from the agent process.
    async fn disconnect(&mut self) -> Result<()>;

    /// Send a JSON-RPC request and return the response.
    async fn send_request(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value>;

    /// Check whether the connector is currently connected.
    fn is_connected(&self) -> bool;
}

// ── PlatformAdapter ──────────────────────────────────────────────────

/// Platform-specific integrations (notifications, clipboard, URLs).
#[async_trait]
pub trait PlatformAdapter: Send + Sync {
    /// Send a message / notification to the user.
    async fn send_message(&self, title: &str, body: &str) -> Result<()>;

    /// Show a desktop notification.
    async fn show_notification(&self, title: &str, body: &str) -> Result<()>;

    /// Open a URL in the default browser / handler.
    async fn open_url(&self, url: &str) -> Result<()>;

    /// Write content to the system clipboard.
    async fn clipboard_write(&self, content: &str) -> Result<()>;
}

// ── SlashCommand ─────────────────────────────────────────────────────

/// A slash command (e.g. `/run`, `/read`, `/edit`).
#[async_trait]
pub trait SlashCommand: Send + Sync {
    /// Command name, including the leading `/`.
    fn name(&self) -> &str;

    /// Short description shown in help text.
    fn description(&self) -> &str;

    /// Alternative names that also route to this command.
    fn aliases(&self) -> Vec<&str> {
        vec![]
    }

    /// Tab-completion suggestions for the given partial input.
    fn completions(&self, _partial: &str) -> Vec<String> {
        vec![]
    }

    /// Execute the command.
    ///
    /// `args` is a JSON object with at minimum `"raw"` (the joined arg string)
    /// and `"args"` (positional tokens as an array). Commands perform their own
    /// validation and parsing inside this method.
    ///
    /// `ctx` holds session-scoped settings (model, temperature, approval_mode)
    /// and may be mutated by commands such as `/temperature` or `/approval`.
    async fn execute(&self, args: serde_json::Value, ctx: &mut CommandContext) -> Result<String>;
}
