use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tracing::debug;

use hyprcollab_core::ApprovalMode;

// ── Sub-config structs ──────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneralConfig {
    #[serde(default = "GeneralConfig::default_app_name")]
    pub app_name: String,
    #[serde(default = "GeneralConfig::default_version")]
    pub version: String,
    #[serde(default = "GeneralConfig::default_language")]
    pub language: String,
    #[serde(default = "GeneralConfig::default_theme")]
    pub theme: String,
    #[serde(default = "GeneralConfig::default_data_dir")]
    pub data_dir: String,
}

impl GeneralConfig {
    fn default_app_name() -> String {
        "HyprCollab".into()
    }
    fn default_version() -> String {
        "0.1.0".into()
    }
    fn default_language() -> String {
        "en".into()
    }
    fn default_theme() -> String {
        "dark".into()
    }
    fn default_data_dir() -> String {
        dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("hyprcollab")
            .to_string_lossy()
            .into_owned()
    }
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            app_name: Self::default_app_name(),
            version: Self::default_version(),
            language: Self::default_language(),
            theme: Self::default_theme(),
            data_dir: Self::default_data_dir(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default)]
    pub base_url: Option<String>,
    #[serde(default)]
    pub models: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiConfig {
    #[serde(default = "UiConfig::default_sidebar_width")]
    pub sidebar_width: u32,
    #[serde(default = "UiConfig::default_show_status_bar")]
    pub show_status_bar: bool,
    #[serde(default = "UiConfig::default_font_size")]
    pub font_size: u32,
}

impl UiConfig {
    fn default_sidebar_width() -> u32 {
        280
    }
    fn default_show_status_bar() -> bool {
        true
    }
    fn default_font_size() -> u32 {
        14
    }
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            sidebar_width: Self::default_sidebar_width(),
            show_status_bar: Self::default_show_status_bar(),
            font_size: Self::default_font_size(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RagConfig {
    #[serde(default = "RagConfig::default_chunk_size")]
    pub chunk_size: usize,
    #[serde(default = "RagConfig::default_chunk_overlap")]
    pub chunk_overlap: usize,
    #[serde(default = "RagConfig::default_top_k")]
    pub top_k: usize,
}

impl RagConfig {
    fn default_chunk_size() -> usize {
        512
    }
    fn default_chunk_overlap() -> usize {
        64
    }
    fn default_top_k() -> usize {
        5
    }
}

impl Default for RagConfig {
    fn default() -> Self {
        Self {
            chunk_size: Self::default_chunk_size(),
            chunk_overlap: Self::default_chunk_overlap(),
            top_k: Self::default_top_k(),
        }
    }
}

// ── GlobalConfig ─────────────────────────────────────────────────────

/// Top-level configuration stored at `~/.config/hyprcollab/config.yaml`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalConfig {
    #[serde(default)]
    pub general: GeneralConfig,

    #[serde(default = "GlobalConfig::default_model")]
    pub default_model: String,

    #[serde(default)]
    pub providers: HashMap<String, ProviderConfig>,

    #[serde(default = "GlobalConfig::default_approval_mode")]
    pub approval_mode: ApprovalMode,

    #[serde(default)]
    pub ui: UiConfig,

    #[serde(default)]
    pub rag: RagConfig,
}

impl GlobalConfig {
    fn default_model() -> String {
        "anthropic/claude-sonnet-4".into()
    }

    fn default_approval_mode() -> ApprovalMode {
        ApprovalMode::Normal
    }

    /// Return the canonical config path: `~/.config/hyprcollab/config.yaml`.
    pub fn config_path() -> Result<PathBuf> {
        let base = dirs::config_dir().context("cannot determine config directory")?;
        Ok(base.join("hyprcollab").join("config.yaml"))
    }

    /// Load the global config from disk.
    ///
    /// If the file does not exist, returns the default config.
    /// String values containing `${ENV_VAR}` are interpolated from env vars.
    pub fn load() -> Result<Self> {
        let path = Self::config_path()?;
        if !path.exists() {
            debug!(
                "Global config not found at {}, using defaults",
                path.display()
            );
            return Ok(Self::default());
        }

        let raw = std::fs::read_to_string(&path)
            .with_context(|| format!("reading {}", path.display()))?;
        let interpolated = interpolate_env_vars(&raw);
        let config: Self = serde_yaml::from_str(&interpolated)
            .with_context(|| format!("parsing {}", path.display()))?;
        Ok(config)
    }

    /// Save the global config back to disk, creating parent directories as needed.
    pub fn save(&self) -> Result<()> {
        let path = Self::config_path()?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating {}", parent.display()))?;
        }
        let yaml = serde_yaml::to_string(self).context("serializing global config")?;
        std::fs::write(&path, yaml).with_context(|| format!("writing {}", path.display()))?;
        debug!("Saved global config to {}", path.display());
        Ok(())
    }
}

impl Default for GlobalConfig {
    fn default() -> Self {
        Self {
            general: GeneralConfig::default(),
            default_model: Self::default_model(),
            providers: HashMap::new(),
            approval_mode: Self::default_approval_mode(),
            ui: UiConfig::default(),
            rag: RagConfig::default(),
        }
    }
}

// ── Env var interpolation ────────────────────────────────────────────

/// Replace `${ENV_VAR}` occurrences in the input string with the value of
/// the corresponding environment variable. Missing variables are replaced
/// with an empty string.
pub(crate) fn interpolate_env_vars(input: &str) -> String {
    let mut result = input.to_owned();
    // Naïve but sufficient for config files: find `${...}` patterns.
    let re = regex_lazy();
    re.find_all(&input, &mut result);
    result
}

// We avoid adding `regex` as a dep — implement a simple scan instead.
struct EnvVarScanner;

impl EnvVarScanner {
    fn find_all(&self, _input: &str, output: &mut String) {
        // We rewrite `output` in-place. Start from the original.
        let src = output.clone();
        let mut buf = String::with_capacity(src.len());
        let mut chars = src.chars().peekable();

        while let Some(ch) = chars.next() {
            if ch == '$' && chars.peek() == Some(&'{') {
                chars.next(); // consume '{'
                let mut var = String::new();
                loop {
                    match chars.next() {
                        Some('}') => break,
                        Some(c) => var.push(c),
                        None => {
                            // Unterminated — push back what we consumed
                            buf.push_str("${");
                            buf.push_str(&var);
                            break;
                        }
                    }
                }
                if !var.is_empty() {
                    let val = std::env::var(&var).unwrap_or_default();
                    buf.push_str(&val);
                }
            } else {
                buf.push(ch);
            }
        }

        *output = buf;
    }
}

fn regex_lazy() -> EnvVarScanner {
    EnvVarScanner
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_round_trips() {
        let config = GlobalConfig::default();
        let yaml = serde_yaml::to_string(&config).unwrap();
        let parsed: GlobalConfig = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(config.default_model, parsed.default_model);
    }

    #[test]
    fn interpolate_known_var() {
        std::env::set_var("HYPRCOLLAB_TEST_INTERP", "hello_world");
        let result = interpolate_env_vars("key: ${HYPRCOLLAB_TEST_INTERP}");
        assert_eq!(result, "key: hello_world");
        std::env::remove_var("HYPRCOLLAB_TEST_INTERP");
    }

    #[test]
    fn interpolate_missing_var_yields_empty() {
        let result = interpolate_env_vars("key: ${HYPRCOLLAB_SHOULD_NOT_EXIST_XYZ}");
        assert_eq!(result, "key: ");
    }
}
