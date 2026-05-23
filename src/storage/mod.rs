pub mod folders;
pub mod messages;

use serde::{Deserialize, Serialize};
use std::fs;
use tracing::info;

/// Settings loaded from ~/.config/HyprCollab/config.yaml
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {
    #[serde(default)]
    pub general: GeneralConfig,
    #[serde(default)]
    pub agents: Vec<AgentConfig>,
    #[serde(default)]
    pub ui: UiConfig,
    #[serde(default)]
    pub waybar: WaybarConfig,
    #[serde(default)]
    pub rag: RagConfig,
    #[serde(default)]
    pub keys: KeysConfig,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GeneralConfig {
    pub language: String,
    pub default_agent: String,
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self { language: "en".into(), default_agent: "openai".into() }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AgentConfig {
    pub name: String,
    pub agent_type: String,
    pub base_url: Option<String>,
    pub api_key: Option<String>,
    pub model: Option<String>,
    pub command: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UiConfig {
    pub theme: String,
    pub font_size: u32,
    pub opacity: f64,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self { theme: "console".into(), font_size: 13, opacity: 0.95 }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct WaybarConfig {
    pub enabled: bool,
    pub refresh_interval: u64,
}

impl Default for WaybarConfig {
    fn default() -> Self {
        Self { enabled: true, refresh_interval: 1 }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RagConfig {
    pub chunk_size: usize,
    pub chunk_overlap: usize,
    pub top_k: usize,
    pub embedder: String,
    /// "folder" = solo carpeta activa | "global" = todas las carpetas | "both" = carpeta + global
    #[serde(default = "RagConfig::default_scope")]
    pub scope: String,
}

impl RagConfig {
    fn default_scope() -> String { "both".into() }

    pub fn is_folder(&self) -> bool { self.scope == "folder" }
    pub fn is_global(&self) -> bool { self.scope == "global" }
    pub fn is_both(&self)   -> bool { self.scope == "both" || (!self.is_folder() && !self.is_global()) }
}

impl Default for RagConfig {
    fn default() -> Self {
        Self {
            chunk_size: 500,
            chunk_overlap: 50,
            top_k: 5,
            embedder: "text-embedding-3-small".into(),
            scope: Self::default_scope(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct KeysConfig {
    pub toggle: String,
    pub menu: String,
}

impl Default for KeysConfig {
    fn default() -> Self {
        Self { toggle: "SUPER+ALT+A".into(), menu: "SUPER+ALT+M".into() }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            general: GeneralConfig::default(),
            agents: vec![AgentConfig {
                name: "openai".into(),
                agent_type: "openai".into(),
                base_url: Some("http://localhost:11434/v1".into()),
                api_key: None,
                model: Some("llama3".into()),
                command: None,
            }],
            ui: UiConfig::default(),
            waybar: WaybarConfig::default(),
            rag: RagConfig::default(),
            keys: KeysConfig::default(),
        }
    }
}

impl Config {
    pub fn load() -> anyhow::Result<Self> {
        let config_path = crate::utils::paths::config_file();

        if config_path.exists() {
            let content = fs::read_to_string(&config_path)?;
            let config: Config = serde_yaml::from_str(&content)?;
            info!("Config loaded from {}", config_path.display());
            Ok(config)
        } else {
            fs::create_dir_all(config_path.parent().unwrap())?;
            let config = Config::default();
            let yaml = serde_yaml::to_string(&config)?;
            fs::write(&config_path, &yaml)?;
            info!("Default config created at {}", config_path.display());
            Ok(config)
        }
    }
}
