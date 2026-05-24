//! Agent-specific types for the HyprCollab agent runtime.

use hyprcollab_core::types::*;
use serde::{Deserialize, Serialize};

/// The final output of an agent run, including the LLM response and a log of
/// every tool invocation that occurred during the loop.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentOutput {
    /// The terminal `ChatResponse` returned by the LLM (finish_reason == Stop,
    /// Length, or ContentFilter).
    pub final_response: ChatResponse,
    /// Ordered record of every tool call executed across all turns.
    pub tool_calls: Vec<TurnToolCall>,
    /// Number of LLM round-trips consumed (including the final one).
    pub turns_used: u32,
}

/// A single tool invocation recorded during the agent loop.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnToolCall {
    /// Which turn (1-based) this invocation belonged to.
    pub turn: u32,
    /// Name of the tool that was called.
    pub tool_name: String,
    /// JSON arguments passed to the tool.
    pub arguments: serde_json::Value,
    /// String result returned by the tool.
    pub result: String,
    /// Wall-clock execution time in milliseconds.
    pub duration_ms: u64,
}

/// Configuration for an agent session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    /// Model identifier (e.g. `"openai/gpt-4o"`).
    pub model: String,
    /// System prompt prepended to every conversation.
    pub system_prompt: String,
    /// Maximum number of LLM round-trips before aborting.
    pub max_turns: u32,
    /// Sampling temperature.
    pub temperature: Option<f32>,
    /// How tool-call approvals are handled.
    pub approval_mode: ApprovalMode,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            model: "openai/gpt-4o".to_string(),
            system_prompt: String::new(),
            max_turns: 10,
            temperature: None,
            approval_mode: ApprovalMode::Auto,
        }
    }
}
