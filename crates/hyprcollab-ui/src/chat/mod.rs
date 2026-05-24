pub mod autocomplete;

use dioxus::prelude::*;
use serde::{Deserialize, Serialize};

// ── Types ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: MessageRole,
    pub content: String,
    pub timestamp: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    User,
    Assistant,
    System,
}

// ── ChatPanel ────────────────────────────────────────────────────────────────

#[component]
pub fn ChatPanel(messages: Vec<ChatMessage>) -> Element {
    rsx! {
        div {
            style: "background: #0d1117; height: 100%; overflow-y: auto; display: flex; flex-direction: column; gap: 8px; padding: 16px; box-sizing: border-box;",
            for msg in messages.iter() {
                MessageBubble { message: msg.clone() }
            }
        }
    }
}

// ── MessageBubble ────────────────────────────────────────────────────────────

#[component]
pub fn MessageBubble(message: ChatMessage) -> Element {
    let (wrapper_style, bubble_style) = match message.role {
        MessageRole::User => (
            "display: flex; justify-content: flex-end; margin: 4px 0;",
            "background: #1f6feb; color: #c9d1d9; padding: 10px 14px; border-radius: 12px 12px 2px 12px; max-width: 70%;",
        ),
        MessageRole::Assistant => (
            "display: flex; justify-content: flex-start; margin: 4px 0;",
            "background: #161b22; color: #c9d1d9; padding: 10px 14px; border-radius: 12px 12px 12px 2px; max-width: 70%; border: 1px solid #30363d;",
        ),
        MessageRole::System => (
            "display: flex; justify-content: center; margin: 4px 0;",
            "background: #21262d; color: #8b949e; padding: 6px 12px; border-radius: 8px; font-size: 0.85em;",
        ),
    };

    rsx! {
        div { style: "{wrapper_style}",
            div { style: "{bubble_style}",
                p { style: "margin: 0; white-space: pre-wrap; word-break: break-word;",
                    "{message.content}"
                }
                span {
                    style: "font-size: 0.75em; opacity: 0.6; display: block; margin-top: 4px;",
                    "{message.timestamp}"
                }
            }
        }
    }
}

// ── ChatInput ────────────────────────────────────────────────────────────────

#[component]
pub fn ChatInput(on_send: EventHandler<String>) -> Element {
    let mut input_value = use_signal(String::new);

    rsx! {
        div {
            style: "padding: 12px; border-top: 1px solid #30363d; background: #0d1117;",
            textarea {
                style: "width: 100%; background: #161b22; color: #c9d1d9; border: 1px solid #30363d; border-radius: 8px; padding: 10px; resize: none; font-family: inherit; font-size: 14px; outline: none; box-sizing: border-box; min-height: 42px; max-height: 200px; overflow-y: auto;",
                rows: "1",
                placeholder: "Type a message... (Enter to send, Shift+Enter for newline)",
                value: "{input_value}",
                oninput: move |evt| {
                    input_value.set(evt.value());
                },
                onkeydown: move |evt: Event<KeyboardData>| {
                    let key = evt.key();
                    let shift = evt.modifiers().contains(Modifiers::SHIFT);
                    if key == Key::Enter && !shift {
                        let text = input_value.peek().clone();
                        if !text.trim().is_empty() {
                            on_send.call(text);
                            input_value.set(String::new());
                        }
                    }
                }
            }
        }
    }
}
