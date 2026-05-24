//! Core agent loop: LLM → tool call → execute → repeat.

use std::time::Instant;

use hyprcollab_core::errors::CoreError;
use hyprcollab_core::traits::LlmProvider;
use hyprcollab_core::types::*;

use crate::tool_registry::ToolRegistry;
use crate::types::{AgentConfig, AgentOutput};

/// Run the agent loop to completion.
///
/// `messages` is mutated in-place: the user message must already be appended
/// by the caller, and this function will push assistant messages, tool-result
/// messages, etc. as the loop progresses.
///
/// `approval` is consulted before each tool invocation. When `Strict` mode
/// returns `needs_approval = true`, the tool call is skipped and a synthetic
/// tool-result message is injected instead.
pub async fn run_agent_loop(
    provider: &dyn LlmProvider,
    messages: &mut Vec<Message>,
    registry: &ToolRegistry,
    chat_id: ChatId,
    config: &AgentConfig,
    approval: Option<&hyprcollab_approval::ApprovalEngine>,
) -> Result<AgentOutput, CoreError> {
    let mut tool_history: Vec<crate::types::TurnToolCall> = Vec::new();
    let tool_defs = registry.tool_definitions();

    for turn in 0..config.max_turns {
        // Build the ChatRequest from the current conversation state.
        let request = ChatRequest {
            model: config.model.clone(),
            messages: messages.clone(),
            tools: tool_defs.clone(),
            temperature: config.temperature,
            max_tokens: None,
            stream: false,
            persona: None,
            agent_role: None,
        };

        let response = provider.chat_completion(request).await?;

        // Append the assistant's reply to the conversation history.
        messages.push(response.message.clone());

        match response.finish_reason {
            FinishReason::Stop => {
                tracing::debug!(turn, "Agent loop finished (Stop)");
                return Ok(AgentOutput {
                    final_response: response,
                    tool_calls: tool_history,
                    turns_used: turn + 1,
                });
            }

            FinishReason::ToolCalls => {
                // Grab the tool calls from the assistant message we just pushed.
                let assistant_msg = messages.last().expect("just pushed a message");
                let calls = assistant_msg.tool_calls.clone();

                tracing::debug!(turn, n_calls = calls.len(), "Executing tool calls");

                for tc in calls {
                    // Check approval before executing the tool.
                    let blocked = approval.is_some_and(|eng| {
                        eng.needs_approval(&tc.name, &tc.arguments)
                            && config.approval_mode == hyprcollab_core::types::ApprovalMode::Strict
                    });
                    if blocked {
                        tracing::info!(tool = %tc.name, "Tool skipped: requires approval in Strict mode");
                        let tool_msg = Message {
                            id: MessageId::new(),
                            chat_id,
                            role: MessageRole::Tool,
                            content: format!(
                                "Tool '{}' was not executed: approval required (Strict mode)",
                                tc.name
                            ),
                            tool_calls: Vec::new(),
                            artifacts: Vec::new(),
                            timestamp: chrono::Utc::now(),
                            metadata: serde_json::json!({
                                "tool_call_id": tc.id,
                                "is_error": true,
                                "approval_blocked": true,
                            }),
                        };
                        messages.push(tool_msg);
                        continue;
                    }

                    let tool = registry.get(&tc.name).ok_or_else(|| {
                        CoreError::Tool(format!("Tool not found: {}", tc.name))
                    })?;

                    let start = Instant::now();
                    let exec_result = tool.execute(tc.arguments.clone()).await;
                    let duration_ms = start.elapsed().as_millis() as u64;

                    let (result_str, is_error) = match exec_result {
                        Ok(s) => (s, false),
                        Err(e) => (e.to_string(), true),
                    };

                    tracing::debug!(
                        turn,
                        tool = %tc.name,
                        duration_ms,
                        is_error,
                        "Tool execution complete"
                    );

                    tool_history.push(crate::types::TurnToolCall {
                        turn: turn + 1,
                        tool_name: tc.name.clone(),
                        arguments: tc.arguments.clone(),
                        result: result_str.clone(),
                        duration_ms,
                    });

                    // Append a Tool-result message so the LLM sees the output.
                    let tool_msg = Message {
                        id: MessageId::new(),
                        chat_id,
                        role: MessageRole::Tool,
                        content: result_str,
                        tool_calls: Vec::new(),
                        artifacts: Vec::new(),
                        timestamp: chrono::Utc::now(),
                        metadata: serde_json::json!({
                            "tool_call_id": tc.id,
                            "is_error": is_error,
                        }),
                    };
                    messages.push(tool_msg);
                }
                // Loop back to the LLM with the tool results appended.
                continue;
            }

            // Length, ContentFilter, or any future variant – return as-is.
            other => {
                tracing::debug!(turn, ?other, "Agent loop finished (non-Stop finish reason)");
                return Ok(AgentOutput {
                    final_response: response,
                    tool_calls: tool_history,
                    turns_used: turn + 1,
                });
            }
        }
    }

    // Exhausted the turn budget.
    tracing::warn!(max_turns = config.max_turns, "Max turns exceeded");
    Err(CoreError::Llm(format!(
        "Agent exceeded maximum turns ({})",
        config.max_turns
    )))
}
