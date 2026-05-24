use dioxus::prelude::*;
use serde::{Deserialize, Serialize};

// ── RiskLevel ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RiskLevel {
    Low,
    Medium,
    High,
}

impl RiskLevel {
    fn color(&self) -> &'static str {
        match self {
            Self::Low => "#3fb950",
            Self::Medium => "#d29922",
            Self::High => "#f85149",
        }
    }

    fn label(&self) -> &'static str {
        match self {
            Self::Low => "low risk",
            Self::Medium => "medium risk",
            Self::High => "high risk",
        }
    }
}

// ── ApprovalDialog ────────────────────────────────────────────────────────────

#[component]
pub fn ApprovalDialog(
    tool_name: String,
    args: String,
    risk: RiskLevel,
    on_approve: EventHandler<()>,
    on_deny: EventHandler<()>,
    on_modify: EventHandler<String>,
) -> Element {
    let mut modify_mode = use_signal(|| false);
    let mut edited_args = use_signal(|| args.clone());

    let risk_color = risk.color();
    let risk_label = risk.label();

    let pretty_args = serde_json::from_str::<serde_json::Value>(&args)
        .map(|v| serde_json::to_string_pretty(&v).unwrap_or_else(|_| args.clone()))
        .unwrap_or_else(|_| args.clone());

    rsx! {
        // Backdrop
        div {
            style: "position: fixed; inset: 0; background: rgba(0,0,0,0.7); backdrop-filter: blur(4px); display: flex; align-items: center; justify-content: center; z-index: 1000;",
            onclick: move |_| on_deny.call(()),

            // Card — stop propagation so clicking inside doesn't close
            div {
                style: "background: #161b22; border: 1px solid #58a6ff; border-radius: 12px; padding: 24px; width: 480px; max-width: 90vw; box-shadow: 0 0 32px rgba(88,166,255,0.2);",
                // Stop click from reaching backdrop
                onclick: move |evt| evt.stop_propagation(),

                // Title row
                div {
                    style: "display: flex; align-items: center; gap: 10px; margin-bottom: 16px;",
                    span { style: "font-size: 1.1em; color: #58a6ff;", "🔐" }
                    h3 {
                        style: "margin: 0; color: #c9d1d9; font-size: 1em; flex: 1;",
                        "Tool Approval Required"
                    }
                    // Risk badge
                    span {
                        style: "font-size: 0.75em; color: {risk_color}; background: #21262d; border: 1px solid {risk_color}; padding: 2px 8px; border-radius: 4px; font-weight: 600;",
                        "{risk_label}"
                    }
                }

                // Tool name
                p {
                    style: "margin: 0 0 12px; color: #8b949e; font-size: 0.85em; text-transform: uppercase; letter-spacing: 0.05em;",
                    "Tool"
                }
                div {
                    style: "background: #0d1117; padding: 8px 12px; border-radius: 6px; margin-bottom: 16px; font-family: monospace; color: #c9d1d9; font-weight: 600;",
                    "{tool_name}"
                }

                // Args section
                p {
                    style: "margin: 0 0 6px; color: #8b949e; font-size: 0.85em; text-transform: uppercase; letter-spacing: 0.05em;",
                    "Arguments"
                }

                if *modify_mode.read() {
                    textarea {
                        style: "width: 100%; background: #0d1117; color: #c9d1d9; border: 1px solid #58a6ff; border-radius: 6px; padding: 10px; font-family: monospace; font-size: 0.85em; resize: vertical; min-height: 120px; box-sizing: border-box; margin-bottom: 16px; outline: none;",
                        value: "{edited_args}",
                        oninput: move |evt| edited_args.set(evt.value()),
                    }
                } else {
                    pre {
                        style: "background: #0d1117; color: #c9d1d9; padding: 10px; border-radius: 6px; overflow-x: auto; font-size: 0.85em; margin: 0 0 16px;",
                        "{pretty_args}"
                    }
                }

                // Action buttons
                div {
                    style: "display: flex; gap: 8px; justify-content: flex-end;",

                    if *modify_mode.read() {
                        // Confirm modified args
                        button {
                            style: "background: #d29922; color: #0d1117; border: none; padding: 8px 18px; border-radius: 6px; cursor: pointer; font-weight: 600;",
                            onclick: move |_| {
                                let args = edited_args.peek().clone();
                                on_modify.call(args);
                            },
                            "Confirm Edit"
                        }
                        button {
                            style: "background: transparent; color: #8b949e; border: 1px solid #30363d; padding: 8px 18px; border-radius: 6px; cursor: pointer;",
                            onclick: move |_| modify_mode.set(false),
                            "Cancel"
                        }
                    } else {
                        button {
                            style: "background: #d29922; color: #0d1117; border: none; padding: 8px 18px; border-radius: 6px; cursor: pointer; font-weight: 600;",
                            onclick: move |_| {
                                edited_args.set(pretty_args.clone());
                                modify_mode.set(true);
                            },
                            "Modify"
                        }
                        button {
                            style: "background: #f85149; color: #fff; border: none; padding: 8px 18px; border-radius: 6px; cursor: pointer; font-weight: 600;",
                            onclick: move |_| on_deny.call(()),
                            "Deny"
                        }
                        button {
                            style: "background: #3fb950; color: #0d1117; border: none; padding: 8px 18px; border-radius: 6px; cursor: pointer; font-weight: 600;",
                            onclick: move |_| on_approve.call(()),
                            "Approve"
                        }
                    }
                }
            }
        }
    }
}
