use std::path::Path;

pub enum SlashCommand {
    Help,
    Run { command: String },
    Read { path: String, start: Option<usize>, end: Option<usize> },
    Clear,
    Unknown { raw: String },
}

/// Returns `None` if input does not start with "/".
pub fn parse(input: &str) -> Option<SlashCommand> {
    let input = input.trim();
    if !input.starts_with('/') {
        return None;
    }

    let rest = &input[1..]; // strip leading /
    let (cmd, args) = rest
        .split_once(char::is_whitespace)
        .map(|(c, a)| (c, a.trim()))
        .unwrap_or((rest, ""));

    Some(match cmd.to_lowercase().as_str() {
        "help" => SlashCommand::Help,
        "clear" => SlashCommand::Clear,
        "run" => {
            if args.is_empty() {
                SlashCommand::Unknown { raw: input.to_string() }
            } else {
                SlashCommand::Run { command: args.to_string() }
            }
        }
        "read" => {
            // /read <path> [start:end]
            let parts: Vec<&str> = args.splitn(2, char::is_whitespace).collect();
            let path = parts.first().copied().unwrap_or("").to_string();
            let range = parts.get(1).copied().unwrap_or("");
            let (start, end) = parse_range(range);
            if path.is_empty() {
                SlashCommand::Unknown { raw: input.to_string() }
            } else {
                SlashCommand::Read { path, start, end }
            }
        }
        _ => SlashCommand::Unknown { raw: input.to_string() },
    })
}

fn parse_range(s: &str) -> (Option<usize>, Option<usize>) {
    let s = s.trim();
    if s.is_empty() {
        return (None, None);
    }
    if let Some((a, b)) = s.split_once(':') {
        let start = a.trim().parse().ok();
        let end = b.trim().parse().ok();
        (start, end)
    } else {
        (None, None)
    }
}

/// Execute a parsed slash command; returns text for the chat bubble.
pub async fn execute(cmd: SlashCommand) -> String {
    match cmd {
        SlashCommand::Help => help_text(),

        SlashCommand::Run { command } => {
            match tokio::process::Command::new("sh")
                .arg("-c")
                .arg(&command)
                .output()
                .await
            {
                Ok(out) => {
                    let stdout = String::from_utf8_lossy(&out.stdout);
                    let stderr = String::from_utf8_lossy(&out.stderr);
                    let exit = out.status.code().unwrap_or(-1);
                    let mut result = format!("```\n$ {}\n", command);
                    if !stdout.is_empty() {
                        result.push_str(&stdout);
                    }
                    if !stderr.is_empty() {
                        result.push_str(&stderr);
                    }
                    if exit != 0 {
                        result.push_str(&format!("\n[exit {}]", exit));
                    }
                    result.push_str("```");
                    result
                }
                Err(e) => format!("Error running command: {}", e),
            }
        }

        SlashCommand::Read { path, start, end } => {
            let expanded = expand_tilde(&path);
            match std::fs::read_to_string(&expanded) {
                Ok(content) => {
                    let lines: Vec<&str> = content.lines().collect();
                    let total = lines.len();
                    let s = start.unwrap_or(1).saturating_sub(1);
                    let e = end.unwrap_or(total).min(total);
                    let slice = lines[s..e].join("\n");
                    let ext = Path::new(&expanded)
                        .extension()
                        .and_then(|e| e.to_str())
                        .unwrap_or("");
                    format!("```{}\n{}\n```\n_(lines {}-{} of {})_", ext, slice, s + 1, e, total)
                }
                Err(e) => format!("Cannot read `{}`: {}", path, e),
            }
        }

        SlashCommand::Clear => {
            // folder_id is not available here; caller should handle clear directly
            "Use /clear from a folder with an active chat to reset its RAG index.".into()
        }

        SlashCommand::Unknown { raw } => {
            format!("Unknown command: `{}`\n\n{}", raw, help_text())
        }
    }
}

fn help_text() -> String {
    "**HyprCollab — Slash Commands**\n\n\
     `/help` — Show this message\n\
     `/run <cmd>` — Execute shell command and show output\n\
     `/read <path> [start:end]` — Show file contents (optional line range)\n\
     `/clear` — Clear RAG index for current folder\n\
    "
    .into()
}

fn expand_tilde(path: &str) -> String {
    if path.starts_with("~/") {
        if let Ok(home) = std::env::var("HOME") {
            return format!("{}{}", home, &path[1..]);
        }
    }
    path.to_string()
}
