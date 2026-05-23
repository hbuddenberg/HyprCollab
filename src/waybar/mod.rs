pub mod keybinds;
use serde::Serialize;
use std::fs;
use tracing::info;

use crate::state::State;
use crate::utils::paths;

/// Waybar state published to the status file
#[derive(Serialize)]
pub struct WaybarState {
    pub text: String,
    pub alt: String,
    pub class: String,
    pub tooltip: String,
}

/// Write waybar.json reflecting current daemon state.
/// Called at daemon start and after every SendMessage (begin + end).
pub fn publish_from_state(state: &State) -> anyhow::Result<()> {
    let (text, alt, class) = if state.working {
        ("⏳ HyprCollab".into(), "processing".into(), "processing".into())
    } else {
        ("\u{f1ae} HyprCollab".into(), "idle".into(), "idle".into())
    };

    let tooltip = match state.active_chat() {
        Some(c) => {
            let agent = state.agents.get(state.active_agent_idx)
                .map(|a| a.name.as_str())
                .unwrap_or("?");
            format!("HyprCollab — {} • {}", c.title, agent)
        }
        None => "HyprCollab — no active chat".into(),
    };

    let ws = WaybarState { text, alt, class, tooltip };
    let path = paths::waybar_status_path();
    fs::create_dir_all(path.parent().unwrap())?;
    fs::write(&path, serde_json::to_string(&ws)?)?;
    Ok(())
}

/// Handle waybar action from CLI
pub fn handle_install(force: bool) -> anyhow::Result<()> {
    let waybar_dir = crate::utils::paths::config_dir().join("waybar");

    fs::create_dir_all(&waybar_dir)?;

    let module_config = r#"{
        "custom/hyprcollab": {
            "exec": "hyprcollab waybar-status",
            "exec-on-event": true,
            "return-type": "json",
            "interval": 1,
            "signal": 10,
            "format": "{}",
            "on-click": "hyprcollab gui toggle"
        }
    }"#;

    let config_path = waybar_dir.join("hyprcollab.jsonc");
    if config_path.exists() && !force {
        println!("Waybar module already installed at {}. Use --force to reinstall.", config_path.display());
        return Ok(());
    }
    fs::write(&config_path, module_config)?;

    info!("Waybar module installed at {}", config_path.display());
    println!("Waybar module installed at {}", config_path.display());
    println!("Add to your waybar config: \"custom/HyprCollab\"");
    Ok(())
}

pub fn handle_remove() -> anyhow::Result<()> {
    let config_path = crate::utils::paths::config_dir().join("waybar").join("HyprCollab.jsonc");

    if config_path.exists() {
        fs::remove_file(&config_path)?;
        info!("Waybar module removed");
        println!("Waybar module removed");
    } else {
        println!("Waybar module not installed");
    }
    Ok(())
}

pub fn handle_status() -> anyhow::Result<()> {
    let config_path = crate::utils::paths::config_dir().join("waybar").join("HyprCollab.jsonc");

    if config_path.exists() {
        println!("Waybar module installed at {}", config_path.display());
    } else {
        println!("Waybar module not installed");
    }
    Ok(())
}

pub fn handle_update() -> anyhow::Result<()> {
    handle_install(true)
}

/// Print waybar status JSON to stdout
pub fn print_status() -> anyhow::Result<()> {
    let state_path = paths::waybar_status_path();
    if state_path.exists() {
        let content = fs::read_to_string(&state_path)?;
        println!("{}", content);
    } else {
        let idle = WaybarState {
            text: "🤖 HyprCollab".to_string(),
            alt: "idle".to_string(),
            class: "idle".to_string(),
            tooltip: "HyprCollab — Click to open".to_string(),
        };
        println!("{}", serde_json::to_string(&idle)?);
    }
    Ok(())
}
