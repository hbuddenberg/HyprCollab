use std::fs;
use std::path::PathBuf;
use tracing::info;

use crate::storage::KeysConfig;

/// Install Hyprland keybinding to toggle HyprCollab
pub fn install_keybinds(keys: &KeysConfig) -> anyhow::Result<()> {
    let bind_file = get_bind_path();
    let content = generate_bind_content(keys);

    fs::create_dir_all(bind_file.parent().unwrap())?;
    fs::write(&bind_file, &content)?;

    // Add source line to hyprland.conf
    let hypr_conf = get_hypr_conf_path();
    let source_line = format!("source = {}", bind_file.display());

    if hypr_conf.exists() {
        let conf = fs::read_to_string(&hypr_conf)?;
        if !conf.contains(&source_line) {
            let updated = format!("{}\n# HyprCollab keybind\n{}\n", conf.trim_end(), source_line);
            fs::write(&hypr_conf, &updated)?;
            info!("Keybind source added to {}", hypr_conf.display());
        }
    }

    info!("Keybinds installed to {}", bind_file.display());
    println!("Keybinds installed to {}", bind_file.display());
    println!("Reload Hyprland: hyprctl reload");

    // Try to reload hyprctl
    let _ = std::process::Command::new("hyprctl")
        .args(["reload"])
        .output();

    Ok(())
}

/// Remove Hyprland keybinding
pub fn remove_keybinds() -> anyhow::Result<()> {
    let bind_file = get_bind_path();
    if bind_file.exists() {
        fs::remove_file(&bind_file)?;
        info!("Keybinds removed");
        println!("Keybinds removed");
    }

    // Remove source line from hyprland.conf
    let hypr_conf = get_hypr_conf_path();
    if hypr_conf.exists() {
        let conf = fs::read_to_string(&hypr_conf)?;
        let source_line = format!("source = {}", bind_file.display());
        let updated = conf.replace(&format!("\n# HyprCollab keybind\n{}\n", source_line), "");
        fs::write(&hypr_conf, &updated)?;
    }

    Ok(())
}

/// Show keybind status
pub fn show_status() -> anyhow::Result<()> {
    let bind_file = get_bind_path();
    if bind_file.exists() {
        println!("Keybinds installed at {}", bind_file.display());
        println!("{}", fs::read_to_string(&bind_file)?);
    } else {
        println!("Keybinds not installed");
    }
    Ok(())
}

fn get_bind_path() -> PathBuf {
    let config = crate::utils::paths::config_dir();
    config.join("hypr").join("HyprCollab-keybinds.conf")
}

fn get_hypr_conf_path() -> PathBuf {
    // Standard location for Hyprland config
    let xdg = std::env::var("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
            PathBuf::from(home).join(".config")
        });
    xdg.join("hypr").join("hyprland.conf")
}

fn generate_bind_content(keys: &KeysConfig) -> String {
    // Detect Hyprland version to use correct config syntax
    let version = get_hyprland_version();
    let is_lua = version >= 0.55;

    let toggle_bind = parse_keybind(&keys.toggle);
    let menu_bind = parse_keybind(&keys.menu);

    if is_lua {
        generate_lua_binds(&toggle_bind, &menu_bind)
    } else {
        generate_conf_binds(&toggle_bind, &menu_bind)
    }
}

fn generate_conf_binds(toggle: &KeyCombo, menu: &KeyCombo) -> String {
    format!(
        "# HyprCollab keybinds\n\
         # https://github.com/hbuddenberg/hypr-collab\n\n\
         bind = {}, exec, hypr-collab gui toggle\n\
         bind = {}, exec, hypr-collab gui show\n",
        toggle.to_modmask_string(),
        menu.to_modmask_string(),
    )
}

fn generate_lua_binds(toggle: &KeyCombo, menu: &KeyCombo) -> String {
    format!(
        "-- HyprCollab keybinds\n\
         -- https://github.com/hbuddenberg/hypr-collab\n\n\
         hyprland.keybinds = {{\n\
         \t{{ modmask = {}, key = \"{}\", dispatcher = \"exec\", args = \"hypr-collab gui toggle\" }},\n\
         \t{{ modmask = {}, key = \"{}\", dispatcher = \"exec\", args = \"hypr-collab gui show\" }},\n\
         }}\n",
        toggle.modmask, toggle.key,
        menu.modmask, menu.key,
    )
}

fn get_hyprland_version() -> f64 {
    let output = std::process::Command::new("hyprctl")
        .args(["version"])
        .output();
    match output {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            // Extract version number: "Hyprland 0.47.2" → 0.47
            for part in stdout.split_whitespace() {
                if let Some(ver) = part.strip_prefix("Hyprland") {
                    // Try next token
                    continue;
                }
                if let Ok(v) = part.parse::<f64>() {
                    return v;
                }
                // Try parsing semver: "0.47.2" → 0.47
                let parts: Vec<&str> = part.splitn(3, '.').collect();
                if parts.len() >= 2 {
                    if let (Ok(major), Ok(minor)) = (parts[0].parse::<f64>(), parts[1].parse::<f64>()) {
                        return major + minor / 10.0;
                    }
                }
            }
            0.0
        }
        Err(_) => 0.0,
    }
}

#[derive(Debug)]
struct KeyCombo {
    modmask: u32,
    key: String,
}

impl KeyCombo {
    fn to_modmask_string(&self) -> String {
        format!("{}, {}", self.modmask, self.key)
    }
}

fn parse_keybind(bind: &str) -> KeyCombo {
    // Parse "SUPER+ALT+A" into modmask + key
    let parts: Vec<&str> = bind.rsplitn(2, '+').collect();
    let key = parts[0].to_lowercase();

    let mods = if parts.len() > 1 {
        parts[1].to_uppercase()
    } else {
        String::new()
    };

    let mut modmask = 0u32;
    for m in mods.split('+') {
        match m.trim() {
            "SUPER" | "WIN" | "MOD4" => modmask |= 64,
            "SHIFT" => modmask |= 1,
            "CTRL" | "CONTROL" => modmask |= 4,
            "ALT" | "MOD1" => modmask |= 8,
            "MOD2" => modmask |= 2,
            "MOD3" => modmask |= 16,
            "MOD5" => modmask |= 32,
            _ => {}
        }
    }

    KeyCombo { modmask, key }
}
