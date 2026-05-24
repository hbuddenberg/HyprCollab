use dioxus::prelude::*;

// ── Status type ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum ToolStatus {
    Running,
    Success,
    Error(String),
}

impl ToolStatus {
    fn color(&self) -> &'static str {
        match self {
            Self::Running => "#d29922",
            Self::Success => "#3fb950",
            Self::Error(_) => "#f85149",
        }
    }

    fn label(&self) -> String {
        match self {
            Self::Running => "running".to_string(),
            Self::Success => "success".to_string(),
            Self::Error(msg) => format!("error: {msg}"),
        }
    }
}

// ── ToolCallCard ─────────────────────────────────────────────────────────────

static CARD_CSS: &str = r#"
@keyframes pulse-yellow {
  0%, 100% { opacity: 1; }
  50% { opacity: 0.4; }
}
.tool-status-running { animation: pulse-yellow 1.2s ease-in-out infinite; }
"#;

#[component]
pub fn ToolCallCard(
    tool_name: String,
    args: String,
    output: String,
    status: ToolStatus,
    duration_ms: Option<u64>,
) -> Element {
    let mut expanded = use_signal(|| false);

    let status_color = status.color();
    let status_label = status.label();
    let is_running = matches!(status, ToolStatus::Running);

    let duration_text = duration_ms
        .map(|ms| format!("{ms}ms"))
        .unwrap_or_default();

    let pretty_args = serde_json::from_str::<serde_json::Value>(&args)
        .map(|v| serde_json::to_string_pretty(&v).unwrap_or_else(|_| args.clone()))
        .unwrap_or_else(|_| args.clone());

    let status_class = if is_running { "tool-status-running" } else { "" };

    rsx! {
        style { "{CARD_CSS}" }
        div {
            style: "background: #161b22; border: 1px solid #30363d; border-radius: 8px; overflow: hidden; margin: 6px 0;",

            // Header
            div {
                style: "background: #1c2129; padding: 8px 12px; display: flex; align-items: center; gap: 8px; cursor: pointer; user-select: none;",
                onclick: move |_| {
                    let cur = *expanded.read();
                    expanded.set(!cur);
                },

                // Tool icon
                span { style: "font-size: 1em;", "⚙" }

                // Tool name
                span {
                    style: "color: #c9d1d9; font-weight: 600; font-family: monospace; flex: 1;",
                    "{tool_name}"
                }

                // Duration badge
                if !duration_text.is_empty() {
                    span {
                        style: "font-size: 0.75em; color: #8b949e; background: #21262d; padding: 2px 6px; border-radius: 4px;",
                        "{duration_text}"
                    }
                }

                // Status badge
                span {
                    class: "{status_class}",
                    style: "font-size: 0.75em; color: {status_color}; background: #21262d; padding: 2px 6px; border-radius: 4px;",
                    "{status_label}"
                }

                // Expand/collapse arrow
                span {
                    style: "color: #8b949e; font-size: 0.8em; margin-left: 4px;",
                    if *expanded.read() { "▲" } else { "▼" }
                }
            }

            // Expanded body
            if *expanded.read() {
                div { style: "padding: 12px;",

                    // Args
                    p { style: "margin: 0 0 6px; font-size: 0.75em; color: #8b949e; text-transform: uppercase; letter-spacing: 0.05em;", "Arguments" }
                    pre {
                        style: "background: #0d1117; color: #c9d1d9; padding: 10px; border-radius: 6px; overflow-x: auto; font-size: 0.85em; margin: 0 0 12px;",
                        "{pretty_args}"
                    }

                    // Output
                    p { style: "margin: 0 0 6px; font-size: 0.75em; color: #8b949e; text-transform: uppercase; letter-spacing: 0.05em;", "Output" }
                    pre {
                        style: "background: #0d1117; color: #c9d1d9; padding: 10px; border-radius: 6px; overflow-x: auto; font-size: 0.85em; margin: 0; border-left: 2px solid #30363d;",
                        "{output}"
                    }
                }
            }
        }
    }
}
