use dioxus::prelude::*;

// ── Command registry ─────────────────────────────────────────────────────────

struct CommandEntry {
    name: &'static str,
    description: &'static str,
}

static COMMANDS: &[CommandEntry] = &[
    CommandEntry { name: "/agent",       description: "Switch or configure agent" },
    CommandEntry { name: "/skill",       description: "Invoke a registered skill" },
    CommandEntry { name: "/mcp",         description: "Manage MCP server connections" },
    CommandEntry { name: "/acp",         description: "Agent communication protocol" },
    CommandEntry { name: "/design",      description: "Run design review" },
    CommandEntry { name: "/review",      description: "Review code or PR" },
    CommandEntry { name: "/run",         description: "Execute a command or script" },
    CommandEntry { name: "/browse",      description: "Open browser / navigate URL" },
    CommandEntry { name: "/model",       description: "Select AI model" },
    CommandEntry { name: "/help",        description: "Show available commands" },
    CommandEntry { name: "/config",      description: "Edit configuration" },
    CommandEntry { name: "/temperature", description: "Set sampling temperature" },
    CommandEntry { name: "/approval",    description: "Configure tool approval mode" },
    CommandEntry { name: "/workspace",   description: "Manage workspace settings" },
    CommandEntry { name: "/image",       description: "Attach or generate an image" },
    CommandEntry { name: "/scrape",      description: "Scrape a web page" },
];

// ── Fuzzy matcher ─────────────────────────────────────────────────────────────

fn fuzzy_match(pattern: &str, text: &str) -> bool {
    let mut text_chars = text.chars();
    'outer: for pc in pattern.chars() {
        loop {
            match text_chars.next() {
                Some(tc) if tc == pc => continue 'outer,
                Some(_) => {}
                None => return false,
            }
        }
    }
    true
}

// ── SlashAutocomplete ─────────────────────────────────────────────────────────

/// Dropdown that appears when the user types `/` in the chat input.
///
/// `input`     — current raw input text from the textarea.
/// `on_select` — called with the chosen command string (e.g. "/model").
/// `on_close`  — called when the dropdown should be dismissed (Escape).
///
/// Attach `onkeydown` on the parent input element to forward arrow / Enter /
/// Tab / Escape events to this component via the `tabindex="0"` wrapper.
#[component]
pub fn SlashAutocomplete(
    input: String,
    on_select: EventHandler<String>,
    on_close: EventHandler<()>,
) -> Element {
    let mut selected_idx = use_signal(|| 0_usize);

    let visible = input.starts_with('/');
    let query = input.to_lowercase();

    if !visible {
        return rsx! { Fragment {} };
    }

    let matches: Vec<&CommandEntry> = COMMANDS
        .iter()
        .filter(|cmd| fuzzy_match(&query, cmd.name))
        .collect();

    if matches.is_empty() {
        return rsx! { Fragment {} };
    }

    let max_idx = matches.len().saturating_sub(1);
    if *selected_idx.read() > max_idx {
        selected_idx.set(max_idx);
    }

    // Pre-extract names so the on_key closure can own them independently of
    // the `matches` Vec that the rsx! loop borrows below.
    let match_count = matches.len();
    let match_names: Vec<&'static str> = matches.iter().map(|c| c.name).collect();

    let on_key = move |evt: Event<KeyboardData>| {
        let key = evt.key();
        match key {
            Key::ArrowDown => {
                let cur = *selected_idx.read();
                selected_idx.set((cur + 1).min(match_count.saturating_sub(1)));
            }
            Key::ArrowUp => {
                let cur = *selected_idx.read();
                selected_idx.set(cur.saturating_sub(1));
            }
            Key::Escape => on_close.call(()),
            Key::Enter | Key::Tab => {
                let idx = *selected_idx.read();
                if let Some(&name) = match_names.get(idx) {
                    on_select.call(name.to_string());
                }
            }
            _ => {}
        }
    };

    rsx! {
        div {
            style: "position: absolute; bottom: 100%; left: 0; right: 0; background: #161b22; border: 1px solid #30363d; border-radius: 8px; overflow: hidden; z-index: 100; max-height: 260px; overflow-y: auto;",
            tabindex: "0",
            onkeydown: on_key,
            for (i, cmd) in matches.iter().enumerate() {
                div {
                    key: "{cmd.name}",
                    style: if i == *selected_idx.read() {
                        "background: #1f6feb; color: #c9d1d9; padding: 8px 12px; cursor: pointer; display: flex; justify-content: space-between; align-items: center;"
                    } else {
                        "background: transparent; color: #c9d1d9; padding: 8px 12px; cursor: pointer; display: flex; justify-content: space-between; align-items: center;"
                    },
                    onclick: {
                        let name = cmd.name;
                        move |_| on_select.call(name.to_string())
                    },
                    span { style: "font-weight: 600; font-family: monospace;", "{cmd.name}" }
                    span { style: "font-size: 0.85em; opacity: 0.7;", "{cmd.description}" }
                }
            }
        }
    }
}
