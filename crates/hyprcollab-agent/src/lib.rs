//! # hyprcollab-agent
//!
//! Agent runtime core: LLM → tool call → execute → loop.
//!
//! The primary entry-point is [`AgentSession`], which owns a conversation
//! history, a tool registry, and a reference to an [`LlmProvider`].  Calling
//! [`AgentSession::run`] sends the user's message to the LLM, executes any
//! tool calls the model requests, appends the results, and loops until the
//! model returns a final answer or the turn budget is exhausted.

pub mod agent_loop;
pub mod session;
pub mod tool_registry;
pub mod types;

pub use session::AgentSession;
pub use tool_registry::ToolRegistry;
pub use types::{AgentConfig, AgentOutput, TurnToolCall};

#[cfg(test)]
mod tests;
