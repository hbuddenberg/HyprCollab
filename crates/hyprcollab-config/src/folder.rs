use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tracing::debug;

use hyprcollab_core::ApprovalMode;

/// Per-project / per-folder configuration found in `.hyprcollab/config.yaml`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FolderConfig {
    #[serde(default)]
    pub project_name: String,

    #[serde(default)]
    pub system_prompt: Option<String>,

    #[serde(default)]
    pub rag_enabled: bool,

    #[serde(default)]
    pub rag_paths: Vec<String>,

    #[serde(default)]
    pub default_model: Option<String>,

    #[serde(default)]
    pub approval_mode: Option<ApprovalMode>,
}

impl FolderConfig {
    /// Directory name used to store folder-level config.
    const DIR_NAME: &'static str = ".hyprcollab";

    /// Look for `.hyprcollab/config.yaml` starting from `start_dir` and walking
    /// up to the filesystem root. Returns `None` if no config is found.
    pub fn discover(start_dir: &Path) -> Result<Option<(PathBuf, Self)>> {
        let mut dir = start_dir
            .canonicalize()
            .context("canonicalizing start dir")?;

        loop {
            let config_path = dir.join(Self::DIR_NAME).join("config.yaml");
            if config_path.is_file() {
                debug!("Found folder config at {}", config_path.display());
                let config = Self::load_from_file(&config_path)?;
                return Ok(Some((dir, config)));
            }

            if !dir.pop() {
                // Reached the root without finding a config.
                return Ok(None);
            }
        }
    }

    /// Load a folder config from a specific directory path.
    ///
    /// Looks for `<path>/.hyprcollab/config.yaml`. Returns `Ok(None)` if
    /// the file does not exist.
    pub fn load_from_dir(path: &Path) -> Result<Option<Self>> {
        let config_path = path.join(Self::DIR_NAME).join("config.yaml");
        if !config_path.is_file() {
            return Ok(None);
        }
        let config = Self::load_from_file(&config_path)?;
        Ok(Some(config))
    }

    fn load_from_file(path: &Path) -> Result<Self> {
        let raw =
            std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        let interpolated = crate::global::interpolate_env_vars(&raw);
        let config: Self = serde_yaml::from_str(&interpolated)
            .with_context(|| format!("parsing {}", path.display()))?;
        Ok(config)
    }

    /// Return the path to the folder config given a parent directory.
    pub fn config_path_for(parent: &Path) -> PathBuf {
        parent.join(Self::DIR_NAME).join("config.yaml")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn load_from_dir_missing_returns_none() {
        let tmp = tempfile::tempdir().unwrap();
        let result = FolderConfig::load_from_dir(tmp.path()).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn load_from_dir_with_config() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        let hc = dir.join(".hyprcollab");
        fs::create_dir_all(&hc).unwrap();
        fs::write(
            hc.join("config.yaml"),
            "project_name: test\nrag_enabled: true\n",
        )
        .unwrap();

        let config = FolderConfig::load_from_dir(dir).unwrap().unwrap();
        assert_eq!(config.project_name, "test");
        assert!(config.rag_enabled);
    }
}
