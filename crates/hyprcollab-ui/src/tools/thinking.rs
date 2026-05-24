use dioxus::prelude::*;

static THINKING_CSS: &str = r#"
@keyframes thinking-dot {
  0%, 100% { opacity: 0.2; }
  50% { opacity: 1; }
}
.td1 { animation: thinking-dot 1.4s infinite; }
.td2 { animation: thinking-dot 1.4s infinite 0.2s; }
.td3 { animation: thinking-dot 1.4s infinite 0.4s; }
"#;

// ── ThinkingBlock ─────────────────────────────────────────────────────────────

#[component]
pub fn ThinkingBlock(
    content: String,
    in_progress: bool,
) -> Element {
    let mut expanded = use_signal(|| false);

    let header_label = if in_progress {
        // Animated dots rendered via CSS (see THINKING_CSS)
        rsx! {
            style { "{THINKING_CSS}" }
            span { "💭 Thinking" }
            span { class: "td1", "." }
            span { class: "td2", "." }
            span { class: "td3", "." }
        }
    } else {
        rsx! {
            span { "💭 Thinking" }
        }
    };

    rsx! {
        div {
            style: "margin: 6px 0;",

            // Header
            div {
                style: "display: flex; align-items: center; gap: 6px; cursor: pointer; user-select: none; color: #8b949e; font-size: 0.9em; padding: 4px 0;",
                onclick: move |_| {
                    let cur = *expanded.read();
                    expanded.set(!cur);
                },
                {header_label}
                span { style: "margin-left: 4px; font-size: 0.8em;",
                    if *expanded.read() { "▲" } else { "▼" }
                }
            }

            // Collapsible content
            if *expanded.read() {
                div {
                    style: "border-left: 3px solid #8b949e; padding: 8px 12px; margin-top: 4px; background: #0d1117; border-radius: 0 6px 6px 0;",
                    pre {
                        style: "color: #8b949e; font-family: monospace; font-size: 0.85em; white-space: pre-wrap; word-break: break-word; margin: 0;",
                        "{content}"
                    }
                }
            }
        }
    }
}
