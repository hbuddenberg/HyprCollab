//! Agent session – the primary user-facing API for running an agent.

use std::sync::Arc;

use hyprcollab_core::errors::Result;
use hyprcollab_core::traits::{LlmProvider, Tool};
use hyprcollab_core::types::*;

use crate::agent_loop::run_agent_loop;
use crate::tool_registry::ToolRegistry;
use crate::types::{AgentConfig, AgentOutput};

/// An interactive agent session that maintains conversation history and a tool
/// registry, and drives the LLM ↔ tool loop via an [`LlmProvider`].
pub struct AgentSession {
    /// Unique session identifier.
    pub session_id: String,
    /// Configuration (model, system prompt, turn limits, …).
    pub config: AgentConfig,
    /// Conversation history (system, user, assistant, tool messages).
    messages: Vec<Message>,
    /// Registered tools.
    registry: ToolRegistry,
    /// The LLM provider used for chat completions.
    provider: Arc<dyn LlmProvider>,
    /// Chat ID shared by all messages in this session.
    chat_id: ChatId,
    /// Optional approval engine consulted before each tool call.
    approval_engine: Option<hyprcollab_approval::ApprovalEngine>,
}

impl AgentSession {
    /// Create a new session backed by the given LLM provider.
    pub fn new(provider: Arc<dyn LlmProvider>) -> Self {
        Self {
            session_id: uuid::Uuid::new_v4().to_string(),
            config: AgentConfig::default(),
            messages: Vec::new(),
            registry: ToolRegistry::new(),
            provider,
            chat_id: ChatId::new(),
            approval_engine: None,
        }
    }

    /// Attach an approval engine (builder-style).
    pub fn with_approval_engine(mut self, engine: hyprcollab_approval::ApprovalEngine) -> Self {
        self.approval_engine = Some(engine);
        self
    }

    /// Set a custom system prompt (builder-style).
    pub fn with_system_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.config.system_prompt = prompt.into();
        self
    }

    /// Set the model identifier (builder-style).
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.config.model = model.into();
        self
    }

    /// Set the sampling temperature (builder-style).
    pub fn with_temperature(mut self, temp: f32) -> Self {
        self.config.temperature = Some(temp);
        self
    }

    /// Set the maximum number of turns (builder-style).
    pub fn with_max_turns(mut self, max_turns: u32) -> Self {
        self.config.max_turns = max_turns;
        self
    }

    /// Register a tool that the agent can invoke.
    pub fn register_tool(&mut self, tool: Box<dyn Tool>) {
        self.registry.register(tool);
    }

    /// Run one agent turn: send `user_message` to the LLM, execute any
    /// requested tool calls, and loop until the LLM responds with a final
    /// answer or the turn budget is exhausted.
    pub async fn run(&mut self, user_message: &str) -> Result<AgentOutput> {
        // Build the message list for this run.
        let mut messages: Vec<Message> = Vec::new();

        // Prepend system prompt only on the first turn (self.messages is empty).
        // On subsequent turns the system message is already in the history.
        if !self.config.system_prompt.is_empty() && self.messages.is_empty() {
            messages.push(Message {
                id: MessageId::new(),
                chat_id: self.chat_id,
                role: MessageRole::System,
                content: self.config.system_prompt.clone(),
                tool_calls: Vec::new(),
                artifacts: Vec::new(),
                timestamp: chrono::Utc::now(),
                metadata: serde_json::Value::Null,
            });
        }

        // Carry over the existing conversation history.
        messages.append(&mut self.messages);

        // Append the new user message.
        let user_msg = Message {
            id: MessageId::new(),
            chat_id: self.chat_id,
            role: MessageRole::User,
            content: user_message.to_string(),
            tool_calls: Vec::new(),
            artifacts: Vec::new(),
            timestamp: chrono::Utc::now(),
            metadata: serde_json::Value::Null,
        };
        messages.push(user_msg);

        // Run the agent loop.
        let output = run_agent_loop(
            self.provider.as_ref(),
            &mut messages,
            &self.registry,
            self.chat_id,
            &self.config,
            self.approval_engine.as_ref(),
        )
        .await?;

        // Persist the updated message history.
        self.messages = messages;

        Ok(output)
    }

    /// Read-only access to the conversation history.
    pub fn messages(&self) -> &[Message] {
        &self.messages
    }

    /// Read-only access to the chat ID.
    pub fn chat_id(&self) -> ChatId {
        self.chat_id
    }

    /// Read-only access to the tool registry.
    pub fn registry(&self) -> &ToolRegistry {
        &self.registry
    }
}
